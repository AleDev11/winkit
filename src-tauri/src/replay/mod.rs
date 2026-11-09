//! Clips: la pantalla se graba sin parar en un búfer circular en disco y el
//! atajo (Ctrl+Alt+G) guarda los últimos minutos como un MP4.
//!
//! Interfaz que usa el resto de la app (no cambiarla sin tocar ipc.rs,
//! shortcuts.rs y lib.rs):
//! - `apply(&Clips)`: arranca, reinicia o para la grabación según los ajustes.
//! - `save()`: guarda los últimos `minutes` y devuelve la ruta del MP4.
//! - `status()`: estado actual; los cambios se emiten como `clips:status`.
//! - `resolved_folder(&Clips)`: carpeta real de destino.
//! - `shutdown()`: para y limpia al salir.
//!
//! Cómo funciona, todo nativo y sin procesos externos:
//! - `monitors`: monitores conectados, con un id estable (ruta de dispositivo)
//!   para elegir cuál se graba; si no está, el principal.
//! - `capture`: Windows.Graphics.Capture del monitor (con cursor) y conversión
//!   BGRA → NV12 en la GPU. Con HDR, captura FP16 y `tonemap` lo pasa a SDR.
//! - `encoder`: H.264 con Media Foundation (NVENC/AMF/Quick Sync si los hay); el
//!   vídeo no pasa por la CPU. AAC con un codificador continuo por pista.
//! - `audio`: WASAPI, loopback del sistema y micrófono, como pistas separadas.
//! - `recorder` + `ring`: segmentos de ~15 s en %LOCALAPPDATA%\Winkit\clips-buffer,
//!   cortados en fotogramas alineados con las tramas AAC; se borran los que
//!   quedan fuera de la ventana.
//! - `remux`: al guardar, une los segmentos sin recodificar.

mod audio;
mod capture;
mod encoder;
mod monitors;
mod recorder;
mod remux;
mod tonemap;
mod ring;
#[cfg(test)]
mod tests;

use crate::settings::Clips;
use recorder::{Config, Handle, Recorder, Shared};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Emitter;

/// Duración de cada segmento del búfer.
const SEGMENT_SECS: u32 = 15;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipsStatus {
    /// "off" | "starting" | "recording" | "error"
    pub state: &'static str,
    pub message: Option<String>,
    pub buffered_seconds: u32,
    pub folder: String,
}

/// Carpeta de destino: la de ajustes o, si está vacía, Vídeos\Winkit.
pub fn resolved_folder(c: &Clips) -> PathBuf {
    if !c.folder.trim().is_empty() {
        return PathBuf::from(c.folder.trim());
    }
    let videos = std::env::var("USERPROFILE").map(|p| PathBuf::from(p).join("Videos")).unwrap_or_default();
    videos.join("Winkit")
}

/// Carpeta del búfer circular.
fn buffer_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("Winkit").join("clips-buffer")
}

/// Reloj de la grabación: QueryPerformanceCounter en unidades de 100 ns (la
/// misma base que las marcas QPC que da WASAPI).
pub(crate) fn qpc100() -> i64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut c, mut f) = (0i64, 0i64);
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
        let _ = QueryPerformanceFrequency(&mut f);
    }
    (c as i128 * 10_000_000 / f.max(1) as i128) as i64
}

fn retention(minutes: u32) -> Duration {
    Duration::from_secs(minutes.clamp(1, 30) as u64 * 60 + SEGMENT_SECS as u64)
}

// ------------------------------------------------------------ control

/// Lo que cambia la grabación en sí (los minutos y la carpeta no la reinician).
#[derive(Clone, PartialEq)]
struct Key {
    fps: u32,
    quality: String,
    system_audio: bool,
    microphone: bool,
    monitor: String,
}

impl Key {
    fn of(c: &Clips) -> Self {
        Self {
            fps: if c.fps >= 45 { 60 } else { 30 },
            quality: c.quality.clone(),
            system_audio: c.system_audio,
            microphone: c.microphone,
            monitor: c.monitor.clone(),
        }
    }
}

/// El grabador activo, para `save()` y `status()`.
struct Current {
    handle: Handle,
    minutes: u32,
}

static CURRENT: Mutex<Option<Current>> = Mutex::new(None);

enum Msg {
    Apply(Clips),
    Shutdown(Sender<()>),
}

static CONTROL: Mutex<Option<Sender<Msg>>> = Mutex::new(None);

fn control() -> Sender<Msg> {
    let mut c = CONTROL.lock().unwrap();
    if let Some(tx) = c.as_ref() {
        return tx.clone();
    }
    let (tx, rx) = channel::<Msg>();
    let _ = std::thread::Builder::new().name("clips-control".into()).spawn(move || {
        // Restos de una ejecución anterior.
        ring::wipe(&buffer_dir());
        let mut active: Option<(Recorder, Key)> = None;
        let stop = |active: &mut Option<(Recorder, Key)>| {
            // Sin el cerrojo de CURRENT: el grabador llama a `status()` al parar.
            let current = CURRENT.lock().unwrap().take();
            if let Some((rec, _)) = active.take() {
                drop(rec);
            }
            if let Some(c) = current {
                c.handle.shared.ring.lock().unwrap().clear();
            }
            ring::wipe(&buffer_dir());
        };
        for msg in rx {
            match msg {
                Msg::Apply(c) => {
                    if !c.enabled {
                        stop(&mut active);
                        emit();
                        continue;
                    }
                    let key = Key::of(&c);
                    if active.as_ref().is_some_and(|(_, k)| *k == key) {
                        if let Some(cur) = CURRENT.lock().unwrap().as_mut() {
                            cur.minutes = c.minutes.clamp(1, 30);
                            cur.handle.shared.ring.lock().unwrap().set_retention(retention(c.minutes));
                        }
                        emit();
                        continue;
                    }
                    stop(&mut active);
                    let shared = Arc::new(Shared::new(retention(c.minutes)));
                    let cfg = Config {
                        fps: key.fps,
                        quality: key.quality.clone(),
                        system_audio: key.system_audio,
                        microphone: key.microphone,
                        segment_secs: SEGMENT_SECS,
                        dir: buffer_dir(),
                        monitor: key.monitor.clone(),
                        force_hdr_path: false,
                        synthetic_audio: None,
                    };
                    let rec = Recorder::start(cfg, shared, Arc::new(emit));
                    *CURRENT.lock().unwrap() = Some(Current { handle: rec.handle(), minutes: c.minutes.clamp(1, 30) });
                    active = Some((rec, key));
                    emit();
                }
                Msg::Shutdown(ack) => {
                    stop(&mut active);
                    let _ = ack.send(());
                    break;
                }
            }
        }
    });
    *c = Some(tx.clone());
    tx
}

/// Arranca, reinicia o para la grabación. Vuelve enseguida: el trabajo se
/// hace en el hilo de control.
pub fn apply(c: &Clips) {
    if !c.enabled && CONTROL.lock().unwrap().is_none() {
        // Nunca ha grabado en esta sesión: solo limpiar restos.
        std::thread::spawn(|| ring::wipe(&buffer_dir()));
        return;
    }
    let _ = control().send(Msg::Apply(c.clone()));
}

fn emit() {
    let _ = crate::ui::app().emit("clips:status", status());
}

pub fn status() -> ClipsStatus {
    let c = crate::settings::get().clips;
    let folder = resolved_folder(&c).to_string_lossy().into_owned();
    let off = ClipsStatus { state: "off", message: None, buffered_seconds: 0, folder: folder.clone() };
    if !c.enabled {
        return off;
    }
    let guard = CURRENT.lock().unwrap();
    let Some(cur) = guard.as_ref() else {
        return ClipsStatus { state: "starting", ..off };
    };
    let info = cur.handle.shared.info.lock().unwrap().clone();
    let buffered = info.since.map_or(0, |s| s.elapsed().as_secs().min(cur.minutes as u64 * 60) as u32);
    let message = match info.state {
        "error" => info.error,
        "recording" => info.encoder.map(|e| {
            let mut m = format!("Codificador: {e}");
            if info.hdr {
                m.push_str(" · HDR → SDR");
            }
            if info.fallback {
                m.push_str(&format!(" · El monitor elegido no está conectado: se graba {}", info.monitor));
            }
            if info.microphone.is_some_and(|ok| !ok.load(Ordering::SeqCst)) {
                m.push_str(" · Micrófono no disponible");
            }
            m
        }),
        _ => None,
    };
    let buffered_seconds = if info.state == "recording" { buffered } else { 0 };
    ClipsStatus { state: if info.state == "off" { "starting" } else { info.state }, message, buffered_seconds, folder }
}

/// Un monitor para el selector de Ajustes.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorOption {
    /// Id estable (ruta de dispositivo del monitor); es lo que se guarda en `clips.monitor`.
    pub id: String,
    /// Nombre del monitor («DELL U2720Q») o «Pantalla 2» si no lo da.
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Monitores conectados, el principal primero.
pub fn monitors() -> Vec<MonitorOption> {
    monitors::list()
        .into_iter()
        .map(|d| MonitorOption { id: d.id, name: d.name, width: d.width, height: d.height, primary: d.primary })
        .collect()
}

static SAVING: Mutex<()> = Mutex::new(());

/// Guarda los últimos `minutes` en la carpeta de destino. No bloquea: el
/// trabajo (cerrar el segmento y unir) se hace en otro hilo.
pub async fn save() -> Result<PathBuf, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("clips-save".into())
        .spawn(move || {
            let _ = tx.send(save_blocking());
        })
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "No se ha podido guardar el clip.".to_string())?
}

fn save_blocking() -> Result<PathBuf, String> {
    let _busy = SAVING.try_lock().map_err(|_| "Ya se está guardando un clip.".to_string())?;
    let (handle, minutes) = {
        let guard = CURRENT.lock().unwrap();
        let cur = guard.as_ref().ok_or("Los clips no están grabando ahora mismo.")?;
        (cur.handle.clone(), cur.minutes)
    };
    let folder = resolved_folder(&crate::settings::get().clips);
    std::fs::create_dir_all(&folder).map_err(|e| format!("No se puede crear la carpeta de destino ({e})."))?;
    let out = output_path(&folder);
    let started = Instant::now();
    handle.save(minutes * 60, &out)?;
    eprintln!("clips: guardado {} en {:?}", out.display(), started.elapsed());
    Ok(out)
}

/// «Winkit AAAA-MM-DD HH-MM-SS.mp4» (hora local), sin pisar ninguno.
fn output_path(folder: &Path) -> PathBuf {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let base = format!("Winkit {:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
    let mut path = folder.join(format!("{base}.mp4"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{base} ({n}).mp4"));
        n += 1;
    }
    path
}

/// Para la grabación y vacía el búfer (al salir de la app).
pub fn shutdown() {
    let tx = CONTROL.lock().unwrap().clone();
    match tx {
        Some(tx) => {
            let (ack, done) = channel();
            if tx.send(Msg::Shutdown(ack)).is_ok() {
                let _ = done.recv_timeout(Duration::from_secs(5));
            }
        }
        None => ring::wipe(&buffer_dir()),
    }
}
