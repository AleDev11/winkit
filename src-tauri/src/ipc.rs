//! Comandos que invocan las ventanas (ver src/lib/api.ts, mismos nombres).

use crate::claude::{self, RunEvent, RunRequest};
use crate::launcher::LauncherInit;
use crate::palette::PaletteInit;
use crate::picker::{PickerInit, Rgb};
use crate::shortcuts::ShortcutStatus;
use crate::win::{apps::AppEntry, clipboard, system};
use crate::{lang, launcher, palette, picker, settings, shortcuts, ui};
use serde::Serialize;
use serde_json::Value;
use tauri::ipc::{Channel, Response};
use tauri_plugin_autostart::ManagerExt;

// ------------------------------------------------------------ comunes

#[tauri::command]
pub fn ui_env() -> ui::UiEnv {
    ui::env()
}

#[tauri::command]
pub fn copy_text(text: String) {
    clipboard::write_text(&text, false);
}

#[tauri::command]
pub async fn open_settings() {
    palette::close(false);
    launcher::hide();
    ui::open_settings();
}

#[tauri::command]
pub fn detect_language(text: String) -> lang::LanguageInfo {
    lang::info(&text, &settings::get())
}

// ------------------------------------------------------------ paleta

#[tauri::command]
pub fn palette_current() -> Option<PaletteInit> {
    palette::current()
}

#[tauri::command]
pub fn palette_resize(width: f64, height: f64) {
    palette::place(width, height);
}

#[tauri::command]
pub fn palette_run(request: RunRequest, on_event: Channel<RunEvent>) {
    palette::run(request, on_event);
}

#[tauri::command]
pub fn palette_cancel() {
    palette::cancel();
}

#[tauri::command]
pub async fn palette_replace(text: String) {
    palette::replace(text).await;
}

#[tauri::command]
pub fn palette_copy(text: String) {
    clipboard::write_text(&text, false);
    palette::close(true);
}

#[tauri::command]
pub fn palette_close() {
    palette::close(true);
}

// ------------------------------------------------------------ buscador

#[tauri::command]
pub fn launcher_current() -> Option<LauncherInit> {
    launcher::current()
}

#[tauri::command]
pub fn launcher_resize(width: f64, height: f64) {
    launcher::resize(width, height);
}

#[tauri::command]
pub async fn list_apps() -> Vec<AppEntry> {
    launcher::list_apps().await
}

#[tauri::command]
pub async fn app_icon(id: String) -> Option<String> {
    launcher::icon(id).await
}

#[tauri::command]
pub fn launch_app(id: String) {
    launcher::hide();
    system::open_app(&id);
}

#[tauri::command]
pub fn web_search(query: String) {
    launcher::hide();
    system::open(&settings::get().search_url(&query));
}

#[tauri::command]
pub fn open_url(url: String) {
    launcher::hide();
    let lower = url.to_lowercase();
    let url = if lower.starts_with("http://") || lower.starts_with("https://") { url } else { format!("https://{url}") };
    system::open(&url);
}

#[tauri::command]
pub async fn system_command(id: String) {
    launcher::hide();
    match id.as_str() {
        "colorPicker" => drop(tauri::async_runtime::spawn(picker::open())),
        "understand" => palette::open_understand_empty(),
        "winkitSettings" => ui::open_settings(),
        other => {
            system::run_command(other);
        }
    }
}

#[tauri::command]
pub fn launcher_ask(question: String, on_event: Channel<RunEvent>) {
    launcher::ask(question, on_event);
}

#[tauri::command]
pub fn launcher_copy(text: String) {
    clipboard::write_text(&text, false);
    launcher::hide();
}

#[tauri::command]
pub fn launcher_hide() {
    launcher::hide();
}

// ------------------------------------------------------------ ajustes

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    settings: settings::Settings,
    has_key: bool,
    key_from_env: bool,
    shortcuts: Vec<ShortcutStatus>,
    version: String,
}

#[derive(Serialize)]
pub struct SaveResult {
    settings: settings::Settings,
    shortcuts: Vec<ShortcutStatus>,
}

#[derive(Serialize)]
pub struct KeyTestResult {
    ok: bool,
    message: String,
}

#[tauri::command]
pub fn settings_get() -> SettingsState {
    SettingsState {
        settings: settings::get(),
        has_key: settings::api_key().is_some(),
        key_from_env: settings::key_from_env().is_some(),
        shortcuts: shortcuts::status(),
        version: ui::app().package_info().version.to_string(),
    }
}

pub fn apply_autostart(enabled: bool) {
    // En desarrollo registraría el exe de depuración: solo en la app instalada.
    if cfg!(debug_assertions) {
        return;
    }
    let al = ui::app().autolaunch();
    let _ = if enabled { al.enable() } else { al.disable() };
}

#[tauri::command]
pub fn settings_save(patch: Value) -> Result<SaveResult, String> {
    let s = settings::save_patch(patch.clone())?;
    if patch.get("launchAtLogin").is_some() {
        apply_autostart(s.launch_at_login);
    }
    if patch.get("clips").is_some() {
        crate::replay::apply(&s.clips);
    }
    let shortcuts = if patch.get("shortcuts").is_some() { shortcuts::register_all() } else { shortcuts::status() };
    Ok(SaveResult { settings: s, shortcuts })
}

#[tauri::command]
pub async fn api_key_save(key: String) -> KeyTestResult {
    let key = key.trim().to_string();
    if key.is_empty() {
        return KeyTestResult { ok: false, message: "Pega tu API key de Anthropic.".into() };
    }
    match claude::test_key(&key).await {
        Ok(()) => match settings::set_api_key(&key) {
            Ok(()) => KeyTestResult { ok: true, message: "La API key funciona y está guardada.".into() },
            Err(e) => KeyTestResult { ok: false, message: e },
        },
        Err(f) => KeyTestResult { ok: false, message: f.message },
    }
}

#[tauri::command]
pub async fn api_key_test() -> KeyTestResult {
    let Some(key) = settings::api_key() else {
        return KeyTestResult { ok: false, message: "No hay ninguna API key guardada.".into() };
    };
    match claude::test_key(&key).await {
        Ok(()) => KeyTestResult { ok: true, message: "La API key funciona.".into() },
        Err(f) => KeyTestResult { ok: false, message: f.message },
    }
}

#[tauri::command]
pub fn shortcuts_suspend() {
    shortcuts::unregister_all();
}

#[tauri::command]
pub fn shortcuts_resume() -> Vec<ShortcutStatus> {
    shortcuts::register_all()
}

// ------------------------------------------------------------ selector de color

#[tauri::command]
pub fn picker_current() -> Option<PickerInit> {
    picker::current()
}

#[tauri::command]
pub fn picker_capture() -> Response {
    Response::new(picker::capture_bytes())
}

#[tauri::command]
pub fn picker_done(color: Option<Rgb>) {
    picker::done(color);
}

// ------------------------------------------------------------ clips

#[tauri::command]
pub fn clips_status() -> crate::replay::ClipsStatus {
    crate::replay::status()
}

/// Monitores conectados para elegir cuál se graba.
#[tauri::command]
pub fn clips_monitors() -> Vec<crate::replay::MonitorOption> {
    crate::replay::monitors()
}

/// Micrófonos y salidas de audio activos (asíncrono: enumerar con COM no debe
/// bloquear el hilo de la ventana).
#[tauri::command]
pub async fn clips_audio_devices() -> crate::replay::AudioDevices {
    crate::replay::audio_devices()
}

/// Nivel del micrófono elegido, para el medidor de Ajustes (se consulta ~10 veces por segundo).
#[tauri::command]
pub async fn clips_mic_level(device: String) -> f32 {
    crate::replay::mic_level(&device)
}

/// Selector de carpeta nativo; guarda la elegida en ajustes.
#[tauri::command]
pub async fn clips_choose_folder() -> Option<String> {
    let current = crate::replay::resolved_folder(&settings::get().clips);
    let picked = tauri::async_runtime::spawn_blocking(move || crate::win::dialog::pick_folder(&current)).await.ok().flatten()?;
    let path = picked.to_string_lossy().to_string();
    let mut clips = settings::get().clips;
    clips.folder = path.clone();
    let _ = settings::save_patch(serde_json::json!({ "clips": clips }));
    Some(path)
}

#[tauri::command]
pub fn clips_open_folder() {
    let folder = crate::replay::resolved_folder(&settings::get().clips);
    let _ = std::fs::create_dir_all(&folder);
    system::open(&folder.to_string_lossy());
}

#[tauri::command]
pub async fn clips_save() -> Result<String, String> {
    crate::replay::save().await.map(|p| p.to_string_lossy().to_string())
}

// ------------------------------------------------------------ actualizaciones

#[tauri::command]
pub fn update_status() -> crate::updater::UpdateStatus {
    crate::updater::status()
}

#[tauri::command]
pub async fn update_check() -> crate::updater::UpdateStatus {
    crate::updater::check().await
}

#[tauri::command]
pub fn update_restart() -> Result<(), String> {
    crate::updater::install_and_restart()
}
