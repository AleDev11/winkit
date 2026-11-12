//! Comandos de IA para Ajustes (ver `ai` y `localAi` en src/lib/api.ts).

use super::local::{self, LocalAiStatus, LocalModel};
use super::{ModelInfo, PROVIDERS};
use crate::settings::{self, ModelRef};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    id: &'static str,
    name: String,
    needs_key: bool,
    has_key: bool,
    key_from_env: bool,
    key_url: Option<&'static str>,
    ready: bool,
}

#[derive(Serialize)]
pub struct KeyResult {
    ok: bool,
    message: String,
}

#[derive(Serialize)]
pub struct AiTestResult {
    ok: bool,
    message: String,
    ms: u64,
}

pub fn providers() -> Vec<ProviderInfo> {
    let s = settings::get();
    PROVIDERS
        .iter()
        .map(|&id| ProviderInfo {
            id,
            name: super::provider_title(&s, id),
            needs_key: super::needs_key(id),
            has_key: id != "local" && settings::provider_key(id).is_some(),
            key_from_env: settings::provider_key_from_env(id).is_some(),
            key_url: super::key_url(id),
            ready: super::ready(&s, id),
        })
        .collect()
}

#[tauri::command]
pub async fn ai_providers() -> Vec<ProviderInfo> {
    providers()
}

#[tauri::command]
pub async fn ai_save_key(provider: String, key: String) -> KeyResult {
    let key = key.trim().to_string();
    let s = settings::get();
    if provider == "local" || !PROVIDERS.contains(&provider.as_str()) {
        return KeyResult { ok: false, message: "Este proveedor no usa API key.".into() };
    }
    if key.is_empty() {
        return KeyResult { ok: false, message: format!("Pega tu API key de {}.", super::provider_name(&s, &provider)) };
    }
    match super::test_key(&s, &provider, &key).await {
        Ok(()) => match settings::set_provider_key(&provider, &key) {
            Ok(()) => KeyResult { ok: true, message: "La API key funciona y está guardada.".into() },
            Err(e) => KeyResult { ok: false, message: e },
        },
        Err(f) => KeyResult { ok: false, message: f.message },
    }
}

#[tauri::command]
pub fn ai_remove_key(provider: String) {
    if PROVIDERS.contains(&provider.as_str()) && provider != "local" {
        settings::remove_provider_key(&provider);
    }
}

#[tauri::command]
pub async fn ai_models(provider: String) -> Result<Vec<ModelInfo>, String> {
    super::models(&settings::get(), &provider).await.map_err(|f| f.message)
}

/// Una respuesta corta para ver que el modelo contesta (y cuánto tarda).
#[tauri::command]
pub async fn ai_test(model: ModelRef) -> AiTestResult {
    let s = settings::get();
    let t = std::time::Instant::now();
    let r = super::chat(
        &s,
        &model,
        "Eres una prueba de conexión. Responde con una sola frase corta, en español.",
        "Saluda y di qué modelo eres en menos de diez palabras.",
        &|_| {},
    )
    .await;
    let ms = t.elapsed().as_millis() as u64;
    match r {
        Ok(text) => AiTestResult { ok: true, message: text.trim().to_string(), ms },
        Err(f) => AiTestResult { ok: false, message: f.message, ms },
    }
}

#[tauri::command]
pub fn ai_model_for(tool: String) -> ModelRef {
    settings::get().ai.for_tool(&tool)
}

// ------------------------------------------------------------ IA local

#[tauri::command]
pub fn local_ai_catalog() -> Vec<LocalModel> {
    local::catalog()
}

#[tauri::command]
pub async fn local_ai_status() -> LocalAiStatus {
    local::status()
}

#[tauri::command]
pub async fn local_ai_install(model: String) -> Result<(), String> {
    local::install(&model).await
}

#[tauri::command]
pub fn local_ai_cancel() {
    local::cancel();
}

#[tauri::command]
pub async fn local_ai_remove(model: String) -> Result<(), String> {
    local::remove(&model)
}

#[tauri::command]
pub async fn local_ai_unload() {
    local::unload();
}

#[tauri::command]
pub fn local_ai_open_folder() {
    local::open_folder();
}
