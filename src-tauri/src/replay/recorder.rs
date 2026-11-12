//! El grabador: un hilo que captura la pantalla a ritmo constante (30/60 fps),
//! la codifica en segmentos de ~15 s y los va cerrando sin cortes; un hilo
//! de E/S crea y finaliza los escritores (lo lento) para que la captura no se
//! pare nunca, y un hilo por fuente de audio.
//!
//! Si la captura se pierde (cambio de resolución, monitor apagado o
//! desconectado, GPU reiniciada, suspensión) el grabador vuelve a empezar solo.
//!
//! Guardar nunca puede quedarse colgado:
//! - Las peticiones de corte van a una cola compartida y quien guarda mira la
//!   fase del grabador (`Phase`, atómica): si no está grabando, contesta al
//!   instante («se está reiniciando…»), y si deja de grabar mientras espera,
//!   también.
//! - El corte cae en el primer fotograma alineado con las tramas AAC que haya
//!   a partir de la petición, aunque el bucle vaya tarde y se salte fotogramas
//!   (antes se exigía `k % paso == 0` con el `k` del reloj: con el bucle lento
//!   ese múltiplo exacto podía no tocar nunca y el guardado se quedaba
//!   esperando hasta agotar el tiempo).
//! - Al reiniciar, los hilos (audio, E/S) se esperan con tiempo máximo; si no
//!   terminan se abandonan con una línea en el registro.
//! - Un perro guardián reinicia el grabador si el bucle deja de avanzar.

use super::audio::{AudioSource, Chunk};
use super::capture::{self, Capture, Converter};
use super::encoder::{self, AacEncoder, EncoderConfig, SegmentWriter, SAMPLE_RATE};
use super::monitors;
use super::ring::{self, Ring, Segment};
use super::tonemap::ToneMapper;
use super::{join_bounded, qpc100, remux};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{CreateWaitableTimerExW, SetWaitableTimer, WaitForSingleObject, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION};

const AREA: &str = "clips";
/// Texturas NV12 en vuelo hacia el codificador.
const POOL: usize = 8;
const HNS: i64 = 10_000_000;
/// Lo máximo que espera quien guarda a que se cierre el segmento.
const SAVE_TIMEOUT: Duration = Duration::from_secs(8);
/// Si una petición de corte lleva esto sin atenderse, el grabador se reinicia.
const CUT_TIMEOUT: Duration = Duration::from_secs(5);
/// Perro guardián: sin avanzar este tiempo grabando (o arrancando), se reinicia.
const STALL_RECORDING: Duration = Duration::from_secs(5);
/// (Arrancar con la CPU llena puede tardar: crear el primer codificador NVENC
/// ha llegado a 18 s con todos los núcleos ocupados.)
const STALL_STARTING: Duration = Duration::from_secs(60);
/// Operaciones más lentas que esto quedan en el registro.
const SLOW: Duration = Duration::from_millis(500);

pub const STARTING: &str = "La grabación aún está arrancando; prueba en unos segundos.";
pub const RESTARTING: &str = "La grabación se está reiniciando; prueba en unos segundos.";
const NOT_RECORDING: &str = "Los clips no están grabando ahora mismo.";
const NO_ANSWER: &str = "La grabación no responde; se reiniciará sola. Prueba en unos segundos.";

#[derive(Clone)]
pub struct Config {
    pub fps: u32,
    pub quality: String,
    pub system_audio: bool,
    pub microphone: bool,
    pub segment_secs: u32,
    pub dir: PathBuf,
    /// Monitor que se graba (id de `monitors::list`); vacío = el principal.
    pub monitor: String,
    /// Solo pruebas: captura en FP16 y pasa por el mapeo de tonos aunque el
    /// monitor sea SDR.
    pub force_hdr_path: bool,
    /// Solo pruebas: en vez de WASAPI, un seno de esta frecuencia en cada pista.
    pub synthetic_audio: Option<f64>,
    /// Dispositivos elegidos (ids de WASAPI); vacío = el predeterminado de Windows.
    pub system_audio_device: String,
    pub microphone_device: String,
}

/// Estado que lee la interfaz.
#[derive(Clone, Default)]
pub struct Info {
    /// "starting" | "recording" | "error" | "off"
    pub state: &'static str,
    pub error: Option<String>,
    pub encoder: Option<String>,
    /// Desde cuándo graba la época actual (lo que hay en el búfer).
    pub since: Option<Instant>,
    pub microphone: Option<Arc<AtomicBool>>,
    /// El monitor tiene HDR y se convierte a SDR.
    pub hdr: bool,
    /// El monitor elegido no está conectado y se graba el principal.
    pub fallback: bool,
    /// Nombre del monitor que se graba.
    pub monitor: String,
    /// El micrófono / la salida elegidos no están conectados y se usa el predeterminado.
    pub microphone_fallback: Option<Arc<AtomicBool>>,
    pub system_fallback: Option<Arc<AtomicBool>>,
    /// Id de WASAPI del micrófono abierto ahora mismo.
    #[cfg_attr(not(test), allow(dead_code))]
    pub microphone_device: Option<Arc<Mutex<String>>>,
}

/// Fase del grabador. Atómica: quien guarda la consulta sin esperar a nadie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Phase {
    Starting = 0,
    Recording = 1,
    Restarting = 2,
    Error = 3,
    Stopped = 4,
}

impl Phase {
    fn from(v: u8) -> Self {
        match v {
            1 => Phase::Recording,
            2 => Phase::Restarting,
            3 => Phase::Error,
            4 => Phase::Stopped,
            _ => Phase::Starting,
        }
    }
}

/// Qué estaba haciendo el grabador la última vez que dio señales de vida.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Step {
    Setup = 0,
    Gpu,
    Monitor,
    CaptureStart,
    Encoders,
    FirstWriter,
    AudioStart,
    Wait,
    Requests,
    Rotate,
    Capture,
    ToneMap,
    Convert,
    EncodeVideo,
    Audio,
    Finalize,
    Checks,
    Teardown,
}

impl Step {
    fn from(v: u8) -> Self {
        use Step::*;
        [Setup, Gpu, Monitor, CaptureStart, Encoders, FirstWriter, AudioStart, Wait, Requests, Rotate, Capture, ToneMap, Convert, EncodeVideo, Audio, Finalize, Checks, Teardown]
            .get(v as usize)
            .copied()
            .unwrap_or(Setup)
    }
}

/// Milisegundos desde el primer uso (reloj monótono para los latidos).
fn now_ms() -> u64 {
    static BASE: OnceLock<Instant> = OnceLock::new();
    BASE.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Señales de vida del grabador (todo atómico: se lee sin bloquear a nadie).
#[derive(Default)]
pub struct Health {
    phase: AtomicU8,
    step: AtomicU8,
    beat: AtomicU64,
    /// Último fotograma producido y cuándo (ms).
    frame: AtomicI64,
    frame_at: AtomicU64,
    /// Peticiones de corte que el bucle tiene pendientes.
    waiters: AtomicUsize,
    /// Generación viva: si el perro guardián abandona un hilo colgado, este
    /// lo ve al despertar y se va sin tocar nada.
    generation: AtomicU64,
}

impl Health {
    pub fn phase(&self) -> Phase {
        Phase::from(self.phase.load(Ordering::SeqCst))
    }

    fn beat(&self, step: Step) {
        self.step.store(step as u8, Ordering::Relaxed);
        self.beat.store(now_ms(), Ordering::Relaxed);
    }

    fn idle(&self) -> Duration {
        Duration::from_millis(now_ms().saturating_sub(self.beat.load(Ordering::Relaxed)))
    }

    fn step(&self) -> Step {
        Step::from(self.step.load(Ordering::Relaxed))
    }
}

pub struct Shared {
    pub ring: Mutex<Ring>,
    pub info: Mutex<Info>,
    pub health: Health,
    /// Peticiones de corte (las recoge el bucle de grabación en cada fotograma).
    requests: Mutex<Vec<Sender<Result<(), String>>>>,
}

impl Shared {
    pub fn new(retention: Duration) -> Self {
        Self {
            ring: Mutex::new(Ring::new(retention)),
            info: Mutex::new(Info { state: "starting", ..Default::default() }),
            health: Health::default(),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn set(&self, f: impl FnOnce(&mut Info)) {
        f(&mut self.info.lock().unwrap());
    }

    /// Cambia de fase. Fuera de `Recording`, contesta ya a lo que hubiera en cola.
    fn set_phase(&self, p: Phase) {
        let old = self.health.phase();
        self.health.phase.store(p as u8, Ordering::SeqCst);
        self.health.beat.store(now_ms(), Ordering::Relaxed);
        if old != p {
            log_info!(AREA, "fase: {old:?} → {p:?}");
        }
        if p != Phase::Recording {
            let msg = self.phase_message(p);
            for tx in self.requests.lock().unwrap().drain(..) {
                let _ = tx.send(Err(msg.clone()));
            }
        }
    }

    fn phase_message(&self, p: Phase) -> String {
        match p {
            Phase::Recording => String::new(),
            Phase::Starting => STARTING.into(),
            Phase::Restarting => RESTARTING.into(),
            Phase::Error => self.info.lock().unwrap().error.clone().unwrap_or_else(|| RESTARTING.into()),
            Phase::Stopped => NOT_RECORDING.into(),
        }
    }

    /// Ok si se puede guardar ahora; si no, el mensaje para el usuario.
    fn ready(&self) -> Result<(), String> {
        match self.health.phase() {
            Phase::Recording => Ok(()),
            p => Err(self.phase_message(p)),
        }
    }

    fn take_requests(&self) -> Vec<Sender<Result<(), String>>> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }

    /// Foto del estado para el registro cuando algo no responde.
    fn snapshot(&self) -> String {
        let h = &self.health;
        let frame_ago = now_ms().saturating_sub(h.frame_at.load(Ordering::Relaxed));
        let segments = self.ring.try_lock().map_or_else(|_| "?".into(), |r| r.len().to_string());
        format!(
            "fase {:?}, paso {:?} (sin señales desde hace {} ms), último fotograma {} hace {frame_ago} ms, peticiones en el bucle {}, en cola {}, segmentos en el búfer {segments}",
            h.phase(),
            h.step(),
            h.idle().as_millis(),
            h.frame.load(Ordering::Relaxed),
            h.waiters.load(Ordering::Relaxed),
            self.requests.try_lock().map_or(0, |r| r.len()),
        )
    }
}

pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// Para pedirle cosas al grabador desde otros hilos.
#[derive(Clone)]
pub struct Handle {
    pub shared: Arc<Shared>,
}

impl Handle {
    /// Cierra el segmento en curso (la grabación sigue en uno nuevo) y une
    /// los que cubren los últimos `keep_secs` en `out`. Nunca espera más de
    /// `SAVE_TIMEOUT` y, si no se está grabando, contesta al momento.
    pub fn save(&self, keep_secs: u32, out: &Path) -> Result<remux::Clip, String> {
        let t0 = Instant::now();
        let _boost = super::boost_above_normal();
        log_info!(AREA, "guardar: petición de {keep_secs} s");
        let r = self.save_inner(keep_secs, out, t0);
        match &r {
            Ok(c) => log_info!(AREA, "guardar: listo en {} ms ({:.1} s de clip)", t0.elapsed().as_millis(), c.duration as f64 / 1e7),
            Err(e) => log_warn!(AREA, "guardar: no se ha guardado tras {} ms: {e}", t0.elapsed().as_millis()),
        }
        r
    }

    fn save_inner(&self, keep_secs: u32, out: &Path, t0: Instant) -> Result<remux::Clip, String> {
        let shared = &self.shared;
        shared.ready()?;
        let (tx, rx) = channel();
        shared.requests.lock().unwrap().push(tx);
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(r) => {
                    r?;
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Si el grabador deja de grabar mientras tanto, se contesta ya.
                    shared.ready()?;
                    if t0.elapsed() > SAVE_TIMEOUT {
                        log_error!(AREA, "guardar: sin respuesta en {SAVE_TIMEOUT:?}; {}", shared.snapshot());
                        return Err(NO_ANSWER.into());
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return Err(shared.ready().err().unwrap_or_else(|| RESTARTING.into())),
            }
        }
        let cut_ms = t0.elapsed().as_millis();
        let segments = shared.ring.lock().unwrap().pin();
        let t1 = Instant::now();
        let r = match ring::select(&segments, keep_secs as i64 * HNS) {
            Some((list, cut)) => remux::remux(&list, cut, out),
            None => Err("Aún no hay nada grabado.".into()),
        };
        shared.ring.lock().unwrap().unpin();
        log_info!(AREA, "guardar: corte en {cut_ms} ms, unión en {} ms", t1.elapsed().as_millis());
        r
    }
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    shared: Arc<Shared>,
    supervisor: Arc<Mutex<Option<JoinHandle<()>>>>,
    watchdog: Option<JoinHandle<()>>,
}

static EPOCH: AtomicU64 = AtomicU64::new(0);

fn spawn_supervisor(cfg: Config, stop: Arc<AtomicBool>, shared: Arc<Shared>, notify: Notify, generation: u64) -> Option<JoinHandle<()>> {
    std::thread::Builder::new()
        .name(format!("clips-{generation}"))
        .spawn(move || supervise(cfg, stop, shared, notify, generation))
        .ok()
}

impl Recorder {
    pub fn start(cfg: Config, shared: Arc<Shared>, notify: Notify) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let generation = shared.health.generation.fetch_add(1, Ordering::SeqCst) + 1;
        shared.set_phase(Phase::Starting);
        let supervisor = Arc::new(Mutex::new(spawn_supervisor(cfg.clone(), stop.clone(), shared.clone(), notify.clone(), generation)));
        let (s, sh, sup) = (stop.clone(), shared.clone(), supervisor.clone());
        let watchdog = std::thread::Builder::new()
            .name("clips-vigilante".into())
            .spawn(move || watchdog(cfg, s, sh, notify, sup))
            .ok();
        Self { stop, shared, supervisor, watchdog }
    }

    pub fn handle(&self) -> Handle {
        Handle { shared: self.shared.clone() }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.watchdog.take() {
            join_bounded(t, Duration::from_secs(2), "el vigilante de clips");
        }
        if let Some(t) = self.supervisor.lock().unwrap().take() {
            join_bounded(t, Duration::from_secs(5), "el grabador de clips");
        }
        self.shared.set_phase(Phase::Stopped);
    }
}

/// Perro guardián: si el grabador deja de dar señales de vida, abandona ese
/// hilo (seguirá colgado, pero ya no toca nada) y arranca otro.
fn watchdog(cfg: Config, stop: Arc<AtomicBool>, shared: Arc<Shared>, notify: Notify, supervisor: Arc<Mutex<Option<JoinHandle<()>>>>) {
    let mut abandoned = 0u32;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        let h = &shared.health;
        let limit = match h.phase() {
            Phase::Recording => STALL_RECORDING,
            Phase::Starting => STALL_STARTING,
            _ => continue,
        };
        let idle = h.idle();
        if idle < limit || stop.load(Ordering::SeqCst) {
            continue;
        }
        abandoned += 1;
        log_error!(AREA, "el grabador no avanza desde hace {} ms; se reinicia (hilos abandonados: {abandoned}). {}", idle.as_millis(), shared.snapshot());
        let generation = h.generation.fetch_add(1, Ordering::SeqCst) + 1;
        if abandoned > 3 {
            // Algo del sistema está colgado de verdad: no seguir acumulando hilos.
            let msg = "La grabación se ha bloqueado varias veces. Desactiva y activa los clips o reinicia Winkit.".to_string();
            shared.set(|i| *i = Info { state: "error", error: Some(msg.clone()), ..Default::default() });
            shared.set_phase(Phase::Error);
            notify();
            return;
        }
        shared.set(|i| *i = Info { state: "starting", ..Default::default() });
        shared.set_phase(Phase::Restarting);
        notify();
        // El hilo colgado queda suelto (no se puede matar); se suelta su JoinHandle.
        let new = spawn_supervisor(cfg.clone(), stop.clone(), shared.clone(), notify.clone(), generation);
        *supervisor.lock().unwrap() = new;
    }
}

enum Stop {
    /// Algo ha cambiado (resolución, monitor…): volver a empezar ya.
    Restart(String),
    /// Error que se enseña; se reintenta con espera creciente.
    Fail(String),
}

fn fail(ctx: &'static str) -> impl FnOnce(windows::core::Error) -> Stop {
    move |e| Stop::Fail(format!("{ctx} ({}).", e.message().trim_end_matches(['.', '\r', '\n'])))
}

fn restart(ctx: &'static str) -> impl FnOnce(windows::core::Error) -> Stop {
    move |e| Stop::Restart(format!("{ctx}: {}", e.message()))
}

fn supervise(cfg: Config, stop: Arc<AtomicBool>, shared: Arc<Shared>, notify: Notify, generation: u64) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        // Las colas de trabajo de Media Foundation (escritores, codificadores)
        // también con prioridad multimedia: con la CPU llena no se quedan atrás.
        let mut task = 0u32;
        if let Err(e) = MFRegisterPlatformWithMMCSS(&windows::core::HSTRING::from("Capture"), &mut task, 0) {
            log_warn!(AREA, "sin MMCSS para Media Foundation: {}", e.message());
        }
    }
    let _boost = super::boost_mmcss("Capture");
    let alive =|| !stop.load(Ordering::SeqCst) && shared.health.generation.load(Ordering::SeqCst) == generation;
    let mut backoff = 1u64;
    while alive() {
        let epoch = EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
        shared.set(|i| *i = Info { state: "starting", ..Default::default() });
        shared.set_phase(Phase::Starting);
        notify();
        log_info!(AREA, "arrancando (época {epoch}, {} fps, calidad {}, audio sistema {}, micrófono {})", cfg.fps, cfg.quality, cfg.system_audio, cfg.microphone);
        let r = run(&cfg, epoch, generation, &stop, &shared, &notify);
        if !alive() {
            break; // parado o abandonado por el perro guardián
        }
        match r {
            Ok(()) => break,
            Err(Stop::Restart(why)) => {
                log_warn!(AREA, "reinicio: {why}");
                backoff = 1;
                shared.set_phase(Phase::Restarting);
                notify();
                pause(&alive, Duration::from_millis(500));
            }
            Err(Stop::Fail(msg)) => {
                log_error!(AREA, "error: {msg} (reintento en {backoff} s)");
                shared.set(|i| *i = Info { state: "error", error: Some(msg.clone()), ..Default::default() });
                shared.set_phase(Phase::Error);
                notify();
                pause(&alive, Duration::from_secs(backoff));
                backoff = (backoff * 2).min(30);
            }
        }
    }
    if shared.health.generation.load(Ordering::SeqCst) == generation {
        shared.set(|i| *i = Info { state: "off", ..Default::default() });
        if stop.load(Ordering::SeqCst) {
            shared.set_phase(Phase::Stopped);
        }
    } else {
        log_warn!(AREA, "un grabador abandonado (generación {generation}) ha terminado por fin");
    }
    unsafe {
        if shared.health.generation.load(Ordering::SeqCst) == generation {
            let _ = MFUnregisterPlatformFromMMCSS();
        }
        let _ = MFShutdown();
        CoUninitialize();
    }
}

/// Espera interrumpible.
fn pause(alive: &impl Fn() -> bool, d: Duration) {
    let until = Instant::now() + d;
    while alive() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn segment_path(dir: &Path, epoch: u64, index: u32) -> PathBuf {
    dir.join(format!("seg-{epoch}-{index:05}.mp4"))
}

/// Segmento en curso.
struct Active {
    w: SegmentWriter,
    start_k: i64,
}

/// Segmento ya sustituido que aún recibe el audio que llega con retraso.
struct Closing {
    w: SegmentWriter,
    start_k: i64,
    end_k: i64,
    waiters: Vec<Sender<Result<(), String>>>,
    deadline: Instant,
}

/// Primer múltiplo de `step` en o después de `k`.
fn ceil_to(k: i64, step: i64) -> i64 {
    (k + step - 1).div_euclid(step) * step
}

/// Qué fotograma producir en esta vuelta. `next` es el primero aún no
/// producido y `due` el que toca por reloj (`due ≥ next`; si el bucle va tarde,
/// los de en medio se saltan). Si hay que cortar y en `[next, due]` cae un
/// fotograma alineado con las tramas AAC (múltiplo de `step`), se produce ese
/// —con su marca de tiempo, un poco antes que «ahora»— y se corta ahí. Como
/// las vueltas cubren todos los índices sin dejar huecos, el corte llega
/// siempre en el primer múltiplo tras la petición: como mucho `step`
/// fotogramas (0,53 s) más tarde.
fn plan(next: i64, due: i64, step: i64, want_cut: bool) -> (i64, bool) {
    if want_cut {
        let c = ceil_to(next, step);
        if c <= due {
            return (c, true);
        }
    }
    (due, false)
}

/// Mide cada paso del bucle: latido para el perro guardián y registro de los
/// lentos (como mucho una línea cada 10 s).
struct Pace<'a> {
    health: &'a Health,
    /// Un hilo abandonado por el perro guardián que despierta no debe tocar
    /// las señales de vida del que lo sustituye.
    generation: u64,
    step: Step,
    since: Instant,
    last_log: Option<Instant>,
    suppressed: u32,
}

impl<'a> Pace<'a> {
    fn new(health: &'a Health, generation: u64) -> Self {
        Self { health, generation, step: Step::Setup, since: Instant::now(), last_log: None, suppressed: 0 }
    }

    fn step(&mut self, step: Step) {
        let took = self.since.elapsed();
        if took > SLOW {
            if self.last_log.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
                let extra = if self.suppressed > 0 { format!(" (y {} más en silencio)", self.suppressed) } else { String::new() };
                log_warn!(AREA, "paso lento: {:?} tardó {} ms{extra}", self.step, took.as_millis());
                self.last_log = Some(Instant::now());
                self.suppressed = 0;
            } else {
                self.suppressed += 1;
            }
        }
        self.step = step;
        self.since = Instant::now();
        if self.health.generation.load(Ordering::SeqCst) == self.generation {
            self.health.beat(step);
        }
    }
}

fn run(cfg: &Config, epoch: u64, generation: u64, stop: &AtomicBool, shared: &Arc<Shared>, notify: &Notify) -> Result<(), Stop> {
    let health = &shared.health;
    let mut pace = Pace::new(health, generation);
    let started = Instant::now();
    pace.step(Step::Gpu);
    let gpu = capture::create_gpu().map_err(fail("No se ha podido iniciar Direct3D 11"))?;
    pace.step(Step::Monitor);
    let (display, fallback) = monitors::resolve(&cfg.monitor).ok_or_else(|| Stop::Fail("No hay ningún monitor conectado.".into()))?;
    let hdr = monitors::is_hdr(display.hmonitor);
    // Con HDR la captura es FP16 y pasa por el mapeo de tonos; si no, BGRA de 8 bits.
    let fp16 = hdr || cfg.force_hdr_path;
    pace.step(Step::CaptureStart);
    let cap = Capture::start(&gpu, display.hmonitor, cfg.fps, fp16).map_err(fail("No se puede capturar la pantalla"))?;
    // NV12 necesita dimensiones pares.
    let (w, h) = (cap.width & !1, cap.height & !1);
    let conv = Converter::new(&gpu, cap.width, cap.height, w, h, cfg.fps, POOL).map_err(fail("No se puede convertir el vídeo en la GPU"))?;
    let tone = if fp16 {
        let white = if hdr { display.sdr_white } else { 1.0 };
        Some(ToneMapper::new(&gpu, cap.width, cap.height, &conv.source, white).map_err(fail("No se puede convertir el HDR"))?)
    } else {
        None
    };
    let mut sdr_white = display.sdr_white;
    let manager = unsafe {
        let mut token = 0u32;
        let mut m: Option<IMFDXGIDeviceManager> = None;
        MFCreateDXGIDeviceManager(&mut token, &mut m).map_err(fail("Media Foundation no está disponible"))?;
        let m = m.unwrap();
        m.ResetDevice(&gpu.device, token).map_err(fail("Media Foundation no acepta la GPU"))?;
        m
    };
    pace.step(Step::Encoders);
    let kinds: Vec<bool> = [(cfg.system_audio, true), (cfg.microphone, false)].iter().filter(|(on, _)| *on).map(|&(_, lb)| lb).collect();
    // Un codificador AAC por pista para toda la sesión (ver `AacEncoder`).
    let mut aac = kinds
        .iter()
        .map(|&lb| AacEncoder::new(if lb { encoder::AAC_SYSTEM } else { encoder::AAC_MIC }))
        .collect::<windows::core::Result<Vec<_>>>()
        .map_err(fail("No se puede iniciar el codificador de audio AAC"))?;
    let enc = EncoderConfig {
        width: w,
        height: h,
        fps: cfg.fps,
        bitrate: encoder::bitrate(&cfg.quality, w, h, cfg.fps),
        gop: encoder::aligned_gop(cfg.fps),
        audio: aac.iter().map(|e| e.output.clone()).collect(),
    };
    pace.step(Step::FirstWriter);
    let first = encoder::create(&segment_path(&cfg.dir, epoch, 0), &enc, &manager).map_err(Stop::Fail)?;
    let io = Io::spawn(manager.clone(), enc.clone(), shared.clone(), epoch, cfg.fps);

    pace.step(Step::AudioStart);
    let start100 = qpc100();
    let (atx, arx) = channel::<Chunk>();
    let sources: Vec<AudioSource> = kinds
        .iter()
        .enumerate()
        .map(|(i, &lb)| match cfg.synthetic_audio {
            Some(freq) => AudioSource::sine(i, freq, start100, atx.clone()),
            None => AudioSource::spawn(i, lb, if lb { cfg.system_audio_device.clone() } else { cfg.microphone_device.clone() }, start100, atx.clone()),
        })
        .collect();
    drop(atx);
    let mic = kinds.iter().position(|lb| !lb).map(|i| sources[i].available.clone());
    let mic_fallback = kinds.iter().position(|lb| !lb).map(|i| sources[i].fallback.clone());
    let system_fallback = kinds.iter().position(|lb| *lb).map(|i| sources[i].fallback.clone());
    let mic_device = kinds.iter().position(|lb| !lb).map(|i| sources[i].device.clone());
    let encoder_name = first.encoder.clone();
    log_info!(
        AREA,
        "grabando en {} ms: monitor «{}»{} {w}×{h} a {} fps, {} kbps, codificador {encoder_name}{}",
        started.elapsed().as_millis(),
        display.name,
        if fallback { " (el elegido no está conectado)" } else { "" },
        cfg.fps,
        enc.bitrate / 1000,
        if hdr { ", HDR → SDR" } else if fp16 { ", ruta FP16 forzada" } else { "" },
    );
    shared.set(|i| {
        *i = Info {
            state: "recording",
            encoder: Some(encoder_name),
            since: Some(Instant::now()),
            microphone: mic,
            error: None,
            hdr,
            fallback,
            monitor: display.name.clone(),
            microphone_fallback: mic_fallback,
            system_fallback,
            microphone_device: mic_device,
        }
    });
    shared.set_phase(Phase::Recording);
    notify();

    let fps = cfg.fps as i64;
    let frame_time = |k: i64| k * HNS / fps;
    let frame_sample = |k: i64| (k * SAMPLE_RATE as i64 / fps) as u64;
    let seg_frames = cfg.segment_secs.max(2) as i64 * fps;
    // Los cortes solo en fotogramas alineados con las tramas AAC (cada 0,53 s).
    let step = encoder::aac_aligned_step(cfg.fps) as i64;
    let timer = Timer::new();

    let mut cur = Active { w: first, start_k: 0 };
    let mut closing: Option<Closing> = None;
    let mut next: Option<Receiver<Result<SegmentWriter, String>>> = None;
    let mut ready: Option<SegmentWriter> = None;
    let mut seg_index = 0u32;
    let mut waiters: Vec<Sender<Result<(), String>>> = Vec::new();
    let mut waiting_since: Option<Instant> = None;
    let mut received = vec![0u64; kinds.len()];
    let mut encoded = vec![0i64; kinds.len()];
    // Siguiente fotograma por producir.
    let mut k: i64 = 0;
    let mut last_status = Instant::now();
    let mut last_check = Instant::now();
    // Resumen cada minuto de los fotogramas saltados (bucle que va tarde).
    let (mut skipped, mut produced, mut stats_since) = (0i64, 0i64, Instant::now());

    let mut body = || -> Result<(), Stop> {
        while !stop.load(Ordering::SeqCst) {
            #[cfg(test)]
            {
                let stall = LOOP_STALL.lock().unwrap().take();
                if let Some(d) = stall {
                    std::thread::sleep(d); // simula un bucle bloqueado (GPU saturada, driver…)
                }
            }
            // Abandonado por el perro guardián (se quedó colgado y ya hay otro): irse
            // sin tocar nada compartido, sobre todo sin quitarle peticiones al nuevo.
            if health.generation.load(Ordering::SeqCst) != generation {
                return Ok(());
            }
            pace.step(Step::Requests);
            let fresh = shared.take_requests();
            if !fresh.is_empty() {
                waiting_since.get_or_insert_with(Instant::now);
                waiters.extend(fresh);
            }
            health.waiters.store(waiters.len(), Ordering::Relaxed);
            if waiting_since.is_some_and(|t| t.elapsed() > CUT_TIMEOUT) {
                log_error!(
                    AREA,
                    "una petición de corte lleva {CUT_TIMEOUT:?} sin atenderse (escritor nuevo {}); {}",
                    if ready.is_some() { "listo" } else if next.is_some() { "en preparación" } else { "sin pedir" },
                    shared.snapshot()
                );
                return Err(Stop::Restart("el corte no llega".into()));
            }

            // Ritmo constante: el fotograma k sale en k/fps s. Si vamos tarde se saltan.
            pace.step(Step::Wait);
            let now = qpc100() - start100;
            let due = now * fps / HNS;
            if due < k {
                timer.sleep(frame_time(k) - now);
                continue;
            }
            if due - k > fps * 3 {
                // Más de 3 s sin pasar por aquí: el equipo ha estado suspendido o
                // bloqueado. Mejor empezar de cero que dejar un hueco enorme.
                return Err(Stop::Restart(format!("salto de {} s (fotograma {k} → {due})", (due - k) / fps)));
            }

            // Cambio de segmento: el escritor nuevo se prepara en el hilo de E/S
            // y se cambia en cuanto está listo, sin perder fotogramas.
            pace.step(Step::Rotate);
            let length = due - cur.start_k;
            let want_cut = !waiters.is_empty() || length >= seg_frames;
            // Siempre hay un escritor de repuesto preparado: crearlo tarda de 0,3 s
            // a 3 s según la carga, y guardar no debe esperarlo.
            if next.is_none() && ready.is_none() {
                seg_index += 1;
                next = Some(io.create(segment_path(&cfg.dir, epoch, seg_index)));
            }
            if let Some(rx) = &next {
                match rx.try_recv() {
                    Ok(Ok(w)) => {
                        ready = Some(w);
                        next = None;
                    }
                    Ok(Err(e)) => return Err(Stop::Fail(e)),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => return Err(Stop::Fail("El hilo de escritura se ha detenido.".into())),
                }
            }
            let (kk, cut) = plan(k, due, step, want_cut && ready.is_some());
            skipped += kk - k;
            produced += 1;
            let first_free = k;
            k = kk;
            if cut {
                if k > first_free {
                    // Fotogramas saltados justo antes del corte (bucle parado): el
                    // segmento que se cierra tiene que llegar hasta el corte, o su
                    // vídeo acabaría antes que su audio. Se repite la imagen actual.
                    pace.step(Step::EncodeVideo);
                    if let Some(idx) = conv.convert().map_err(restart("procesador de vídeo"))? {
                        let (t, d) = (frame_time(first_free - cur.start_k), frame_time(k) - frame_time(first_free));
                        encoder::write_video(&cur.w, &conv.targets[idx], idx, &conv.free, t, d).map_err(fail("Error del codificador de vídeo"))?;
                    }
                }
                if let Some(c) = closing.take() {
                    io.finalize(c);
                }
                let w = ready.take().unwrap();
                let old = std::mem::replace(&mut cur, Active { w, start_k: k });
                if !waiters.is_empty() {
                    log_info!(AREA, "corte para guardar en el fotograma {k} ({} ms después de la petición)", waiting_since.map_or(0, |t| t.elapsed().as_millis()));
                }
                closing = Some(Closing {
                    w: old.w,
                    start_k: old.start_k,
                    end_k: k,
                    waiters: std::mem::take(&mut waiters),
                    deadline: Instant::now() + Duration::from_secs(1),
                });
                waiting_since = None;
                health.waiters.store(0, Ordering::Relaxed);
            }

            // Fotograma: el último que haya entregado Windows (o el anterior, repetido).
            // En HDR se copia a la textura FP16 y el mapeo de tonos escribe la BGRA.
            pace.step(Step::Capture);
            let dst = tone.as_ref().map_or(&conv.source, |t| &t.input);
            if let Some((fw, fh)) = cap.grab_latest(&gpu, dst).map_err(restart("captura"))? {
                if (fw, fh) != (cap.width, cap.height) {
                    return Err(Stop::Restart(format!("resolución {fw}×{fh}")));
                }
                if let Some(t) = &tone {
                    pace.step(Step::ToneMap);
                    t.render(&gpu);
                }
            }
            if cap.closed() {
                return Err(Stop::Restart("captura cerrada (monitor desconectado o apagado)".into()));
            }
            pace.step(Step::Convert);
            if let Some(idx) = conv.convert().map_err(restart("procesador de vídeo"))? {
                pace.step(Step::EncodeVideo);
                let (t, d) = (frame_time(k - cur.start_k), frame_time(k + 1) - frame_time(k));
                encoder::write_video(&cur.w, &conv.targets[idx], idx, &conv.free, t, d).map_err(fail("Error del codificador de vídeo"))?;
            }
            if health.generation.load(Ordering::Relaxed) == generation {
                health.frame.store(k, Ordering::Relaxed);
                health.frame_at.store(now_ms(), Ordering::Relaxed);
            }

            // Audio: cada pista pasa por su codificador AAC continuo y cada trama
            // va al segmento en el que empieza. `received` es hasta dónde ha
            // entrado PCM en cada pista: lo que llegue por detrás se recorta.
            pace.step(Step::Audio);
            while let Ok(mut c) = arx.try_recv() {
                let t = c.track;
                let Some(skip) = overlap(received[t], c.start, c.data.len() as u64 / 2) else { continue };
                c.data.drain(..skip as usize * 2);
                c.start += skip;
                if c.start > received[t] {
                    // No debería pasar (las fuentes son contiguas), pero el AAC no admite huecos.
                    let gap = vec![0i16; (c.start - received[t]) as usize * 2];
                    let frames = aac[t].push(&gap).map_err(fail("Error del codificador de audio"))?;
                    route_frames(frames, t, &cur, closing.as_ref(), frame_sample, &mut encoded[t]).map_err(fail("Error del codificador de audio"))?;
                }
                received[t] = c.start + c.data.len() as u64 / 2;
                let frames = aac[t].push(&c.data).map_err(fail("Error del codificador de audio"))?;
                route_frames(frames, t, &cur, closing.as_ref(), frame_sample, &mut encoded[t]).map_err(fail("Error del codificador de audio"))?;
            }
            // Si un hilo de audio se queda bloqueado (WASAPI puede tardar segundos
            // al cambiar de dispositivo), el silencio se pone aquí para no desfasar.
            let now_sample = ((qpc100() - start100).max(0) as i128 * SAMPLE_RATE as i128 / HNS as i128) as u64;
            for t in 0..received.len() {
                if let Some(n) = lagging(received[t], now_sample) {
                    received[t] += n;
                    let frames = aac[t].push(&vec![0i16; n as usize * 2]).map_err(fail("Error del codificador de audio"))?;
                    route_frames(frames, t, &cur, closing.as_ref(), frame_sample, &mut encoded[t]).map_err(fail("Error del codificador de audio"))?;
                }
            }
            pace.step(Step::Finalize);
            if let Some(c) = &closing {
                let boundary = frame_sample(c.end_k) as i64;
                let complete = encoded.iter().all(|&e| e >= boundary);
                if complete || Instant::now() > c.deadline {
                    if !complete {
                        log_warn!(AREA, "segmento cerrado sin todo su audio (límite {boundary}, codificado {encoded:?})");
                    }
                    io.finalize(closing.take().unwrap());
                }
            }

            if last_check.elapsed() >= Duration::from_secs(2) {
                pace.step(Step::Checks);
                last_check = Instant::now();
                // ¿Sigue siendo el mismo monitor? (Se ha conectado el elegido, ha
                // cambiado el principal, se ha activado o desactivado el HDR…)
                match monitors::resolve(&cfg.monitor) {
                    Some((d, fb)) if d.hmonitor == cap.monitor && fb == fallback => {
                        if monitors::is_hdr(d.hmonitor) != hdr {
                            return Err(Stop::Restart("HDR activado o desactivado".into()));
                        }
                        if let Some(t) = tone.as_ref().filter(|_| hdr && (d.sdr_white - sdr_white).abs() > 0.01) {
                            sdr_white = d.sdr_white;
                            t.set_white(&gpu, sdr_white);
                        }
                    }
                    _ => return Err(Stop::Restart("monitor cambiado".into())),
                }
                unsafe { gpu.device.GetDeviceRemovedReason() }.map_err(restart("GPU reiniciada"))?;
            }
            if stats_since.elapsed() >= Duration::from_secs(60) {
                if skipped * 100 > (skipped + produced) {
                    log_info!(AREA, "el bucle va tarde: {skipped} fotogramas saltados y {produced} producidos en el último minuto");
                }
                (skipped, produced, stats_since) = (0, 0, Instant::now());
            }
            if last_status.elapsed() >= Duration::from_secs(5) {
                last_status = Instant::now();
                notify();
            }
            k += 1;
        }
        Ok(())
    };
    let r = body();
    drop(body);
    // Fuera de «grabando» antes de soltar nada: quien quiera guardar ahora
    // recibe la respuesta al momento aunque cerrar los hilos tarde.
    if health.generation.load(Ordering::SeqCst) == generation {
        match &r {
            Ok(()) => shared.set_phase(Phase::Stopped),
            Err(Stop::Restart(_)) => shared.set_phase(Phase::Restarting),
            Err(Stop::Fail(_)) => shared.set_phase(Phase::Error),
        }
    }
    pace.step(Step::Teardown);
    let t = Instant::now();
    // Al parar no se finaliza nada: la carpeta del búfer se vacía después.
    drop(sources);
    drop(io);
    if t.elapsed() > SLOW {
        log_warn!(AREA, "cerrar los hilos ha tardado {} ms", t.elapsed().as_millis());
    }
    r
}

/// Solo pruebas: bloquea el bucle de grabación una vez durante este tiempo.
#[cfg(test)]
pub static LOOP_STALL: Mutex<Option<Duration>> = Mutex::new(None);

/// Muestras que hay que saltar de un bloque `[start, start + len)` porque ya
/// se escribió hasta `received`. None si no aporta nada nuevo.
fn overlap(received: u64, start: u64, len: u64) -> Option<u64> {
    let end = start + len;
    (end > received).then(|| received.saturating_sub(start))
}

/// Si una pista va más de 0,5 s por detrás de «ahora», cuánto silencio poner
/// para dejarla a 0,1 s (margen para los paquetes que aún estén en camino).
fn lagging(received: u64, now: u64) -> Option<u64> {
    let (max_lag, margin) = (SAMPLE_RATE as u64 / 2, SAMPLE_RATE as u64 / 10);
    (now > received + max_lag).then(|| now - margin - received)
}

/// Reparte tramas AAC: cada una va al segmento en el que empieza, con la marca
/// relativa a ese segmento (al unir, las marcas vuelven a ser las originales y
/// el flujo AAC queda exactamente como salió del codificador).
fn route_frames(
    frames: Vec<encoder::AacFrame>,
    track: usize,
    cur: &Active,
    closing: Option<&Closing>,
    sample_of: impl Fn(i64) -> u64,
    encoded: &mut i64,
) -> windows::core::Result<()> {
    let cur_s = sample_of(cur.start_k) as i64;
    for f in frames {
        *encoded = f.start + encoder::AAC_FRAME;
        if f.start >= cur_s {
            encoder::write_aac(&cur.w, track, &f.sample, encoder::frames_to_hns(f.start - cur_s))?;
        } else if let Some(cl) = closing {
            let cl_s = sample_of(cl.start_k) as i64;
            if f.start >= cl_s {
                encoder::write_aac(&cl.w, track, &f.sample, encoder::frames_to_hns(f.start - cl_s))?;
            }
        }
        // Si no, es el «priming» inicial o llega tarde para un segmento ya cerrado.
    }
    Ok(())
}

/// E/S de los segmentos, en dos hilos para que una cosa no espere a la otra:
/// uno crea los escritores de repuesto (con carga tarda hasta 3 s) y otro
/// finaliza los terminados (escribe el índice del MP4), que es lo que espera
/// quien guarda. Con un solo hilo, cerrar el segmento para guardar quedaba
/// detrás de crear el siguiente.
struct Io {
    create_tx: Option<Sender<(PathBuf, Sender<Result<SegmentWriter, String>>)>>,
    finalize_tx: Option<Sender<Closing>>,
    threads: Vec<JoinHandle<()>>,
}

struct SendBox<T>(T);
unsafe impl<T> Send for SendBox<T> {}

/// Un hilo de trabajo con COM y prioridad alta que atiende una cola.
fn worker<J: Send + 'static>(name: &str, mut f: impl FnMut(J) + Send + 'static) -> (Sender<J>, Option<JoinHandle<()>>) {
    let (tx, rx) = channel::<J>();
    let thread = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            let _boost = super::boost_above_normal();
            for job in rx {
                f(job);
            }
            unsafe { CoUninitialize() };
        })
        .ok();
    (tx, thread)
}

impl Io {
    fn spawn(manager: IMFDXGIDeviceManager, enc: EncoderConfig, shared: Arc<Shared>, epoch: u64, fps: u32) -> Self {
        let manager = SendBox(manager);
        let (create_tx, creator) = worker("clips-crear", move |(path, reply): (PathBuf, Sender<Result<SegmentWriter, String>>)| {
            let manager = &manager;
            let t = Instant::now();
            let r = encoder::create(&path, &enc, &manager.0);
            if let Err(e) = &r {
                log_error!(AREA, "no se puede crear el segmento: {e}");
            }
            let _ = reply.send(r);
            if t.elapsed() > SLOW {
                log_warn!(AREA, "crear un segmento ha tardado {} ms", t.elapsed().as_millis());
            }
        });
        let (finalize_tx, finalizer) = worker("clips-cerrar", move |c: Closing| {
            let t = Instant::now();
            let r = unsafe { c.w.writer.Finalize() };
            let path = c.w.path.clone();
            drop(c.w);
            let result = match r {
                Ok(()) => {
                    let fps = fps as i64;
                    let start = c.start_k * HNS / fps;
                    let end = c.end_k * HNS / fps;
                    shared.ring.lock().unwrap().push(Segment { path, epoch, start, duration: end - start, ended: Instant::now() });
                    Ok(())
                }
                Err(e) => {
                    log_error!(AREA, "no se puede cerrar un segmento: {}", e.message());
                    let _ = std::fs::remove_file(&path);
                    Err(format!("No se ha podido cerrar el segmento ({}).", e.message()))
                }
            };
            for w in c.waiters {
                let _ = w.send(result.clone());
            }
            if t.elapsed() > SLOW {
                log_warn!(AREA, "cerrar un segmento ha tardado {} ms", t.elapsed().as_millis());
            }
        });
        Self { create_tx: Some(create_tx), finalize_tx: Some(finalize_tx), threads: creator.into_iter().chain(finalizer).collect() }
    }

    fn create(&self, path: PathBuf) -> Receiver<Result<SegmentWriter, String>> {
        let (tx, rx) = channel();
        if let Some(t) = &self.create_tx {
            let _ = t.send((path, tx));
        }
        rx
    }

    fn finalize(&self, c: Closing) {
        if let Some(t) = &self.finalize_tx {
            let _ = t.send(c);
        }
    }
}

impl Drop for Io {
    fn drop(&mut self) {
        self.create_tx.take();
        self.finalize_tx.take();
        for t in self.threads.drain(..) {
            join_bounded(t, Duration::from_secs(3), "un hilo de escritura de clips");
        }
    }
}

/// Espera de alta resolución (los `sleep` normales van de ~15 ms en 15 ms).
struct Timer(Option<HANDLE>);

impl Timer {
    fn new() -> Self {
        const TIMER_ALL_ACCESS: u32 = 0x1F0003;
        Self(unsafe { CreateWaitableTimerExW(None, None, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS) }.ok())
    }

    fn sleep(&self, hns: i64) {
        let hns = hns.clamp(0, HNS);
        if let Some(h) = self.0 {
            let due = -hns;
            if unsafe { SetWaitableTimer(h, &due, 0, None, None, false) }.is_ok() {
                unsafe { WaitForSingleObject(h, 1000) };
                return;
            }
        }
        std::thread::sleep(Duration::from_nanos(hns as u64 * 100));
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(h) = self.0 {
            unsafe {
                let _ = CloseHandle(h);
            }
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn recorta_el_audio_repetido_o_tardio() {
        assert_eq!(overlap(0, 0, 480), Some(0));
        assert_eq!(overlap(1000, 1000, 480), Some(0));
        assert_eq!(overlap(1000, 800, 480), Some(200)); // solapa 200
        assert_eq!(overlap(1000, 400, 480), None); // todo repetido
        assert_eq!(overlap(1000, 1200, 480), Some(0)); // hueco: lo rellena el hilo de audio
    }

    #[test]
    fn rellena_si_una_pista_se_queda_atras() {
        let r = SAMPLE_RATE as u64;
        assert_eq!(lagging(0, r / 4), None);
        assert_eq!(lagging(10 * r, 10 * r + r / 2), None);
        // Bloqueada 5 s: silencio hasta 0,1 s antes de «ahora».
        assert_eq!(lagging(10 * r, 15 * r), Some(5 * r - r / 10));
    }

    /// Generador pseudoaleatorio sencillo (sin dependencias) para las pruebas.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
    }

    /// Simula el bucle con un reloj que avanza a saltos: devuelve, para cada
    /// petición, en qué fotograma se cortó, y todos los cortes.
    fn simulate(fps: i64, jumps: &mut dyn FnMut() -> i64, requests: &[i64], old_logic: bool) -> (Vec<Option<i64>>, Vec<i64>) {
        let step = encoder::aac_aligned_step(fps as u32) as i64;
        let seg_frames = 15 * fps;
        let (mut k, mut clock, mut start) = (0i64, 0i64, 0i64);
        let mut served: Vec<Option<i64>> = vec![None; requests.len()];
        let mut cuts = Vec::new();
        while clock < 120 * fps {
            clock += jumps();
            let due = clock;
            if due < k {
                continue;
            }
            let pending: Vec<usize> = (0..requests.len()).filter(|&i| served[i].is_none() && requests[i] <= due).collect();
            let want = !pending.is_empty() || due - start >= seg_frames;
            let (kk, cut) = if old_logic {
                (due, due - start > 0 && due % step == 0 && want)
            } else {
                plan(k, due, step, want)
            };
            k = kk;
            if cut {
                cuts.push(k);
                start = k;
                for i in pending {
                    served[i] = Some(k);
                }
            }
            k += 1;
        }
        (served, cuts)
    }

    #[test]
    fn el_corte_llega_siempre_aunque_el_bucle_se_salte_fotogramas() {
        for fps in [30i64, 60] {
            let step = encoder::aac_aligned_step(fps as u32) as i64;
            for seed in 0..200u64 {
                let mut rng = Lcg(seed * 7919 + fps as u64);
                // Saltos de 1 a 4 fotogramas, y de vez en cuando hasta 40 (≈0,7 s).
                let mut jumps = || if rng.next() % 50 == 0 { 1 + (rng.next() % 40) as i64 } else { 1 + (rng.next() % 4) as i64 };
                let requests: Vec<i64> = (1..=10).map(|i| i * 10 * fps + (seed as i64 * 13) % fps).collect();
                let (served, cuts) = simulate(fps, &mut jumps, &requests, false);
                for (req, s) in requests.iter().zip(&served) {
                    let s = s.unwrap_or_else(|| panic!("petición en {req} sin atender (fps {fps}, semilla {seed})"));
                    // Puede caer en un hueco saltado justo antes (el fotograma se marca
                    // ahí), nunca antes de lo ya producido: como mucho un salto (40).
                    assert!(s >= *req - 40, "corte {} fotogramas antes de la petición", req - s);
                    // Como mucho un paso de alineación (0,53 s) más el salto que la reveló.
                    assert!(s - req <= fps, "atendida {} fotogramas tarde (fps {fps}, semilla {seed})", s - req);
                }
                for c in &cuts {
                    assert_eq!(c % step, 0, "corte no alineado con el AAC");
                    assert_eq!(*c * SAMPLE_RATE as i64 / fps % encoder::AAC_FRAME, 0);
                }
                // Los cortes por tiempo siguen llegando: ningún segmento de más de 16 s.
                let mut prev = 0;
                for c in &cuts {
                    assert!(c - prev <= 16 * fps, "segmento de {} s", (c - prev) / fps);
                    prev = *c;
                }
            }
        }
    }

    #[test]
    fn la_logica_anterior_no_cortaba_si_el_bucle_va_a_saltos_de_dos() {
        // Así era antes (`k % paso == 0` con el `k` del reloj): con el bucle a
        // 30 fps reales en una grabación de 60 (saltos de 2 fotogramas) y la
        // paridad equivocada, el múltiplo exacto no toca nunca.
        let mut odd = true;
        let mut jumps = || {
            let j = if odd { 1 } else { 2 };
            odd = false;
            j
        };
        let (served, cuts) = simulate(60, &mut jumps, &[600, 1200], true);
        assert!(served.iter().all(|s| s.is_none()), "{served:?}");
        assert!(cuts.is_empty());
        // Con la lógica nueva, mismo reloj: cortes y peticiones atendidas.
        let mut odd = true;
        let mut jumps = || {
            let j = if odd { 1 } else { 2 };
            odd = false;
            j
        };
        let (served, _) = simulate(60, &mut jumps, &[600, 1200], false);
        assert!(served.iter().all(|s| s.is_some()), "{served:?}");
    }

    #[test]
    fn redondeo_al_multiplo() {
        assert_eq!(ceil_to(0, 32), 0);
        assert_eq!(ceil_to(1, 32), 32);
        assert_eq!(ceil_to(32, 32), 32);
        assert_eq!(ceil_to(33, 32), 64);
        assert_eq!(plan(30, 40, 32, true), (32, true));
        assert_eq!(plan(30, 31, 32, true), (31, false));
        assert_eq!(plan(30, 40, 32, false), (40, false));
    }
}
