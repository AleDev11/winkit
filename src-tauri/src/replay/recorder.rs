//! El grabador: un hilo que captura la pantalla a ritmo constante (30/60 fps),
//! la codifica en segmentos de ~15 s y los va cerrando sin cortes; un hilo
//! de E/S crea y finaliza los escritores (lo lento) para que la captura no se
//! pare nunca, y un hilo por fuente de audio.
//!
//! Si la captura se pierde (cambio de resolución, monitor apagado o
//! desconectado, GPU reiniciada) el grabador vuelve a empezar solo.

use super::audio::{AudioSource, Chunk};
use super::capture::{self, Capture, Converter};
use super::monitors;
use super::tonemap::ToneMapper;
use super::encoder::{self, AacEncoder, EncoderConfig, SegmentWriter, SAMPLE_RATE};
use super::ring::{self, Ring, Segment};
use super::{qpc100, remux};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{CreateWaitableTimerExW, SetWaitableTimer, WaitForSingleObject, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION};

/// Texturas NV12 en vuelo hacia el codificador.
const POOL: usize = 8;
const HNS: i64 = 10_000_000;

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
}

pub struct Shared {
    pub ring: Mutex<Ring>,
    pub info: Mutex<Info>,
}

impl Shared {
    pub fn new(retention: Duration) -> Self {
        Self { ring: Mutex::new(Ring::new(retention)), info: Mutex::new(Info { state: "starting", ..Default::default() }) }
    }

    fn set(&self, f: impl FnOnce(&mut Info)) {
        f(&mut self.info.lock().unwrap());
    }
}

pub type Notify = Arc<dyn Fn() + Send + Sync>;

enum Cmd {
    /// Cerrar el segmento actual; responde cuando ya está en el búfer.
    Rotate(Sender<Result<(), String>>),
}

const NOT_RECORDING: &str = "Los clips no están grabando ahora mismo.";

/// Para pedirle cosas al grabador desde otros hilos.
#[derive(Clone)]
pub struct Handle {
    cmd: Sender<Cmd>,
    pub shared: Arc<Shared>,
}

impl Handle {
    /// Cierra el segmento en curso (la grabación sigue en uno nuevo) y une
    /// los que cubren los últimos `keep_secs` en `out`.
    pub fn save(&self, keep_secs: u32, out: &Path) -> Result<remux::Clip, String> {
        let (tx, rx) = channel();
        self.cmd.send(Cmd::Rotate(tx)).map_err(|_| NOT_RECORDING.to_string())?;
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(r) => r?,
            Err(RecvTimeoutError::Timeout) => return Err("La grabación no responde.".into()),
            Err(RecvTimeoutError::Disconnected) => return Err(NOT_RECORDING.into()),
        }
        let segments = self.shared.ring.lock().unwrap().pin();
        let r = match ring::select(&segments, keep_secs as i64 * HNS) {
            Some((list, cut)) => remux::remux(&list, cut, out),
            None => Err("Aún no hay nada grabado.".into()),
        };
        self.shared.ring.lock().unwrap().unpin();
        r
    }
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    handle: Handle,
}

static EPOCH: AtomicU64 = AtomicU64::new(0);

impl Recorder {
    pub fn start(cfg: Config, shared: Arc<Shared>, notify: Notify) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let (s, sh) = (stop.clone(), shared.clone());
        let thread = std::thread::Builder::new()
            .name("clips".into())
            .spawn(move || supervise(cfg, s, rx, sh, notify))
            .ok();
        Self { stop, thread, handle: Handle { cmd: tx, shared } }
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
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

fn supervise(cfg: Config, stop: Arc<AtomicBool>, cmd: Receiver<Cmd>, shared: Arc<Shared>, notify: Notify) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
    }
    let mut backoff = 1u64;
    while !stop.load(Ordering::SeqCst) {
        let epoch = EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
        shared.set(|i| *i = Info { state: "starting", ..Default::default() });
        notify();
        match run(&cfg, epoch, &stop, &cmd, &shared, &notify) {
            Ok(()) => break,
            Err(Stop::Restart(why)) => {
                eprintln!("clips: reinicio ({why})");
                backoff = 1;
                wait(&stop, &cmd, Duration::from_millis(500), "La grabación se está reiniciando.");
            }
            Err(Stop::Fail(msg)) => {
                eprintln!("clips: {msg}");
                shared.set(|i| *i = Info { state: "error", error: Some(msg.clone()), ..Default::default() });
                notify();
                wait(&stop, &cmd, Duration::from_secs(backoff), &msg);
                backoff = (backoff * 2).min(30);
            }
        }
    }
    shared.set(|i| *i = Info { state: "off", ..Default::default() });
    unsafe {
        let _ = MFShutdown();
        CoUninitialize();
    }
}

/// Espera (interrumpible) contestando con un error a quien quiera guardar.
fn wait(stop: &AtomicBool, cmd: &Receiver<Cmd>, d: Duration, msg: &str) {
    let until = Instant::now() + d;
    while !stop.load(Ordering::SeqCst) && Instant::now() < until {
        while let Ok(Cmd::Rotate(tx)) = cmd.try_recv() {
            let _ = tx.send(Err(msg.to_string()));
        }
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

fn run(cfg: &Config, epoch: u64, stop: &AtomicBool, cmd: &Receiver<Cmd>, shared: &Arc<Shared>, notify: &Notify) -> Result<(), Stop> {
    let gpu = capture::create_gpu().map_err(fail("No se ha podido iniciar Direct3D 11"))?;
    let (display, fallback) = monitors::resolve(&cfg.monitor).ok_or_else(|| Stop::Fail("No hay ningún monitor conectado.".into()))?;
    let hdr = monitors::is_hdr(display.hmonitor);
    // Con HDR la captura es FP16 y pasa por el mapeo de tonos; si no, BGRA de 8 bits.
    let fp16 = hdr || cfg.force_hdr_path;
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
    let first = encoder::create(&segment_path(&cfg.dir, epoch, 0), &enc, &manager).map_err(Stop::Fail)?;
    let io = Io::spawn(manager.clone(), enc.clone(), shared.clone(), epoch, cfg.fps);

    let start100 = qpc100();
    let (atx, arx) = channel::<Chunk>();
    let sources: Vec<AudioSource> = kinds
        .iter()
        .enumerate()
        .map(|(i, &lb)| match cfg.synthetic_audio {
            Some(freq) => AudioSource::sine(i, freq, start100, atx.clone()),
            None => AudioSource::spawn(i, lb, start100, atx.clone()),
        })
        .collect();
    drop(atx);
    let mic = kinds.iter().position(|lb| !lb).map(|i| sources[i].available.clone());
    let encoder_name = first.encoder.clone();
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
        }
    });
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
    let mut seg_index = 0u32;
    let mut waiters: Vec<Sender<Result<(), String>>> = Vec::new();
    let mut received = vec![0u64; kinds.len()];
    let mut encoded = vec![0i64; kinds.len()];
    let mut k: i64 = 0;
    let mut last_status = Instant::now();
    let mut last_check = Instant::now();

    while !stop.load(Ordering::SeqCst) {
        while let Ok(Cmd::Rotate(tx)) = cmd.try_recv() {
            waiters.push(tx);
        }

        // Ritmo constante: el fotograma k sale en k/fps s. Si vamos tarde se saltan.
        let now = qpc100() - start100;
        let due = now * fps / HNS;
        if due < k {
            timer.sleep(frame_time(k) - now);
            continue;
        }
        if due - k > fps * 3 {
            // Más de 3 s sin pasar por aquí: el equipo ha estado suspendido o
            // bloqueado. Mejor empezar de cero que dejar un hueco enorme.
            return Err(Stop::Restart(format!("salto de {} s", (due - k) / fps)));
        }
        k = due;

        // Cambio de segmento: el escritor nuevo se prepara en el hilo de E/S y
        // se cambia en cuanto está listo, sin perder fotogramas.
        let length = k - cur.start_k;
        let rotate = length > 0 && k % step == 0 && (!waiters.is_empty() || length >= seg_frames);
        if next.is_none() && (rotate || length >= seg_frames - fps * 3 / 2) {
            seg_index += 1;
            next = Some(io.create(segment_path(&cfg.dir, epoch, seg_index)));
        }
        if rotate {
            if let Some(rx) = &next {
                match rx.try_recv() {
                    Ok(Ok(w)) => {
                        next = None;
                        if let Some(c) = closing.take() {
                            io.finalize(c);
                        }
                        let old = std::mem::replace(&mut cur, Active { w, start_k: k });
                        closing = Some(Closing {
                            w: old.w,
                            start_k: old.start_k,
                            end_k: k,
                            waiters: std::mem::take(&mut waiters),
                            deadline: Instant::now() + Duration::from_secs(1),
                        });
                    }
                    Ok(Err(e)) => return Err(Stop::Fail(e)),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => return Err(Stop::Fail("El hilo de escritura se ha detenido.".into())),
                }
            }
        }

        // Fotograma: el último que haya entregado Windows (o el anterior, repetido).
        // En HDR se copia a la textura FP16 y el mapeo de tonos escribe la BGRA.
        let dst = tone.as_ref().map_or(&conv.source, |t| &t.input);
        if let Some((fw, fh)) = cap.grab_latest(&gpu, dst).map_err(restart("captura"))? {
            if (fw, fh) != (cap.width, cap.height) {
                return Err(Stop::Restart(format!("resolución {fw}×{fh}")));
            }
            if let Some(t) = &tone {
                t.render(&gpu);
            }
        }
        if cap.closed() {
            return Err(Stop::Restart("captura cerrada".into()));
        }
        if let Some(idx) = conv.convert().map_err(restart("procesador de vídeo"))? {
            let (t, d) = (frame_time(k - cur.start_k), frame_time(k + 1) - frame_time(k));
            encoder::write_video(&cur.w, &conv.targets[idx], idx, &conv.free, t, d).map_err(fail("Error del codificador de vídeo"))?;
        }

        // Audio: cada pista pasa por su codificador AAC continuo y cada trama va
        // al segmento en el que empieza. `received` es hasta dónde ha entrado PCM
        // en cada pista: lo que llegue por detrás (repetido o tarde) se recorta.
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
        // Si un hilo de audio se queda bloqueado (WASAPI puede tardar segundos al
        // cambiar de dispositivo), el silencio se pone aquí para no desfasar.
        let now_sample = ((qpc100() - start100).max(0) as i128 * SAMPLE_RATE as i128 / HNS as i128) as u64;
        for t in 0..received.len() {
            if let Some(n) = lagging(received[t], now_sample) {
                received[t] += n;
                let frames = aac[t].push(&vec![0i16; n as usize * 2]).map_err(fail("Error del codificador de audio"))?;
                route_frames(frames, t, &cur, closing.as_ref(), frame_sample, &mut encoded[t]).map_err(fail("Error del codificador de audio"))?;
            }
        }
        if let Some(c) = &closing {
            let boundary = frame_sample(c.end_k) as i64;
            let complete = encoded.iter().all(|&e| e >= boundary);
            if complete || Instant::now() > c.deadline {
                if !complete {
                    eprintln!("clips: segmento cerrado sin todo su audio (límite {boundary}, codificado {encoded:?})");
                }
                io.finalize(closing.take().unwrap());
            }
        }

        if last_check.elapsed() >= Duration::from_secs(2) {
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
        if last_status.elapsed() >= Duration::from_secs(5) {
            last_status = Instant::now();
            notify();
        }
        k += 1;
    }
    // Al parar no se finaliza nada: la carpeta del búfer se vacía después.
    drop(sources);
    Ok(())
}

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

/// Hilo de E/S: crea los escritores de los segmentos siguientes y finaliza
/// los terminados (escribir el índice del MP4 tarda).
enum Job {
    Create(PathBuf, Sender<Result<SegmentWriter, String>>),
    Finalize(Closing),
}

struct Io {
    tx: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

struct SendBox<T>(T);
unsafe impl<T> Send for SendBox<T> {}

impl Io {
    fn spawn(manager: IMFDXGIDeviceManager, enc: EncoderConfig, shared: Arc<Shared>, epoch: u64, fps: u32) -> Self {
        let (tx, rx) = channel::<Job>();
        let manager = SendBox(manager);
        let thread = std::thread::Builder::new()
            .name("clips-io".into())
            .spawn(move || {
                let manager = manager;
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                for job in rx {
                    match job {
                        Job::Create(path, reply) => {
                            let _ = reply.send(encoder::create(&path, &enc, &manager.0));
                        }
                        Job::Finalize(c) => {
                            let r = unsafe { c.w.writer.Finalize() };
                            let path = c.w.path.clone();
                            drop(c.w);
                            let result = match r {
                                Ok(()) => {
                                    let fps = fps as i64;
                                    let start = c.start_k * HNS / fps;
                                    let end = c.end_k * HNS / fps;
                                    shared.ring.lock().unwrap().push(Segment {
                                        path,
                                        epoch,
                                        start,
                                        duration: end - start,
                                        ended: Instant::now(),
                                    });
                                    Ok(())
                                }
                                Err(e) => {
                                    let _ = std::fs::remove_file(&path);
                                    Err(format!("No se ha podido cerrar el segmento ({}).", e.message()))
                                }
                            };
                            for w in c.waiters {
                                let _ = w.send(result.clone());
                            }
                        }
                    }
                }
                unsafe { CoUninitialize() };
            })
            .ok();
        Self { tx: Some(tx), thread }
    }

    fn create(&self, path: PathBuf) -> Receiver<Result<SegmentWriter, String>> {
        let (tx, rx) = channel();
        if let Some(t) = &self.tx {
            let _ = t.send(Job::Create(path, tx));
        }
        rx
    }

    fn finalize(&self, c: Closing) {
        if let Some(t) = &self.tx {
            let _ = t.send(Job::Finalize(c));
        }
    }
}

impl Drop for Io {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
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
}
