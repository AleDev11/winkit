//! Voz (texto a voz): voces de Windows, voz local descargada al activarla
//! (Supertonic 3 con sherpa-onnx; el motor se llama «kokoro» en los ajustes),
//! ElevenLabs y OpenAI. Ver los comandos en src/lib/api.ts (`voice`) y los
//! tipos en src/shared/types.ts.
//!
//! Cómo suena: el texto se limpia de Markdown, se parte en frases y la
//! primera se sintetiza sola para que empiece a sonar enseguida; las nubes
//! mandan PCM por streaming. Un único reproductor (WASAPI) por frase en curso:
//! hablar otra vez corta lo anterior.

mod cloud;
mod install;
mod local;
mod pcm;
mod player;
mod sherpa;
mod text;
mod windows_tts;

use crate::settings;
use player::Playback;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const ENGINES: [&str; 4] = ["windows", "kokoro", "elevenlabs", "openai"];

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    /// "es-ES", "en-US"… o "mul" si habla varios idiomas.
    pub language: String,
    /// "Mujer", "Hombre" o None.
    pub gender: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceEngineInfo {
    id: &'static str,
    name: &'static str,
    hint: String,
    needs_key: bool,
    has_key: bool,
    key_from_env: bool,
    key_url: Option<&'static str>,
    needs_download: bool,
    installed: bool,
    download_mb: Option<u32>,
    ready: bool,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    /// "idle" | "downloading" | "loading" | "speaking" | "error"
    state: &'static str,
    progress: Option<f64>,
    message: Option<String>,
}

#[derive(Serialize)]
pub struct KeyTestResult {
    ok: bool,
    message: String,
}

// ---------------------------------------------------------------- estado

static STATUS: Mutex<VoiceStatus> = Mutex::new(VoiceStatus { state: "idle", progress: None, message: None });
/// Cada lectura tiene un número; `stop` o una lectura nueva lo cambian y la anterior se corta.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static CURRENT_STOP: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
/// Descarga de la voz local en curso.
static INSTALLING: AtomicBool = AtomicBool::new(false);
static INSTALL_CANCEL: AtomicBool = AtomicBool::new(false);
/// Se pidió quitarla mientras se descargaba: se borra al cancelar.
static REMOVE_AFTER_CANCEL: AtomicBool = AtomicBool::new(false);

fn set_status(state: &'static str, progress: Option<f64>, message: Option<String>) {
    let s = VoiceStatus { state, progress, message };
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = s.clone();
    // En las pruebas no hay ventanas a las que avisar.
    if !cfg!(test) {
        use tauri::Emitter;
        let _ = crate::ui::app().emit("voice:status", &s);
    }
}

pub fn status() -> VoiceStatus {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn speaking_now() -> bool {
    matches!(status().state, "loading" | "speaking")
}

// ---------------------------------------------------------------- motores

fn has_key(engine: &str) -> bool {
    match engine {
        "elevenlabs" | "openai" => settings::provider_key(engine).is_some(),
        _ => true,
    }
}

fn local_installed() -> bool {
    install::all_installed(&install::root())
}

/// Si un motor se puede usar ya (key puesta, descargado…).
fn ready(engine: &str) -> bool {
    match engine {
        "windows" => true,
        "kokoro" => local_installed(),
        "elevenlabs" | "openai" => has_key(engine),
        _ => false,
    }
}

pub fn engines() -> Vec<VoiceEngineInfo> {
    let installed = local_installed();
    let downloading = INSTALLING.load(Ordering::SeqCst);
    ENGINES
        .iter()
        .map(|&id| {
            let (name, hint, key_url) = match id {
                "windows" => ("Voces de Windows", "Gratis y sin conexión. Ya vienen con Windows, sin descargas.".to_string(), None),
                "kokoro" => (
                    "Voz local",
                    format!(
                        "Gratis y sin conexión, mucho más natural. Se descarga una vez (unos {} MB).{}",
                        install::download_mb(),
                        if downloading { " Descargando…" } else { "" }
                    ),
                    None,
                ),
                "elevenlabs" => (
                    "ElevenLabs",
                    "De pago, la mejor calidad: aprox. 0,04–0,08 $ por cada 1.000 caracteres según el modelo.".to_string(),
                    Some("https://elevenlabs.io/app/settings/api-keys"),
                ),
                _ => (
                    "OpenAI",
                    "De pago: aprox. 0,015 $ por minuto de audio. Usa la key de OpenAI de la página IA.".to_string(),
                    Some("https://platform.openai.com/api-keys"),
                ),
            };
            let needs_key = matches!(id, "elevenlabs" | "openai");
            VoiceEngineInfo {
                id,
                name,
                hint,
                needs_key,
                has_key: needs_key && has_key(id),
                key_from_env: needs_key && settings::provider_key_from_env(id).is_some(),
                key_url,
                needs_download: id == "kokoro",
                installed: id != "kokoro" || installed,
                download_mb: (id == "kokoro").then(install::download_mb),
                ready: ready(id),
            }
        })
        .collect()
}

pub async fn voices(engine: &str) -> Result<Vec<VoiceInfo>, String> {
    match engine {
        "windows" => tauri::async_runtime::spawn_blocking(|| sort_spanish_first(windows_tts::voices()?))
            .await
            .map_err(|e| e.to_string())?,
        "kokoro" => Ok(local::voices()),
        "openai" => Ok(cloud::OPENAI_VOICES
            .iter()
            .map(|(id, g)| VoiceInfo {
                id: id.to_string(),
                name: capitalize(id),
                language: "mul".into(),
                gender: Some(g.to_string()),
            })
            .collect()),
        "elevenlabs" => {
            let key = settings::provider_key("elevenlabs").ok_or("Falta tu API key de ElevenLabs.")?;
            cloud::elevenlabs_voices(&key).await
        }
        _ => Err("Motor de voz desconocido.".into()),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Las de español primero (la predeterminada de Windows, si es española, delante).
fn sort_spanish_first(mut v: Vec<VoiceInfo>) -> Result<Vec<VoiceInfo>, String> {
    v.sort_by_key(|x| text::primary(&x.language) != "es");
    Ok(v)
}

// ---------------------------------------------------------------- elegir motor y voz

/// Motor y voz con los que se lee de verdad.
#[derive(Debug, PartialEq)]
pub struct Resolved {
    pub engine: &'static str,
    pub voice: String,
    /// El motor pedido no está listo y se usan las voces de Windows.
    pub fell_back: bool,
}

/// `requested_*`: lo que pide la llamada (None = lo de Ajustes).
pub fn resolve_engine(requested_engine: Option<&str>, requested_voice: Option<&str>, cfg: &settings::Voice, is_ready: &dyn Fn(&str) -> bool) -> Resolved {
    let wanted = requested_engine.filter(|e| !e.is_empty()).unwrap_or(&cfg.engine);
    let engine = ENGINES.iter().copied().find(|e| *e == wanted).unwrap_or("windows");
    // La voz de Ajustes solo vale para el motor de Ajustes.
    let voice = requested_voice
        .map(str::to_string)
        .unwrap_or_else(|| if engine == cfg.engine { cfg.voice.clone() } else { String::new() });
    if engine != "windows" && !is_ready(engine) {
        return Resolved { engine: "windows", voice: String::new(), fell_back: true };
    }
    Resolved { engine, voice, fell_back: false }
}

/// Para voces de un solo idioma (las de Windows): si la elegida no es del
/// idioma del texto y hay alguna que sí, esa (mejor del mismo sexo). Así un
/// texto en español nunca se lee con acento inglés.
pub fn pick_voice(voices: &[VoiceInfo], configured: &str, lang: Option<&str>) -> String {
    let current = voices.iter().find(|v| v.id == configured);
    let Some(lang) = lang else { return configured.to_string() };
    if current.is_some_and(|v| text::primary(&v.language) == lang) {
        return configured.to_string();
    }
    // Sin voz elegida, la predeterminada de Windows (la primera) si ya sirve.
    if configured.is_empty() && voices.first().is_some_and(|v| text::primary(&v.language) == lang) {
        return String::new();
    }
    let same_lang: Vec<&VoiceInfo> = voices.iter().filter(|v| text::primary(&v.language) == lang).collect();
    let gender = current.and_then(|v| v.gender.as_deref());
    same_lang
        .iter()
        .find(|v| gender.is_some() && v.gender.as_deref() == gender)
        .or_else(|| same_lang.first())
        .map(|v| v.id.clone())
        .unwrap_or_else(|| configured.to_string())
}

/// ElevenLabs: la elegida; sin elegir, una española si el texto es español.
fn pick_cloud_voice(voices: &[VoiceInfo], configured: &str, lang: Option<&str>) -> Option<String> {
    if !configured.is_empty() {
        return Some(configured.to_string());
    }
    voices
        .iter()
        .find(|v| lang.is_some_and(|l| text::primary(&v.language) == l))
        .or_else(|| voices.first())
        .map(|v| v.id.clone())
}

// ---------------------------------------------------------------- hablar

/// Recibe el audio de un motor y lo pasa al reproductor (que se crea con el
/// primer trozo, cuando se sabe la frecuencia).
struct Sink {
    stop: Arc<AtomicBool>,
    gen: u64,
    volume: f32,
    player: Option<(u32, Playback)>,
    on_start: Option<Box<dyn FnOnce() + Send>>,
}

impl Sink {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst) || GENERATION.load(Ordering::SeqCst) != self.gen
    }

    fn push(&mut self, rate: u32, samples: Vec<f32>) {
        if samples.is_empty() || self.stopped() {
            return;
        }
        if self.player.as_ref().is_some_and(|(r, _)| *r != rate) {
            // Otra frecuencia (no debería pasar dentro de un motor): se termina lo anterior.
            if let Some((_, p)) = self.player.take() {
                let _ = p.finish();
            }
        }
        if self.player.is_none() {
            let start = self.on_start.take().unwrap_or_else(|| Box::new(|| {}));
            self.player = Some((rate, Playback::start(rate, self.volume, self.stop.clone(), start)));
        }
        if let Some((_, p)) = &self.player {
            p.push(samples);
        }
    }

    /// Espera a que suene todo.
    fn finish(mut self) -> Result<(), String> {
        match self.player.take() {
            Some((_, p)) => p.finish(),
            None => Ok(()),
        }
    }
}

/// Corta lo que esté sonando.
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Some(s) = CURRENT_STOP.lock().unwrap_or_else(|e| e.into_inner()).take() {
        s.store(true, Ordering::SeqCst);
    }
    if speaking_now() || status().state == "error" {
        set_status("idle", None, None);
    }
}

/// Lee `text` en voz alta. Vuelve cuando empieza a sonar (o con el error).
pub async fn speak(text: String, engine: Option<String>, voice: Option<String>) -> Result<(), String> {
    let s = settings::get();
    let clean = text::strip_markdown(&text);
    if clean.trim().is_empty() {
        return Err("No hay nada que leer.".into());
    }
    // Corta lo anterior y se queda con un número nuevo.
    stop();
    let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let stop_flag = Arc::new(AtomicBool::new(false));
    *CURRENT_STOP.lock().unwrap_or_else(|e| e.into_inner()) = Some(stop_flag.clone());

    let lang = text::detect(&clean, &s).or_else(|| text::code_of(&s.my_language).filter(|_| clean.chars().count() < 40));
    let r = resolve_engine(engine.as_deref(), voice.as_deref(), &s.voice, &ready);
    if r.fell_back {
        crate::log_info!("voz", "{} no está listo: se usan las voces de Windows", engine.as_deref().unwrap_or(&s.voice.engine));
    }
    crate::log_info!("voz", "lee {} caracteres con {} (idioma {})", clean.chars().count(), r.engine, lang.unwrap_or("?"));
    set_status("loading", None, None);

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let started_at = Instant::now();
    let tx_start = tx.clone();
    let on_start: Box<dyn FnOnce() + Send> = Box::new(move || {
        if GENERATION.load(Ordering::SeqCst) == gen {
            set_status("speaking", None, None);
        }
        crate::log_info!("voz", "empieza a sonar a los {} ms", started_at.elapsed().as_millis());
        if let Some(t) = tx_start.lock().unwrap().take() {
            let _ = t.send(Ok(()));
        }
    });
    let sink = Sink { stop: stop_flag, gen, volume: s.voice.volume, player: None, on_start: Some(on_start) };
    let voice_cfg = s.voice.clone();
    let fell_back = r.fell_back;
    let _ = std::thread::Builder::new().name("voz".into()).spawn(move || {
        let result = run_engine(&r, &clean, lang, &voice_cfg, sink);
        let current = GENERATION.load(Ordering::SeqCst) == gen;
        match &result {
            Ok(()) if current => set_status("idle", None, None),
            Err(e) if current => {
                crate::log_warn!("voz", "{}: {e}", r.engine);
                set_status("error", None, Some(e.clone()));
            }
            _ => {}
        }
        // Si no llegó a sonar (error, nada que decir o cortado), se responde igual.
        if let Some(t) = tx.lock().unwrap().take() {
            let _ = t.send(result);
        }
    });
    let r = rx.await.map_err(|_| "La voz se ha interrumpido.".to_string())?;
    if fell_back && r.is_ok() {
        crate::log_info!("voz", "leído con la voz de Windows en lugar de la elegida");
    }
    r
}

/// Sintetiza y reproduce (en el hilo de la lectura). Termina cuando acaba de sonar.
fn run_engine(r: &Resolved, clean: &str, lang: Option<&'static str>, cfg: &settings::Voice, mut sink: Sink) -> Result<(), String> {
    let speed = cfg.speed.clamp(0.5, 2.0);
    match r.engine {
        "windows" => {
            let voices = windows_tts::voices().unwrap_or_default();
            let voice = pick_voice(&voices, &r.voice, lang);
            let synth = windows_tts::Synth::new(&voice, speed)?;
            for chunk in text::chunks(clean, 90, 400) {
                if sink.stopped() {
                    return Ok(());
                }
                let audio = synth.speak(&chunk)?;
                sink.push(audio.sample_rate, audio.samples);
            }
        }
        "kokoro" => {
            let root = install::root();
            let voice = if r.voice.is_empty() { "F1".to_string() } else { r.voice.clone() };
            for chunk in text::chunks(clean, 80, 300) {
                if sink.stopped() {
                    return Ok(());
                }
                let stop = sink.stop.clone();
                let gen = sink.gen;
                let cancel = move || stop.load(Ordering::SeqCst) || GENERATION.load(Ordering::SeqCst) != gen;
                let (rate, samples) = local::synth(&root, &chunk, &voice, lang, speed, &|| {}, &cancel)?;
                sink.push(rate, samples);
            }
        }
        engine @ ("elevenlabs" | "openai") => {
            let key = settings::provider_key(engine).ok_or("Falta la API key.")?;
            let voice = if engine == "openai" {
                if r.voice.is_empty() { "marin".to_string() } else { r.voice.clone() }
            } else {
                let list = if r.voice.is_empty() {
                    tauri::async_runtime::block_on(cloud::elevenlabs_voices(&key))?
                } else {
                    Vec::new()
                };
                pick_cloud_voice(&list, &r.voice, lang).ok_or("Tu cuenta de ElevenLabs no tiene voces.")?
            };
            // Trozos grandes (la red ya va en streaming); el primero corto para empezar antes.
            let pieces = text::chunks(clean, 250, if engine == "openai" { 3500 } else { 2500 });
            for piece in pieces {
                if sink.stopped() {
                    return Ok(());
                }
                let stop = sink.stop.clone();
                let gen = sink.gen;
                let stopped = move || stop.load(Ordering::SeqCst) || GENERATION.load(Ordering::SeqCst) != gen;
                let mut on_audio = |samples: Vec<f32>| sink.push(cloud::SAMPLE_RATE, samples);
                let fut = async {
                    if engine == "openai" {
                        cloud::openai_speak(&key, &voice, &piece, lang, speed, &stopped, &mut on_audio).await
                    } else {
                        cloud::elevenlabs_speak(&key, &voice, &piece, lang, speed, &stopped, &mut on_audio).await
                    }
                };
                tauri::async_runtime::block_on(fut)?;
            }
        }
        _ => return Err("Motor de voz desconocido.".into()),
    }
    if sink.stopped() {
        return Ok(());
    }
    sink.finish()
}

// ---------------------------------------------------------------- instalar la voz local

pub async fn install_local() -> Result<(), String> {
    if install::all_installed(&install::root()) {
        set_status("idle", None, None);
        return Ok(());
    }
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("Ya se está descargando.".into());
    }
    INSTALL_CANCEL.store(false, Ordering::SeqCst);
    REMOVE_AFTER_CANCEL.store(false, Ordering::SeqCst);
    crate::log_info!("voz", "descargando la voz local ({} MB)", install::download_mb());
    set_status("downloading", Some(0.0), Some("Preparando la descarga…".into()));
    let last = Mutex::new((Instant::now(), -1.0f64));
    let report = |done: Option<u64>, total: u64, what: &str| match done {
        Some(d) => {
            let p = d as f64 / total.max(1) as f64;
            let mut l = last.lock().unwrap();
            // Como mucho ~5 avisos por segundo.
            if p - l.1 >= 0.01 || l.0.elapsed().as_millis() >= 200 || d >= total {
                *l = (Instant::now(), p);
                set_status(
                    "downloading",
                    Some(p.min(1.0)),
                    Some(format!("Descargando {} · {} de {} MB", what.to_lowercase(), d / 1_000_000, total / 1_000_000)),
                );
            }
        }
        None => set_status("downloading", None, Some(format!("Instalando {}…", what.to_lowercase()))),
    };
    let root = install::root();
    let r = install::install(&root, &install::ASSETS, &install::Progress { cancel: &INSTALL_CANCEL, report: &report }).await;
    INSTALLING.store(false, Ordering::SeqCst);
    match r {
        Ok(()) => {
            crate::log_info!("voz", "voz local lista");
            set_status("idle", None, None);
            Ok(())
        }
        Err(install::Error::Cancelled) => {
            crate::log_info!("voz", "descarga de la voz local cancelada");
            if REMOVE_AFTER_CANCEL.swap(false, Ordering::SeqCst) {
                let _ = install::remove_all(&root);
            }
            set_status("idle", None, Some("Descarga cancelada.".into()));
            Err("Descarga cancelada.".into())
        }
        Err(install::Error::Failed(e)) => {
            crate::log_warn!("voz", "no se ha podido instalar la voz local: {e}");
            set_status("error", None, Some(e.clone()));
            Err(e)
        }
    }
}

pub fn cancel_install() {
    INSTALL_CANCEL.store(true, Ordering::SeqCst);
}

pub fn remove_local() -> Result<(), String> {
    if INSTALLING.load(Ordering::SeqCst) {
        REMOVE_AFTER_CANCEL.store(true, Ordering::SeqCst);
        cancel_install();
        return Ok(());
    }
    stop();
    local::unload_all();
    install::remove_all(&install::root())?;
    crate::log_info!("voz", "voz local borrada");
    // Si era el motor elegido, vuelve a las voces de Windows.
    let s = settings::get();
    if s.voice.engine == "kokoro" {
        let mut v = s.voice.clone();
        v.engine = "windows".into();
        v.voice = String::new();
        let _ = settings::save_patch(serde_json::json!({ "voice": v }));
    }
    set_status("idle", None, None);
    Ok(())
}

// ---------------------------------------------------------------- keys

pub async fn save_key(engine: &str, key: &str) -> KeyTestResult {
    let key = key.trim();
    let (name, check) = match engine {
        "elevenlabs" => ("ElevenLabs", cloud::elevenlabs_voices(key).await.map(|_| ())),
        "openai" => ("OpenAI", cloud::openai_validate(key).await),
        _ => return KeyTestResult { ok: false, message: "Este motor no usa API key.".into() },
    };
    if key.is_empty() {
        return KeyTestResult { ok: false, message: format!("Pega tu API key de {name}.") };
    }
    match check.and_then(|()| settings::set_provider_key(engine, key)) {
        Ok(()) => {
            crate::log_info!("voz", "key de {engine} guardada");
            KeyTestResult { ok: true, message: "La API key funciona y está guardada.".into() }
        }
        Err(e) => KeyTestResult { ok: false, message: e },
    }
}

pub fn remove_key(engine: &str) {
    if matches!(engine, "elevenlabs" | "openai") {
        settings::remove_provider_key(engine);
    }
}

// ---------------------------------------------------------------- comandos

pub mod commands {
    use super::*;

    #[tauri::command]
    pub fn voice_engines() -> Vec<VoiceEngineInfo> {
        engines()
    }

    #[tauri::command]
    pub async fn voice_voices(engine: String) -> Result<Vec<VoiceInfo>, String> {
        voices(&engine).await
    }

    #[tauri::command]
    pub async fn voice_save_key(engine: String, key: String) -> KeyTestResult {
        save_key(&engine, &key).await
    }

    #[tauri::command]
    pub fn voice_remove_key(engine: String) {
        remove_key(&engine);
    }

    #[tauri::command]
    pub async fn voice_speak(text: String, engine: Option<String>, voice: Option<String>) -> Result<(), String> {
        speak(text, engine, voice).await
    }

    #[tauri::command]
    pub fn voice_stop() {
        stop();
    }

    #[tauri::command]
    pub fn voice_status() -> VoiceStatus {
        status()
    }

    #[tauri::command]
    pub async fn voice_install(engine: String) -> Result<(), String> {
        if engine != "kokoro" {
            return Ok(());
        }
        install_local().await
    }

    /// Cancela la descarga en curso (lo bajado se conserva para reanudar).
    #[tauri::command]
    pub fn voice_cancel() {
        cancel_install();
    }

    #[tauri::command]
    pub async fn voice_remove(engine: String) -> Result<(), String> {
        if engine != "kokoro" {
            return Ok(());
        }
        tauri::async_runtime::spawn_blocking(remove_local).await.map_err(|e| e.to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(engine: &str, voice: &str) -> settings::Voice {
        settings::Voice { engine: engine.into(), voice: voice.into(), ..Default::default() }
    }

    fn v(id: &str, lang: &str, gender: Option<&str>) -> VoiceInfo {
        VoiceInfo { id: id.into(), name: id.into(), language: lang.into(), gender: gender.map(str::to_string) }
    }

    #[test]
    fn resuelve_el_motor() {
        let all = |_: &str| true;
        let none = |_: &str| false;
        assert_eq!(resolve_engine(None, None, &cfg("kokoro", "M2"), &all), Resolved { engine: "kokoro", voice: "M2".into(), fell_back: false });
        // Lo que pide la llamada manda; la voz de Ajustes no se cuela en otro motor.
        assert_eq!(resolve_engine(Some("openai"), None, &cfg("kokoro", "M2"), &all).voice, "");
        assert_eq!(resolve_engine(Some("openai"), Some("cedar"), &cfg("kokoro", "M2"), &all).voice, "cedar");
        // Sin key o sin descargar: voces de Windows.
        assert_eq!(resolve_engine(None, None, &cfg("elevenlabs", "x"), &none), Resolved { engine: "windows", voice: "".into(), fell_back: true });
        // Motor desconocido → Windows; Windows siempre vale.
        assert_eq!(resolve_engine(Some("raro"), None, &cfg("windows", "w"), &none).engine, "windows");
        assert!(!resolve_engine(None, None, &cfg("windows", "w"), &none).fell_back);
    }

    #[test]
    fn elige_voz_del_idioma() {
        let voices =
            [v("david", "en-US", Some("Hombre")), v("zira", "en-US", Some("Mujer")), v("helena", "es-ES", Some("Mujer")), v("pablo", "es-ES", Some("Hombre"))];
        // Texto en español con una voz inglesa: la española del mismo sexo.
        assert_eq!(pick_voice(&voices, "david", Some("es")), "pablo");
        assert_eq!(pick_voice(&voices, "zira", Some("es")), "helena");
        // Ya es del idioma: se respeta.
        assert_eq!(pick_voice(&voices, "pablo", Some("es")), "pablo");
        // Sin idioma detectado: la elegida.
        assert_eq!(pick_voice(&voices, "david", None), "david");
        // Sin voz elegida: la predeterminada (primera) si ya es del idioma; si no, la primera del idioma.
        assert_eq!(pick_voice(&voices, "", Some("en")), "");
        assert_eq!(pick_voice(&voices, "", Some("es")), "helena");
        // No hay voces de ese idioma: la elegida.
        assert_eq!(pick_voice(&voices, "helena", Some("fr")), "helena");
    }

    #[test]
    fn voz_de_la_nube() {
        let list = [v("en1", "en-US", None), v("es1", "es-ES", None)];
        assert_eq!(pick_cloud_voice(&list, "", Some("es")).as_deref(), Some("es1"));
        assert_eq!(pick_cloud_voice(&list, "", Some("fr")).as_deref(), Some("en1"));
        assert_eq!(pick_cloud_voice(&list, "mia", Some("es")).as_deref(), Some("mia"));
        assert_eq!(pick_cloud_voice(&[], "", None), None);
    }

    #[test]
    fn lista_de_motores() {
        let e = engines();
        assert_eq!(e.iter().map(|x| x.id).collect::<Vec<_>>(), ENGINES);
        assert!(e[0].ready && !e[0].needs_key);
        assert!(e[1].needs_download && e[1].download_mb.unwrap() > 100);
        assert!(e[2].needs_key && e[2].key_url.is_some());
    }

    /// Lectura completa (como `voice_speak`) con el motor y la voz que se pidan; suena de verdad.
    fn lectura_completa(engine: &str) {
        let t = Instant::now();
        tauri::async_runtime::block_on(speak("Hola. Esto es una prueba de Winkit.".into(), Some(engine.into()), None)).unwrap();
        let first = t.elapsed();
        while speaking_now() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        println!("{engine}: empieza a sonar a los {first:?}; termina a los {:?}", t.elapsed());
        assert_eq!(status().state, "idle");
    }

    #[test]
    #[ignore]
    fn lectura_completa_windows() {
        lectura_completa("windows");
    }

    /// Necesita la voz local en %LOCALAPPDATA%\Winkit\voice. Con WINKIT_VOICE_TEST_INSTALL=1 la
    /// descarga antes (para usarlo con LOCALAPPDATA apuntando a una carpeta temporal).
    #[test]
    #[ignore]
    fn lectura_completa_local() {
        if !local_installed() && std::env::var("WINKIT_VOICE_TEST_INSTALL").is_ok() {
            tauri::async_runtime::block_on(install_local()).unwrap();
        }
        if !local_installed() {
            println!("voz local sin instalar: se omite");
            return;
        }
        lectura_completa("kokoro");
        // Ya cargada: lo que tarda en empezar de verdad.
        lectura_completa("kokoro");
    }

    /// Lee una frase en voz alta con la voz de Windows por la salida predeterminada (suena ~2 s).
    #[test]
    #[ignore]
    fn suena_de_verdad() {
        let voices = windows_tts::voices().unwrap();
        let voice = pick_voice(&voices, "", Some("es"));
        let synth = windows_tts::Synth::new(&voice, 1.2).unwrap();
        let audio = synth.speak("Prueba de sonido de Winkit.").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let started = Arc::new(Mutex::new(None));
        let s2 = started.clone();
        let t = Instant::now();
        let p = Playback::start(audio.sample_rate, 0.5, stop, Box::new(move || *s2.lock().unwrap() = Some(t.elapsed())));
        for c in audio.samples.chunks(4096) {
            p.push(c.to_vec());
        }
        p.finish().unwrap();
        let total = t.elapsed();
        println!("arranca en {:?}; {:.2} s de audio sonaron en {total:?}", started.lock().unwrap().unwrap(), audio.seconds());
        assert!(started.lock().unwrap().is_some());
        assert!(total.as_secs_f64() >= audio.seconds() * 0.9);
    }
}
