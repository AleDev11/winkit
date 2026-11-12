//! Ajustes en %APPDATA%\Winkit\settings.json (la misma carpeta que usaba la
//! versión de Electron, así que se conservan). La API key va aparte, cifrada
//! con DPAPI, y nunca sale del proceso de Rust.

use crate::win::dpapi;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub model: String,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "claude-haiku-5-5".into(),
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
        }
    }
}

pub const MODELS: [(&str, &str); 3] =
    [("claude-haiku-5-5", "Haiku 5.5"), ("claude-sonnet-5-5", "Sonnet 5.5"), ("claude-opus-5-5", "Opus 5.5")];

impl Settings {
    pub fn model_label(&self) -> String {
        MODELS.iter().find(|(id, _)| *id == self.model).map_or(self.model.clone(), |(_, l)| l.to_string())
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

// ---------------------------------------------------------------- API key

fn key_file() -> PathBuf {
    dir().join("apikey.bin")
}

pub fn key_from_env() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.trim().is_empty())
}

pub fn api_key() -> Option<String> {
    if let Some(k) = key_from_env() {
        return Some(k);
    }
    let blob = std::fs::read(key_file()).ok()?;
    String::from_utf8(dpapi::unprotect(&blob)?).ok().filter(|k| !k.is_empty())
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    let blob = dpapi::protect(key.trim().as_bytes()).ok_or("No se ha podido cifrar la API key.")?;
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(key_file(), blob).map_err(|e| e.to_string())
}

/// La versión de Electron guardaba la key con `safeStorage`: AES-256-GCM con una
/// clave protegida por DPAPI en el archivo "Local State" de Chromium. Se recupera
/// una vez y se vuelve a guardar con DPAPI directamente.
pub fn migrate_electron_key() {
    if key_file().exists() {
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
