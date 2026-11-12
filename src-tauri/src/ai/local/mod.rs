//! IA local: Winkit descarga el motor oficial de llama.cpp y un modelo GGUF
//! solo cuando el usuario lo activa, y arranca el servidor (compatible con la
//! API de OpenAI, en 127.0.0.1) al primer uso. Pasados unos minutos sin usarse
//! lo para: así no ocupa VRAM mientras juegas o grabas clips.
//!
//! Todo vive en %LOCALAPPDATA%\Winkit\local-ai\ (engine\, models\).

pub mod catalog;
pub mod download;
pub mod gpu;
pub mod server;

use super::error::Failure;
use crate::settings::{self, LocalAi, Settings};
use catalog::{Engine, Model, ENGINE_CPU, ENGINE_TAG, ENGINE_VULKAN, MODELS};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const NAME: &str = "IA local";

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalAiStatus {
    /// off · downloading · verifying · starting · ready · idle · error
    pub state: &'static str,
    pub progress: Option<f64>,
    pub message: Option<String>,
    pub backend: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size_mb: u64,
    pub memory_gb: f32,
    pub installed: bool,
    pub recommended: bool,
}

struct State {
    server: Option<server::Server>,
    /// Backend del servidor en marcha ("GPU (Vulkan): …" o "CPU").
    backend: Option<String>,
    last_used: Instant,
    in_flight: usize,
    /// Descarga, verificación, arranque o error: manda sobre el estado calculado.
    phase: Option<LocalAiStatus>,
    cancel: Option<Arc<AtomicBool>>,
    watcher: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Solo un arranque a la vez (dos peticiones seguidas no lanzan dos servidores).
static STARTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ROOT: OnceLock<PathBuf> = OnceLock::new();

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = g.get_or_insert_with(|| State {
        server: None,
        backend: None,
        last_used: Instant::now(),
        in_flight: 0,
        phase: None,
        cancel: None,
        watcher: false,
    });
    f(st)
}

// ------------------------------------------------------------ rutas

pub fn root() -> PathBuf {
    ROOT.get_or_init(|| {
        let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
        base.join("Winkit").join("local-ai")
    })
    .clone()
}

/// Para el test de punta a punta: todo en una carpeta temporal.
#[cfg(test)]
pub fn set_root(p: PathBuf) {
    let _ = ROOT.set(p);
}

fn engine_dir(e: &Engine) -> PathBuf {
    root().join("engine").join(format!("{ENGINE_TAG}-{}", e.variant))
}

fn engine_exe(e: &Engine) -> PathBuf {
    engine_dir(e).join("llama-server.exe")
}

fn engine_installed(e: &Engine) -> bool {
    engine_dir(e).join(".winkit-ok").exists() && engine_exe(e).exists()
}

/// El motor que hay: Vulkan si está (sirve también sin GPU), si no el de CPU.
fn installed_engine() -> Option<&'static Engine> {
    [&ENGINE_VULKAN, &ENGINE_CPU].into_iter().find(|e| engine_installed(e))
}

pub fn model_path(m: &Model) -> PathBuf {
    root().join("models").join(m.file)
}

pub fn is_installed(m: &Model) -> bool {
    std::fs::metadata(model_path(m)).is_ok_and(|f| f.len() == m.size)
}

fn installed_models() -> Vec<&'static Model> {
    MODELS.iter().filter(|m| is_installed(m)).collect()
}

/// El modelo que se usa: el pedido si está instalado; si no, el de Ajustes; si
/// no, el primero instalado.
pub fn resolve_model(local: &LocalAi, requested: &str) -> Option<&'static Model> {
    let pick = |id: &str| catalog::model(id).filter(|m| is_installed(m));
    pick(requested).or_else(|| pick(&local.model)).or_else(|| installed_models().into_iter().next())
}

/// Lista para usarse: activada, con motor y con algún modelo.
pub fn ready(s: &Settings) -> bool {
    s.ai.local.enabled && installed_engine().is_some() && resolve_model(&s.ai.local, "").is_some()
}

pub fn model_name(id: &str) -> String {
    catalog::model(id).map_or_else(|| id.to_string(), |m| m.name.to_string())
}

// ------------------------------------------------------------ estado

fn gpu_backend(gpu_name: Option<String>) -> String {
    match gpu_name {
        Some(n) => format!("GPU (Vulkan): {n}"),
        None => "GPU (Vulkan)".into(),
    }
}

/// Dónde correría ahora mismo (antes de arrancar).
fn expected_backend(local: &LocalAi) -> String {
    let gpu = gpu::best();
    let vulkan = installed_engine().is_some_and(|e| e.variant == "vulkan");
    if local.gpu && vulkan && gpu.is_some() {
        gpu_backend(gpu.map(|g| g.name))
    } else {
        "CPU".into()
    }
}

pub fn status() -> LocalAiStatus {
    let s = settings::get();
    if let Some(p) = with(|st| st.phase.clone()) {
        return p;
    }
    let (running, backend) = with(|st| (st.server.as_mut().map(|sv| (sv.alive(), sv.alias.clone())), st.backend.clone()));
    if let Some((true, alias)) = running {
        return LocalAiStatus { state: "ready", progress: None, message: None, backend, model: Some(model_name(&alias)) };
    }
    let model = resolve_model(&s.ai.local, "").filter(|_| installed_engine().is_some());
    match model {
        Some(m) if s.ai.local.enabled => LocalAiStatus {
            state: "idle",
            progress: None,
            message: None,
            backend: Some(expected_backend(&s.ai.local)),
            model: Some(m.name.into()),
        },
        _ => LocalAiStatus { state: "off", progress: None, message: None, backend: None, model: model.map(|m| m.name.into()) },
    }
}

fn emit() {
    let st = status();
    if let Some(app) = crate::ui::try_app() {
        use tauri::Emitter;
        let _ = app.emit("local-ai:status", &st);
    }
}

fn set_phase(p: Option<LocalAiStatus>) {
    with(|st| st.phase = p);
    emit();
}

fn phase(state: &'static str, progress: Option<f64>, message: impl Into<String>) -> Option<LocalAiStatus> {
    Some(LocalAiStatus { state, progress, message: Some(message.into()), backend: None, model: None })
}

pub fn catalog() -> Vec<LocalModel> {
    MODELS
        .iter()
        .map(|m| LocalModel {
            id: m.id,
            name: m.name,
            description: m.description,
            size_mb: m.size / 1_000_000,
            memory_gb: m.memory_gb,
            installed: is_installed(m),
            recommended: m.recommended,
        })
        .collect()
}

// ------------------------------------------------------------ instalar

fn gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1e9).replace('.', ",")
}

fn engine_for_this_pc() -> &'static Engine {
    if gpu::best().is_some() {
        &ENGINE_VULKAN
    } else {
        &ENGINE_CPU
    }
}

fn part_len(dest: &Path) -> u64 {
    std::fs::metadata(download::part_path(dest)).map(|m| m.len()).unwrap_or(0)
}

/// Bytes que faltan por bajar (motor + modelo), para comprobar el espacio antes.
fn needed_bytes(engine: Option<&Engine>, model: &Model) -> u64 {
    let engine = engine.map_or(0, |e| {
        let zip = root().join("engine").join(format!("{ENGINE_TAG}-{}.zip", e.variant));
        // El zip más lo descomprimido (~3 veces el zip).
        e.size.saturating_sub(part_len(&zip)) + e.size * 3
    });
    let model = if is_installed(model) { 0 } else { model.size.saturating_sub(part_len(&model_path(model))) };
    engine + model + 300_000_000
}

/// `model`: el modelo que se está instalando (para que Ajustes sepa en qué tarjeta va la barra).
fn progress_cb(label: String, model: &'static str) -> impl Fn(u64, u64) + Send + Sync {
    move |done, total| {
        let p = if total > 0 { done as f64 / total as f64 } else { 0.0 };
        let mut st = phase("downloading", Some(p), format!("{label} · {} de {} GB", gb(done), gb(total)));
        if let Some(st) = st.as_mut() {
            st.model = Some(model.into());
        }
        set_phase(st);
    }
}

fn map_err(e: download::Error) -> Result<(), String> {
    match e {
        download::Error::Cancelled => Err(String::new()),
        download::Error::Failed(m) => Err(m),
    }
}

async fn install_engine(e: &'static Engine, cancel: &Arc<AtomicBool>, for_model: &'static str) -> Result<(), String> {
    if engine_installed(e) {
        return Ok(());
    }
    let dir = root().join("engine");
    let zip = dir.join(format!("{ENGINE_TAG}-{}.zip", e.variant));
    let label = if e.variant == "vulkan" { "Motor llama.cpp (Vulkan)" } else { "Motor llama.cpp (CPU)" };
    let verifying = || set_phase(phase("verifying", None, "Verificando el motor…"));
    if let Err(err) = download::fetch(e.url, &zip, e.size, e.sha256, cancel, &progress_cb(label.into(), for_model), &verifying).await {
        return map_err(err);
    }
    set_phase(phase("verifying", None, "Descomprimiendo el motor…"));
    let target = engine_dir(e);
    let z = zip.clone();
    tauri::async_runtime::spawn_blocking(move || extract(&z, &target))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("No se ha podido descomprimir el motor: {e}"))?;
    let _ = std::fs::remove_file(&zip);
    Ok(())
}

/// Descomprime en una carpeta temporal y la renombra al final: un motor a medias nunca parece instalado.
fn extract(zip: &Path, target: &Path) -> Result<(), String> {
    let tmp = target.with_extension("tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(zip).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        // `enclosed_name` descarta rutas con «..» o absolutas.
        let Some(rel) = entry.enclosed_name() else { continue };
        let out = tmp.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
    }
    if !tmp.join("llama-server.exe").exists() {
        return Err("el zip no trae llama-server.exe".into());
    }
    std::fs::write(tmp.join(".winkit-ok"), ENGINE_TAG).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(target);
    std::fs::rename(&tmp, target).map_err(|e| e.to_string())
}

/// Descarga lo que falte (motor y modelo), lo verifica y deja la IA local
/// activada con ese modelo. Vuelve al terminar; el progreso va por eventos.
pub async fn install(id: &str) -> Result<(), String> {
    let model = catalog::model(id).ok_or("Ese modelo no está en el catálogo.")?;
    let cancel = Arc::new(AtomicBool::new(false));
    if !with(|st| {
        if st.cancel.is_some() {
            return false;
        }
        st.cancel = Some(cancel.clone());
        true
    }) {
        return Err("Ya hay una descarga en marcha.".into());
    }
    let result = install_inner(model, &cancel).await;
    with(|st| st.cancel = None);
    match result {
        Ok(()) => {
            let mut ai = settings::get().ai;
            ai.local.enabled = true;
            // Lo que se acaba de instalar es lo que se quiere usar.
            ai.local.model = model.id.into();
            settings::save_patch(serde_json::json!({ "ai": ai }))?;
            crate::log::info("local-ai", &format!("instalado {}", model.id));
            set_phase(None);
            Ok(())
        }
        Err(m) if m.is_empty() => {
            set_phase(None);
            Ok(())
        }
        Err(m) => {
            crate::log::warn("local-ai", &format!("instalación fallida: {m}"));
            set_phase(phase("error", None, m.clone()));
            Err(m)
        }
    }
}

async fn install_inner(model: &'static Model, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let engine = if installed_engine().is_some() { None } else { Some(engine_for_this_pc()) };
    let need = needed_bytes(engine, model);
    if let Some(free) = download::free_space(&root()) {
        if free < need {
            return Err(format!("No hay espacio en el disco: hacen falta {} GB libres y hay {} GB.", gb(need), gb(free)));
        }
    }
    if let Some(e) = engine {
        install_engine(e, cancel, model.name).await?;
    }
    if is_installed(model) {
        return Ok(());
    }
    let verifying = || set_phase(phase("verifying", None, format!("Verificando {}…", model.name)));
    match download::fetch(model.url, &model_path(model), model.size, model.sha256, cancel, &progress_cb(model.name.into(), model.name), &verifying).await {
        Ok(()) => Ok(()),
        Err(e) => map_err(e),
    }
}

pub fn cancel() {
    if let Some(c) = with(|st| st.cancel.clone()) {
        c.store(true, Ordering::Relaxed);
    }
}

/// Borra un modelo (y el motor si ya no queda ninguno: no se deja nada en disco).
pub fn remove(id: &str) -> Result<(), String> {
    let model = catalog::model(id).ok_or("Ese modelo no está en el catálogo.")?;
    if with(|st| st.server.as_ref().is_some_and(|s| s.alias == model.id)) {
        unload();
    }
    let path = model_path(model);
    let _ = std::fs::remove_file(download::part_path(&path));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("No se ha podido borrar el modelo: {e}"))?;
    }
    let mut ai = settings::get().ai;
    if ai.local.model == model.id {
        ai.local.model = installed_models().first().map(|m| m.id.to_string()).unwrap_or_default();
        settings::save_patch(serde_json::json!({ "ai": ai }))?;
    }
    if installed_models().is_empty() {
        unload();
        let _ = std::fs::remove_dir_all(root().join("engine"));
    }
    set_phase(None);
    Ok(())
}

pub fn open_folder() {
    let _ = std::fs::create_dir_all(root());
    crate::win::system::open(&root().to_string_lossy());
}

// ------------------------------------------------------------ servidor

/// Para el servidor y libera la memoria (VRAM y RAM).
pub fn unload() {
    let server = with(|st| {
        st.backend = None;
        st.server.take()
    });
    if let Some(s) = server {
        s.stop();
        crate::log::info("local-ai", "servidor parado: memoria liberada");
    }
    emit();
}

/// Ajustes guardados: si se ha desactivado o ha cambiado cómo arranca, se para
/// (vuelve a arrancar con lo nuevo en la siguiente petición).
pub fn apply(local: &LocalAi) {
    let stale = with(|st| {
        st.server.as_ref().is_some_and(|s| !local.enabled || s.gpu != local.gpu || s.context != local.context.max(512))
    });
    if stale {
        unload();
    } else {
        emit();
    }
}

pub fn shutdown() {
    cancel();
    unload();
}

/// Mientras exista, el servidor está en uso y no se descarga por inactividad.
pub struct InUse;

impl Drop for InUse {
    fn drop(&mut self) {
        with(|st| {
            st.in_flight = st.in_flight.saturating_sub(1);
            st.last_used = Instant::now();
        });
    }
}

/// La URL del servidor con el modelo listo, arrancándolo si hace falta.
pub async fn ensure_running(s: &Settings, requested: &str) -> Result<(String, String, InUse), Failure> {
    if !s.ai.local.enabled {
        return Err(Failure::settings("La IA local está desactivada. Actívala en Ajustes → IA."));
    }
    let model = resolve_model(&s.ai.local, requested).ok_or_else(|| Failure::settings("Instala un modelo de IA local en Ajustes → IA."))?;
    let gpu = s.ai.local.gpu;
    let context = s.ai.local.context.max(512);
    let _start = STARTING.lock().await;
    let current = with(|st| {
        let reuse = st.server.as_mut().is_some_and(|sv| sv.alive() && sv.alias == model.id && sv.gpu == gpu && sv.context == context);
        reuse.then(|| {
            st.in_flight += 1;
            st.last_used = Instant::now();
            st.server.as_ref().map(|sv| sv.base_url()).unwrap_or_default()
        })
    });
    if let Some(url) = current {
        return Ok((url, model.id.into(), InUse));
    }
    unload();
    set_phase(Some(LocalAiStatus {
        state: "starting",
        progress: None,
        message: Some(format!("Cargando {}…", model.name)),
        backend: None,
        model: Some(model.name.into()),
    }));
    let started = launch(model, gpu, context).await;
    let result = match started {
        Ok((server, backend)) => {
            let url = server.base_url();
            let spawn_watcher = with(|st| {
                st.server = Some(server);
                st.backend = Some(backend);
                st.in_flight += 1;
                st.last_used = Instant::now();
                !std::mem::replace(&mut st.watcher, true)
            });
            if spawn_watcher {
                tauri::async_runtime::spawn(watch_idle());
            }
            set_phase(None);
            Ok((url, model.id.to_string(), InUse))
        }
        Err(m) => {
            crate::log::warn("local-ai", &format!("no arranca: {m}"));
            set_phase(phase("error", None, format!("No ha podido arrancar: {m}")));
            Err(Failure::settings(format!("La IA local no ha podido arrancar: {m}")))
        }
    };
    result
}

/// Arranca con la GPU si se puede; si falla, en CPU; y si ni así (el motor
/// Vulkan no carga en este equipo), baja el motor de CPU y lo intenta con él.
async fn launch(model: &'static Model, gpu: bool, context: u32) -> Result<(server::Server, String), String> {
    let log = root().join("server.log");
    let mk = |e: &Engine, gpu: bool| server::Launch {
        exe: engine_exe(e),
        model: model_path(model),
        alias: model.id.into(),
        context,
        gpu,
        log: log.clone(),
    };
    let engine = installed_engine().ok_or("falta el motor; vuelve a instalar el modelo")?;
    if gpu && engine.variant == "vulkan" {
        let exe = engine_exe(engine);
        let device = tauri::async_runtime::spawn_blocking(move || server::vulkan_device(&exe)).await.ok().flatten();
        if device.is_some() {
            match server::start(&mk(engine, true)).await {
                Ok(s) => return Ok((s, gpu_backend(gpu::best().map(|g| g.name).or(device)))),
                Err(e) => crate::log::warn("local-ai", &format!("con GPU no arranca ({e}); se prueba en CPU")),
            }
        }
    }
    let err = match server::start(&mk(engine, false)).await {
        Ok(s) => return Ok((s, "CPU".into())),
        Err(e) => e,
    };
    // Si el fallo es del modelo, otro motor no lo arregla.
    if engine.variant == "vulkan" && !err.contains("model") {
        let cancel = Arc::new(AtomicBool::new(false));
        install_engine(&ENGINE_CPU, &cancel, model.name).await?;
        let _ = std::fs::remove_dir_all(engine_dir(&ENGINE_VULKAN));
        return server::start(&mk(&ENGINE_CPU, false)).await.map(|s| (s, "CPU".into()));
    }
    Err(err)
}

/// Descarga el modelo tras `unloadAfterMinutes` sin usarse.
async fn watch_idle() {
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let minutes = settings::get().ai.local.unload_after_minutes;
        let (running, idle) = with(|st| (st.server.is_some(), st.in_flight == 0 && st.last_used.elapsed() >= Duration::from_secs(u64::from(minutes) * 60)));
        if !running {
            with(|st| st.watcher = false);
            return;
        }
        if minutes > 0 && idle {
            crate::log::info("local-ai", &format!("{minutes} min sin usarse: se libera la memoria"));
            unload();
        }
    }
}

/// Pid del servidor en marcha (para el test de punta a punta).
#[cfg(test)]
pub fn server_pid() -> Option<u32> {
    with(|st| st.server.as_ref().map(|s| s.pid()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ModelRef;

    #[test]
    fn el_catalogo_para_ajustes() {
        let c = catalog();
        assert_eq!(c.len(), MODELS.len());
        assert!(c.iter().all(|m| m.size_mb > 500));
        assert!(needed_bytes(Some(&ENGINE_VULKAN), &MODELS[0]) > MODELS[0].size);
    }

    fn vram_used_mb() -> Option<u64> {
        let out = std::process::Command::new("nvidia-smi").args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().next()?.trim().parse().ok()
    }

    fn ram_mb(pid: u32) -> Option<u64> {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {pid} -ErrorAction SilentlyContinue).WorkingSet64")])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok().map(|b| b / 1_000_000)
    }

    fn alive(pid: u32) -> bool {
        let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
    }

    /// Velocidad de generación según el propio servidor (sin streaming, con `timings`).
    async fn tokens_per_second(base: &str, model: &str) -> f64 {
        let body = serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": "Escribe un párrafo de unas cien palabras sobre el otoño en Madrid." }],
            "max_tokens": 200,
        });
        let v: serde_json::Value = reqwest::Client::new().post(format!("{base}/chat/completions")).json(&body).send().await.unwrap().json().await.unwrap();
        v["timings"]["predicted_per_second"].as_f64().unwrap_or(0.0)
    }

    /// De punta a punta en este equipo, en una carpeta temporal: descarga el
    /// motor y el modelo más pequeño, arranca, corrige un texto en español en
    /// streaming, mide, libera la memoria y lo borra todo. Baja ~2,9 GB.
    /// `cargo test --lib ai::local::tests::punta_a_punta -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn punta_a_punta() {
        let dir = std::env::temp_dir().join(format!("winkit-local-ai-e2e-{}", std::process::id()));
        set_root(dir.clone());
        assert_eq!(root(), dir);
        let model = MODELS.iter().min_by_key(|m| m.size).unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt.block_on(run_e2e(model))));
        unload();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!dir.exists(), "quedan archivos en {}", dir.display());
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    async fn run_e2e(model: &'static Model) {
        println!("GPU (DXGI): {:?}", gpu::best());
        let cancel = Arc::new(AtomicBool::new(false));
        let t = Instant::now();
        install_inner(model, &cancel).await.unwrap();
        println!("descarga y verificación: {:.1} s · motor {} ({} MB) · {} ({} MB)", t.elapsed().as_secs_f64(), installed_engine().unwrap().variant, ENGINE_VULKAN.size / 1_000_000, model.name, model.size / 1_000_000);
        assert!(is_installed(model));

        let mut s = Settings::default();
        s.ai.local = LocalAi { enabled: true, model: model.id.into(), gpu: true, context: 4096, unload_after_minutes: 5 };
        assert!(ready(&s));
        let vram_before = vram_used_mb();

        for gpu in [true, false] {
            s.ai.local.gpu = gpu;
            let t = Instant::now();
            let (base, id, guard) = ensure_running(&s, "").await.map_err(|f| f.message).unwrap();
            drop(guard);
            let load = t.elapsed();
            let pid = server_pid().unwrap();
            let backend = status().backend;
            let vram_loaded = vram_used_mb();
            let ram = ram_mb(pid);

            // La corrección de verdad, por la misma capa que usa la paleta.
            let mref = ModelRef { provider: "local".into(), model: String::new() };
            let req = crate::ai::RunRequest { action: "correct".into(), text: "Ola, mañana no podre ir a la reunion por que tengo medico. Haber si lo movemos al jueves.".into(), language: None, instruction: None };
            let user = crate::ai::prompts::user_message(&req, &[], None);
            let deltas = std::sync::atomic::AtomicUsize::new(0);
            let t = Instant::now();
            let out = crate::ai::chat(&s, &mref, crate::ai::prompts::SYSTEM, &user, &|_| {
                deltas.fetch_add(1, Ordering::Relaxed);
            })
            .await
            .map_err(|f| f.message)
            .unwrap();
            let latency = t.elapsed();
            let tps = tokens_per_second(&base, &id).await;
            println!(
                "[{}] backend {backend:?} · carga {:.1} s · corrección {} ms ({} deltas) · {tps:.0} tokens/s · RAM {ram:?} MB · VRAM {vram_before:?} → {vram_loaded:?} MB\n  → {out}",
                if gpu { "GPU" } else { "CPU" },
                load.as_secs_f64(),
                latency.as_millis(),
                deltas.load(Ordering::Relaxed),
            );
            assert!(deltas.load(Ordering::Relaxed) > 1, "no llega en streaming");
            assert!(out.contains("mañana") && out.to_lowercase().contains("hola"), "{out}");
            assert_eq!(status().state, "ready");
            if gpu {
                assert!(backend.as_deref().unwrap_or("").starts_with("GPU (Vulkan)"), "{backend:?}");
            } else {
                assert_eq!(backend.as_deref(), Some("CPU"));
            }

            unload();
            tokio::time::sleep(Duration::from_millis(1500)).await;
            assert!(!alive(pid), "el servidor sigue vivo");
            assert!(server_pid().is_none());
            let vram_after = vram_used_mb();
            println!("  liberado: proceso {pid} cerrado · VRAM {vram_after:?} MB");
            if let (Some(b), Some(a)) = (vram_before, vram_after) {
                assert!(a <= b + 150, "la VRAM no se ha liberado: {b} → {a} MB");
            }
        }
    }
}
