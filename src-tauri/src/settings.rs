//! Ajustes en %APPDATA%\Winkit\settings.json (la misma carpeta que usaba la
//! versión de Electron, así que se conservan). La API key va aparte, cifrada
//! con DPAPI, y nunca sale del proceso de Rust.

use crate::win::dpapi;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shortcuts {
    pub palette: String,
    pub correct: String,
    pub translate: String,
    pub understand: String,
    pub launcher: String,
    pub color_picker: String,
    pub save_clip: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            palette: "Control+Alt+X".into(),
            correct: "Control+Alt+C".into(),
            translate: "Control+Alt+T".into(),
            understand: "Control+Alt+L".into(),
            launcher: "Alt+Space".into(),
            color_picker: "Control+Alt+P".into(),
            save_clip: "Control+Alt+G".into(),
        }
    }
}

/// Clips: grabación continua de la pantalla en un búfer; el atajo guarda los últimos minutos.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Clips {
    /// Desactivado por defecto: graba la pantalla todo el rato y hay que pedirlo.
    pub enabled: bool,
    /// Minutos que se guardan al pulsar el atajo (1–30).
    pub minutes: u32,
    /// Carpeta de destino. Vacío = Vídeos\Winkit.
    pub folder: String,
    /// 30 o 60.
    pub fps: u32,
    /// "low" | "medium" | "high": tasa de bits.
    pub quality: String,
    pub system_audio: bool,
    pub microphone: bool,
    /// Monitor que se graba (id estable de `replay::monitors`). Vacío = el principal.
    pub monitor: String,
    /// Salida de la que se graba el audio del sistema (id de WASAPI). Vacío = la predeterminada.
    pub system_audio_device: String,
    /// Micrófono que se graba (id de WASAPI). Vacío = el predeterminado.
    pub microphone_device: String,
}

impl Default for Clips {
    fn default() -> Self {
        Self {
            enabled: false,
            minutes: 5,
            folder: String::new(),
            fps: 60,
            quality: "high".into(),
            system_audio: true,
            microphone: false,
            monitor: String::new(),
            system_audio_device: String::new(),
            microphone_device: String::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CustomAction {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

// ---------------------------------------------------------------- IA y voz

/// Qué modelo usa algo: proveedor + id del modelo en ese proveedor.
/// Proveedores: "anthropic", "openai", "deepseek", "gemini", "openrouter",
/// "custom" (cualquier API compatible con OpenAI) y "local" (IA local de Winkit).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelRef {
    pub provider: String,
    pub model: String,
}

impl Default for ModelRef {
    fn default() -> Self {
        Self { provider: "anthropic".into(), model: "claude-haiku-5-5".into() }
    }
}

/// Servidor propio compatible con la API de OpenAI (vLLM, LM Studio, Ollama remoto…).
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomProvider {
    /// Nombre que se ve en Winkit ("Mi servidor").
    pub name: String,
    /// Raíz de la API, p. ej. "http://192.168.1.20:8000/v1".
    pub base_url: String,
}

/// IA local: Winkit descarga el motor (llama.cpp) y el modelo al activarla.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalAi {
    pub enabled: bool,
    /// Id del modelo en el catálogo de Winkit ("" = ninguno instalado aún).
    pub model: String,
    /// Usar la GPU si la hay (Vulkan). Sin GPU o con false, CPU.
    pub gpu: bool,
    /// Tokens de contexto.
    pub context: u32,
    /// Minutos sin usarse tras los que se descarga de la memoria (0 = nunca).
    /// Importante para no ocupar VRAM mientras juegas.
    pub unload_after_minutes: u32,
}

impl Default for LocalAi {
    fn default() -> Self {
        Self { enabled: false, model: String::new(), gpu: true, context: 8192, unload_after_minutes: 5 }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Ai {
    /// El que se usa si una herramienta no tiene uno propio.
    pub default: ModelRef,
    /// Por herramienta: "writing" (paleta y atajos directos), "understand"
    /// («Entender»), "launcher" (preguntas del buscador), "assistant" (el
    /// asistente, más adelante). Si falta, se usa `default`.
    pub tools: BTreeMap<String, ModelRef>,
    pub custom: CustomProvider,
    pub local: LocalAi,
}

impl Ai {
    /// El modelo de una herramienta (el suyo o el predeterminado).
    pub fn for_tool(&self, tool: &str) -> ModelRef {
        self.tools.get(tool).filter(|m| !m.provider.is_empty()).cloned().unwrap_or_else(|| self.default.clone())
    }
}

/// Voz (texto a voz).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Voice {
    /// "windows" (sin descargas), "kokoro" (local, se descarga), "elevenlabs" o "openai".
    pub engine: String,
    /// Id de la voz en ese motor ("" = la predeterminada del motor).
    pub voice: String,
    /// Velocidad: 1.0 = normal (0.5–2.0).
    pub speed: f32,
    /// Volumen 0.0–1.0.
    pub volume: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self { engine: "windows".into(), voice: String::new(), speed: 1.0, volume: 1.0 }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub shortcuts: Shortcuts,
    pub direct_auto_replace: bool,
    pub select_all_fallback: bool,
    pub languages: Vec<String>,
    pub my_language: String,
    pub translate_target: String,
    pub custom_actions: Vec<CustomAction>,
    pub launch_at_login: bool,
    pub search_engine: String,
    pub color_format: String,
    /// Dónde salen los avisos: "bottomRight" (por defecto), "bottomLeft", "topRight",
    /// "topLeft" o "cursor" (junto al ratón).
    pub notice_position: String,
    pub clips: Clips,
    pub ai: Ai,
    pub voice: Voice,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcuts: Shortcuts::default(),
            direct_auto_replace: true,
            select_all_fallback: true,
            languages: ["Inglés", "Español", "Catalán", "Francés", "Italiano", "Alemán", "Portugués"]
                .map(String::from)
                .to_vec(),
            my_language: "Español".into(),
            translate_target: "Inglés".into(),
            custom_actions: Vec::new(),
            launch_at_login: true,
            search_engine: "google".into(),
            color_format: "hex".into(),
            notice_position: "bottomRight".into(),
            clips: Clips::default(),
            ai: Ai::default(),
            voice: Voice::default(),
        }
    }
}

pub const MODELS: [(&str, &str); 3] =
    [("claude-haiku-5-5", "Haiku 5.5"), ("claude-sonnet-5-5", "Sonnet 5.5"), ("claude-opus-5-5", "Opus 5.5")];

/// Nombre legible de un modelo («Haiku 5.5»). Los de Claude con su nombre corto; el
/// resto, con su id.
pub fn model_label(m: &ModelRef) -> String {
    if m.provider == "anthropic" {
        if let Some((_, l)) = MODELS.iter().find(|(id, _)| *id == m.model) {
            return l.to_string();
        }
    }
    m.model.clone()
}

impl Settings {
    /// El modelo predeterminado, legible. Para el de una herramienta: `model_label(&s.ai.for_tool(..))`.
    pub fn model_label(&self) -> String {
        model_label(&self.ai.default)
    }

    pub fn search_url(&self, query: &str) -> String {
        let base = match self.search_engine.as_str() {
            "duckduckgo" => "https://duckduckgo.com/?q=",
            "bing" => "https://www.bing.com/search?q=",
            _ => "https://www.google.com/search?q=",
        };
        format!("{base}{}", encode_query(query))
    }

    pub fn engine_label(&self) -> &'static str {
        match self.search_engine.as_str() {
            "duckduckgo" => "DuckDuckGo",
            "bing" => "Bing",
            _ => "Google",
        }
    }
}

fn encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("Winkit")
}

fn file() -> PathBuf {
    dir().join("settings.json")
}

static CACHE: Mutex<Option<Settings>> = Mutex::new(None);

/// Los ajustes de la versión de Electron usaban otros nombres: se traducen al leerlos.
fn migrate(mut v: Value) -> Value {
    let Some(obj) = v.as_object_mut() else { return Value::Null };
    if let Some(sc) = obj.get_mut("shortcuts").and_then(Value::as_object_mut) {
        if let Some(menu) = sc.remove("menu") {
            sc.entry("palette").or_insert(menu);
        }
        if let Some(reader) = sc.remove("reader") {
            sc.entry("understand").or_insert(reader);
        }
    }
    // 0.2.x: un solo campo "model" (siempre de Claude) → ai.default.
    if !obj.contains_key("ai") {
        if let Some(Value::String(model)) = obj.remove("model") {
            obj.insert("ai".into(), serde_json::json!({ "default": { "provider": "anthropic", "model": model } }));
        }
    }
    if !obj.contains_key("myLanguage") {
        if let Some(lang) = obj.get("readerLanguage").or_else(|| obj.get("translateFallback")).cloned() {
            obj.insert("myLanguage".into(), lang);
        }
    }
    v
}

pub fn get() -> Settings {
    let mut cache = CACHE.lock().unwrap();
    if let Some(s) = cache.as_ref() {
        return s.clone();
    }
    let loaded = std::fs::read_to_string(file())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map(migrate)
        // Un archivo corrupto no debe impedir que la app arranque: valores por defecto.
        .and_then(|v| serde_json::from_value::<Settings>(v).ok())
        .unwrap_or_default();
    *cache = Some(loaded.clone());
    loaded
}

/// Aplica un parche parcial (lo que manda Ajustes) y guarda.
pub fn save_patch(patch: Value) -> Result<Settings, String> {
    let mut current = serde_json::to_value(get()).map_err(|e| e.to_string())?;
    if let (Some(cur), Some(p)) = (current.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            cur.insert(k.clone(), v.clone());
        }
    }
    let next: Settings = serde_json::from_value(current).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(file(), serde_json::to_string_pretty(&next).unwrap()).map_err(|e| e.to_string())?;
    *CACHE.lock().unwrap() = Some(next.clone());
    Ok(next)
}

// ---------------------------------------------------------------- API keys

/// Una key por proveedor ("anthropic", "openai", "deepseek", "gemini",
/// "openrouter", "custom", "elevenlabs"), cifrada con DPAPI. La de Anthropic
/// sigue en apikey.bin para no perder la de versiones anteriores.
fn key_file_for(provider: &str) -> PathBuf {
    match provider {
        "anthropic" => dir().join("apikey.bin"),
        p => dir().join("keys").join(format!("{p}.bin")),
    }
}

/// Variable de entorno que, si existe, manda sobre la key guardada.
pub fn key_env_var(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        "elevenlabs" => "ELEVENLABS_API_KEY",
        _ => return None,
    })
}

pub fn provider_key_from_env(provider: &str) -> Option<String> {
    std::env::var(key_env_var(provider)?).ok().filter(|k| !k.trim().is_empty())
}

pub fn provider_key(provider: &str) -> Option<String> {
    if let Some(k) = provider_key_from_env(provider) {
        return Some(k);
    }
    let blob = std::fs::read(key_file_for(provider)).ok()?;
    String::from_utf8(dpapi::unprotect(&blob)?).ok().filter(|k| !k.is_empty())
}

pub fn set_provider_key(provider: &str, key: &str) -> Result<(), String> {
    let blob = dpapi::protect(key.trim().as_bytes()).ok_or("No se ha podido cifrar la API key.")?;
    let file = key_file_for(provider);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(file, blob).map_err(|e| e.to_string())
}

pub fn remove_provider_key(provider: &str) {
    let _ = std::fs::remove_file(key_file_for(provider));
}

// La de Anthropic, con los nombres de siempre.

pub fn key_from_env() -> Option<String> {
    provider_key_from_env("anthropic")
}

pub fn api_key() -> Option<String> {
    provider_key("anthropic")
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    set_provider_key("anthropic", key)
}

/// La versión de Electron guardaba la key con `safeStorage`: AES-256-GCM con una
/// clave protegida por DPAPI en el archivo "Local State" de Chromium. Se recupera
/// una vez y se vuelve a guardar con DPAPI directamente.
pub fn migrate_electron_key() {
    if key_file_for("anthropic").exists() {
        return;
    }
    let recover = || -> Option<String> {
        use aes_gcm::aead::{Aead, KeyInit};
        let state: Value = serde_json::from_str(&std::fs::read_to_string(dir().join("Local State")).ok()?).ok()?;
        let encrypted = base64::engine::general_purpose::STANDARD
            .decode(state["os_crypt"]["encrypted_key"].as_str()?)
            .ok()?;
        let aes_key = dpapi::unprotect(encrypted.strip_prefix(b"DPAPI")?)?;
        // Formato: "v10" + nonce (12) + cifrado + etiqueta GCM (16).
        let blob = std::fs::read(dir().join("anthropic.key")).ok()?;
        if blob.len() < 3 + 12 + 16 || &blob[..3] != b"v10" {
            return None;
        }
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&aes_key).ok()?;
        let nonce = aes_gcm::Nonce::try_from(&blob[3..15]).ok()?;
        let plain = cipher.decrypt(&nonce, &blob[15..]).ok()?;
        String::from_utf8(plain).ok()
    };
    if let Some(key) = recover().filter(|k| !k.trim().is_empty()) {
        let _ = set_api_key(&key);
    }
}
