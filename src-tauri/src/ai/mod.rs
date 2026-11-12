//! IA de texto con varios proveedores (Claude, OpenAI, DeepSeek, Gemini,
//! OpenRouter, personalizado compatible con OpenAI) y la IA local de Winkit
//! (llama.cpp descargado al activarla). Ver los comandos en src/lib/api.ts
//! (`ai`, `localAi`) y los tipos en src/shared/types.ts.
//!
//! Cada herramienta (`writing`, `understand`, `launcher`, `assistant`) usa su
//! modelo o el predeterminado: `Settings::ai.for_tool`.

pub mod anthropic;
pub mod commands;
pub mod error;
pub mod gemini;
pub mod local;
pub mod openai;
pub mod prompts;
#[cfg(test)]
mod real_tests;
mod sse;

pub use error::Failure;
pub use prompts::is_translation;

use crate::settings::{self, ModelRef, Settings};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub action: String,
    pub text: String,
    pub language: Option<String>,
    pub instruction: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunEvent {
    Start { model: String, from: Option<String>, to: Option<String> },
    Delta { text: String },
    Done { text: String },
    Error { message: String, #[serde(rename = "openSettings")] open_settings: bool },
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub hint: Option<String>,
    /// El que Winkit propone en ese proveedor (rápido y barato).
    pub recommended: bool,
}

/// Recibe el texto acumulado hasta el momento.
pub type OnDelta<'a> = &'a (dyn Fn(&str) + Send + Sync);

mod http {
    use std::sync::OnceLock;

    /// Un cliente para todo (reutiliza conexiones). Sin límite total: una
    /// respuesta larga en streaming puede tardar; solo se limita la conexión.
    pub fn client() -> reqwest::Client {
        static C: OnceLock<reqwest::Client> = OnceLock::new();
        C.get_or_init(|| {
            reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(10)).build().unwrap_or_default()
        })
        .clone()
    }
}

// ------------------------------------------------------------ streaming común

#[derive(Debug, PartialEq)]
pub enum Step {
    None,
    /// Ha llegado texto nuevo.
    Delta,
    /// Fin de la respuesta.
    Done,
}

/// Interpreta los eventos SSE de un proveedor.
pub trait Decoder {
    fn event(&mut self, ev: &sse::Event) -> Result<Step, Failure>;
    /// El texto visible acumulado.
    fn text(&self) -> &str;
    /// Al terminar: rechazos y demás.
    fn end(&mut self) -> Result<(), Failure>;
}

async fn drive(res: reqwest::Response, provider: &str, dec: &mut impl Decoder, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    drive_stream(res.bytes_stream(), provider, dec, on_delta).await
}

/// Lee el stream hasta el final (o hasta que se cancele la tarea: al soltar el
/// futuro se corta la conexión).
pub async fn drive_stream<B: AsRef<[u8]>, E, D: Decoder>(
    mut bytes: impl Stream<Item = Result<B, E>> + Unpin,
    provider: &str,
    dec: &mut D,
    on_delta: OnDelta<'_>,
) -> Result<String, Failure> {
    let mut parser = sse::Parser::default();
    let mut shown = 0usize;
    // true = ha llegado el final de la respuesta.
    let mut handle = |ev: &sse::Event, dec: &mut D| -> Result<bool, Failure> {
        match dec.event(ev)? {
            Step::Delta => {
                let t = dec.text();
                if t.len() != shown && !t.is_empty() {
                    shown = t.len();
                    on_delta(t);
                }
                Ok(false)
            }
            Step::Done => Ok(true),
            Step::None => Ok(false),
        }
    };
    let mut done = false;
    'read: while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(|_| Failure::cut(provider))?;
        for ev in parser.push(chunk.as_ref()) {
            if handle(&ev, dec)? {
                done = true;
                break 'read;
            }
        }
    }
    if !done {
        if let Some(ev) = parser.finish() {
            handle(&ev, dec)?;
        }
    }
    dec.end()?;
    let text = dec.text().to_string();
    if text.trim().is_empty() {
        return Err(Failure::new(format!("{provider} ha devuelto una respuesta vacía. Prueba de nuevo.")));
    }
    Ok(text)
}

// ------------------------------------------------------------ proveedores

pub const PROVIDERS: [&str; 7] = ["anthropic", "openai", "deepseek", "gemini", "openrouter", "custom", "local"];

/// Nombre visible del proveedor, el de los errores («Anthropic», «Gemini»…).
pub fn provider_name(s: &Settings, provider: &str) -> String {
    match provider {
        "anthropic" => "Anthropic".into(),
        "openai" => "OpenAI".into(),
        "deepseek" => "DeepSeek".into(),
        "gemini" => "Gemini".into(),
        "openrouter" => "OpenRouter".into(),
        "custom" => custom_name(s),
        "local" => local::NAME.into(),
        p => p.into(),
    }
}

fn custom_name(s: &Settings) -> String {
    let n = s.ai.custom.name.trim();
    if n.is_empty() {
        "Servidor propio".into()
    } else {
        n.into()
    }
}

/// Nombre para la lista de Ajustes.
fn provider_title(s: &Settings, provider: &str) -> String {
    match provider {
        "anthropic" => "Claude (Anthropic)".into(),
        "gemini" => "Google Gemini".into(),
        p => provider_name(s, p),
    }
}

pub fn key_url(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "anthropic" => "https://platform.claude.com/settings/keys",
        "openai" => "https://platform.openai.com/api-keys",
        "deepseek" => "https://platform.deepseek.com/api_keys",
        "gemini" => "https://aistudio.google.com/apikey",
        "openrouter" => "https://openrouter.ai/settings/keys",
        _ => return None,
    })
}

fn needs_key(provider: &str) -> bool {
    !matches!(provider, "custom" | "local")
}

/// ¿Se puede usar ya? Key puesta, servidor configurado o IA local instalada.
pub fn ready(s: &Settings, provider: &str) -> bool {
    match provider {
        "custom" => !s.ai.custom.base_url.trim().is_empty(),
        "local" => local::ready(s),
        p if PROVIDERS.contains(&p) => settings::provider_key(p).is_some(),
        _ => false,
    }
}

/// Qué falta para poder usar ese proveedor.
pub fn not_ready_message(s: &Settings, provider: &str) -> String {
    match provider {
        "custom" => "Configura tu servidor propio en Ajustes → IA.".into(),
        "local" if !s.ai.local.enabled => "Activa la IA local en Ajustes → IA.".into(),
        "local" => "Instala un modelo de IA local en Ajustes → IA.".into(),
        p => format!("Añade tu API key de {} en Ajustes → IA para empezar.", provider_name(s, p)),
    }
}

/// «Claude Haiku 5.5», «OpenAI · gpt-…», «IA local · Gemma 4 E2B».
pub fn label(s: &Settings, m: &ModelRef) -> String {
    match m.provider.as_str() {
        "anthropic" => format!("Claude {}", settings::model_label(m)),
        "local" => {
            let id = local::resolve_model(&s.ai.local, &m.model).map_or(m.model.as_str(), |x| x.id);
            format!("{} · {}", local::NAME, local::model_name(id))
        }
        p => format!("{} · {}", provider_name(s, p), m.model),
    }
}

pub fn tool_for_action(action: &str) -> &'static str {
    if action == "understand" {
        "understand"
    } else {
        "writing"
    }
}

/// Raíz de la API de un proveedor compatible con OpenAI.
fn base_url(s: &Settings, provider: &str) -> Result<String, Failure> {
    Ok(match provider {
        "openai" => openai::OPENAI_URL.into(),
        "deepseek" => openai::DEEPSEEK_URL.into(),
        "openrouter" => openai::OPENROUTER_URL.into(),
        _ => {
            let url = s.ai.custom.base_url.trim().trim_end_matches('/').to_string();
            if url.is_empty() {
                return Err(Failure::settings(not_ready_message(s, "custom")));
            }
            url
        }
    })
}

fn endpoint(s: &Settings, provider: &'static str, key: Option<String>) -> Result<openai::Endpoint, Failure> {
    Ok(openai::Endpoint { provider, name: provider_name(s, provider), base_url: base_url(s, provider)?, key })
}

/// Con la key guardada (obligatoria salvo en el servidor propio).
fn saved_endpoint(s: &Settings, provider: &'static str) -> Result<openai::Endpoint, Failure> {
    let key = settings::provider_key(provider);
    if needs_key(provider) && key.is_none() {
        return Err(Failure::settings(not_ready_message(s, provider)));
    }
    endpoint(s, provider, key)
}

fn static_provider(p: &str) -> Option<&'static str> {
    PROVIDERS.iter().copied().find(|x| *x == p)
}

fn key_for(s: &Settings, provider: &str) -> Result<String, Failure> {
    settings::provider_key(provider).ok_or_else(|| Failure::settings(not_ready_message(s, provider)))
}

/// Una petición (sistema + un mensaje del usuario) al modelo indicado, en streaming.
pub async fn chat(s: &Settings, m: &ModelRef, system: &str, user: &str, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    let provider = static_provider(&m.provider).ok_or_else(|| Failure::settings(format!("Proveedor desconocido: {}.", m.provider)))?;
    if m.model.trim().is_empty() && provider != "local" {
        return Err(Failure::settings(format!("Elige un modelo de {} en Ajustes → IA.", provider_name(s, provider))));
    }
    match provider {
        "anthropic" => anthropic::stream(&key_for(s, provider)?, &m.model, system, user, on_delta).await,
        "gemini" => gemini::stream(&key_for(s, provider)?, &m.model, system, user, on_delta).await,
        "local" => {
            let (base_url, model, _in_use) = local::ensure_running(s, &m.model).await?;
            let ep = openai::Endpoint { provider: "local", name: local::NAME.into(), base_url, key: None };
            openai::stream(&ep, &model, system, user, on_delta).await
        }
        p => openai::stream(&saved_endpoint(s, p)?, &m.model, system, user, on_delta).await,
    }
}

/// Transforma el texto con una acción de la paleta.
pub async fn run(s: &Settings, req: &RunRequest, fallback: Option<&str>, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    let m = s.ai.for_tool(tool_for_action(&req.action));
    let user = prompts::user_message(req, &s.custom_actions, fallback);
    let text = chat(s, &m, prompts::SYSTEM, &user, on_delta).await?;
    Ok(prompts::finish(req, &text))
}

/// Pregunta suelta desde el buscador.
pub async fn ask(s: &Settings, question: &str, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    chat(s, &s.ai.for_tool("launcher"), prompts::ASK_SYSTEM, question, on_delta).await
}

/// Comprueba una key contra el proveedor (una llamada barata: listar modelos).
pub async fn test_key(s: &Settings, provider: &str, key: &str) -> Result<(), Failure> {
    match provider {
        "anthropic" => anthropic::test_key(key).await,
        "gemini" => gemini::test_key(key).await,
        "openai" | "deepseek" | "openrouter" | "custom" => {
            let p = static_provider(provider).unwrap_or("custom");
            openai::test_key(&endpoint(s, p, Some(key.to_string()))?).await
        }
        _ => Err(Failure::new("Este proveedor no usa API key.")),
    }
}

/// Modelos de un proveedor, consultados en vivo (los locales: los instalados).
pub async fn models(s: &Settings, provider: &str) -> Result<Vec<ModelInfo>, Failure> {
    match provider {
        "anthropic" => match settings::provider_key("anthropic") {
            Some(k) => anthropic::models(&k).await,
            None => Ok(anthropic::filter_models(&[])),
        },
        "gemini" => gemini::models(&key_for(s, provider)?).await,
        "local" => Ok(local::catalog::MODELS
            .iter()
            .filter(|m| local::is_installed(m))
            .map(|m| ModelInfo {
                id: m.id.into(),
                name: m.name.into(),
                hint: Some(format!("{} GB · gratis", format!("{:.1}", m.size as f64 / 1e9).replace('.', ","))),
                recommended: m.recommended,
            })
            .collect()),
        p => {
            let p = static_provider(p).ok_or_else(|| Failure::new("Proveedor desconocido."))?;
            // La lista de OpenRouter es pública: se puede ver antes de poner la key.
            let ep = match saved_endpoint(s, p) {
                Err(_) if p == "openrouter" => endpoint(s, p, None)?,
                r => r?,
            };
            openai::models(&ep).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Ai;

    #[test]
    fn cada_herramienta_usa_su_modelo() {
        let mut ai = Ai::default();
        assert_eq!(ai.for_tool("writing"), ModelRef::default());
        ai.tools.insert("understand".into(), ModelRef { provider: "openai".into(), model: "gpt-x".into() });
        // Una entrada sin proveedor cuenta como «usar el predeterminado».
        ai.tools.insert("launcher".into(), ModelRef { provider: String::new(), model: String::new() });
        assert_eq!(ai.for_tool(tool_for_action("understand")).provider, "openai");
        assert_eq!(ai.for_tool(tool_for_action("correct")), ai.default);
        assert_eq!(ai.for_tool("launcher"), ai.default);
        assert_eq!(tool_for_action("translate"), "writing");
    }

    #[test]
    fn etiquetas_y_avisos() {
        let mut s = Settings::default();
        assert_eq!(label(&s, &ModelRef::default()), "Claude Haiku 5.5");
        assert_eq!(label(&s, &ModelRef { provider: "deepseek".into(), model: "deepseek-flash".into() }), "DeepSeek · deepseek-flash");
        s.ai.custom.name = "Mi servidor".into();
        assert_eq!(label(&s, &ModelRef { provider: "custom".into(), model: "llama".into() }), "Mi servidor · llama");
        assert!(label(&s, &ModelRef { provider: "local".into(), model: "gemma-4-e2b".into() }).starts_with("IA local · Gemma 4 E2B"));
        assert!(!ready(&s, "custom"));
        s.ai.custom.base_url = "http://x/v1".into();
        assert!(ready(&s, "custom"));
        assert!(not_ready_message(&s, "openai").contains("API key de OpenAI"));
        assert!(not_ready_message(&s, "local").contains("Activa"));
        assert!(!ready(&s, "nadie"));
    }

    struct Fake(String, bool);
    impl Decoder for Fake {
        fn event(&mut self, ev: &sse::Event) -> Result<Step, Failure> {
            match ev.data.as_str() {
                "fin" => Ok(Step::Done),
                "error" => Err(Failure::new("mal")),
                d => {
                    self.0.push_str(d);
                    Ok(Step::Delta)
                }
            }
        }
        fn text(&self) -> &str {
            &self.0
        }
        fn end(&mut self) -> Result<(), Failure> {
            if self.1 {
                Err(Failure::refusal("X"))
            } else {
                Ok(())
            }
        }
    }

    fn drive_chunks(chunks: Vec<Result<&'static str, ()>>, refuse: bool) -> (Result<String, Failure>, Vec<String>) {
        let seen = std::sync::Mutex::new(Vec::new());
        let on = |t: &str| seen.lock().unwrap().push(t.to_string());
        let stream = futures_util::stream::iter(chunks.into_iter().map(|c| c.map(|s| s.as_bytes())));
        let r = tauri::async_runtime::block_on(drive_stream(stream, "X", &mut Fake(String::new(), refuse), &on));
        (r, seen.into_inner().unwrap())
    }

    #[test]
    fn el_stream_acumula_y_para_en_el_final() {
        let (r, seen) = drive_chunks(vec![Ok("data: Ho"), Ok("\n\ndata: la\n\ndata: fin\n\ndata: tarde\n\n")], false);
        assert_eq!(r.unwrap(), "Hola");
        assert_eq!(seen, ["Ho", "Hola"]);
    }

    #[test]
    fn errores_a_mitad_del_stream() {
        let (r, _) = drive_chunks(vec![Ok("data: Ho\n\n"), Ok("data: error\n\n")], false);
        assert_eq!(r.unwrap_err().message, "mal");
        // Se corta la conexión.
        let (r, _) = drive_chunks(vec![Ok("data: Ho\n\n"), Err(())], false);
        assert_eq!(r.unwrap_err(), Failure::cut("X"));
        let (r, _) = drive_chunks(vec![Ok("data: Ho\n\n")], true);
        assert_eq!(r.unwrap_err(), Failure::refusal("X"));
        // Sin nada que pegar, mejor un error que borrar la selección.
        let (r, _) = drive_chunks(vec![Ok("data: fin\n\n")], false);
        assert!(r.unwrap_err().message.contains("vacía"));
        // El último evento sin línea en blanco también cuenta.
        let (r, _) = drive_chunks(vec![Ok("data: Hola")], false);
        assert_eq!(r.unwrap(), "Hola");
    }
}
