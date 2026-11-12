//! Cliente para las APIs compatibles con OpenAI (`/chat/completions` con SSE):
//! OpenAI, DeepSeek, OpenRouter, el servidor propio y la IA local (llama-server).

use super::error::{api_error, status_failure, Failure};
use super::{http, sse, Decoder, ModelInfo, OnDelta, Step};
use serde_json::{json, Map, Value};

/// Adónde se manda la petición.
pub struct Endpoint {
    /// "openai", "deepseek", "openrouter", "custom" o "local".
    pub provider: &'static str,
    /// Nombre visible en los errores.
    pub name: String,
    /// Raíz de la API, sin barra final (".../v1").
    pub base_url: String,
    pub key: Option<String>,
}

pub const OPENAI_URL: &str = "https://api.openai.com/v1";
pub const DEEPSEEK_URL: &str = "https://api.deepseek.com";
pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";

impl Endpoint {
    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.auth(http::client().get(format!("{}{path}", self.base_url)))
    }
    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.auth(http::client().post(format!("{}{path}", self.base_url)))
    }
    fn auth(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(k) = self.key.as_deref().filter(|k| !k.is_empty()) {
            req = req.bearer_auth(k);
        }
        if self.provider == "openrouter" {
            // Cabeceras recomendadas por OpenRouter para identificar la app.
            req = req.header("HTTP-Referer", "https://github.com/AleDev11/winkit").header("X-Title", "Winkit");
        }
        req
    }
}

/// Modelos de razonamiento de OpenAI (familia o-, gpt-5 y posteriores): no
/// aceptan `temperature` y piden `max_completion_tokens`.
fn openai_reasoning(model: &str) -> bool {
    let m = model.rsplit('/').next().unwrap_or(model);
    let o_series = m.starts_with('o') && m[1..].starts_with(|c: char| c.is_ascii_digit());
    o_series || version_after(m, "gpt-").is_some_and(|v| v.0 >= 5)
}

/// Parámetros opcionales que dependen del proveedor y del modelo. Si el
/// servidor rechaza alguno con un 400, se reintenta sin él.
fn extras(provider: &str, model: &str) -> Vec<(&'static str, Value)> {
    match provider {
        // Nunca `temperature`: los modelos de razonamiento la rechazan y el resto va bien sin ella.
        "openai" => {
            let mut v = vec![("max_completion_tokens", json!(16000))];
            if openai_reasoning(model) {
                v.push(("reasoning_effort", json!("low")));
            }
            v
        }
        // DeepSeek piensa por defecto: para corregir y traducir sobra.
        "deepseek" => vec![("thinking", json!({ "type": "disabled" }))],
        "openrouter" => vec![("reasoning", json!({ "effort": "low", "exclude": true }))],
        // Nuestro llama-server: sin azar. Con un modelo pequeño, corregir con
        // temperatura acaba a veces en una frase peor; así siempre sale la mejor.
        "local" => vec![("temperature", json!(0.0))],
        _ => Vec::new(),
    }
}

pub fn body(model: &str, system: &str, user: &str, extras: &[(&'static str, Value)]) -> Value {
    let mut b = Map::new();
    b.insert("model".into(), json!(model));
    b.insert("stream".into(), json!(true));
    b.insert("messages".into(), json!([{ "role": "system", "content": system }, { "role": "user", "content": user }]));
    for (k, v) in extras {
        b.insert((*k).into(), v.clone());
    }
    Value::Object(b)
}

/// ¿El 400 se queja de alguno de los parámetros opcionales? Devuelve cuáles quitar.
pub fn rejected_extras(body: &str, extras: &[(&'static str, Value)]) -> Vec<&'static str> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let param = v["error"]["param"].as_str().unwrap_or("");
    let (message, _) = api_error(body);
    let message = message.unwrap_or_default();
    extras.iter().map(|(k, _)| *k).filter(|k| param == *k || message.contains(k)).collect()
}

/// Lee los eventos de `/chat/completions`.
#[derive(Default)]
pub struct Stream {
    name: String,
    model: String,
    acc: String,
    refused: bool,
}

impl Stream {
    pub fn new(name: &str, model: &str) -> Self {
        Self { name: name.into(), model: model.into(), ..Default::default() }
    }
}

/// Algunos servidores propios devuelven el razonamiento dentro del texto
/// (`<think>…</think>` al principio): no es parte de la respuesta.
fn visible(acc: &str) -> &str {
    let t = acc.trim_start();
    if !t.starts_with("<think>") {
        return acc;
    }
    match t.find("</think>") {
        Some(end) => t[end + "</think>".len()..].trim_start(),
        None => "",
    }
}

impl Decoder for Stream {
    fn event(&mut self, ev: &sse::Event) -> Result<Step, Failure> {
        if ev.data.trim() == "[DONE]" {
            return Ok(Step::Done);
        }
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else { return Ok(Step::None) };
        if v["error"].is_object() {
            // Error a mitad del stream (OpenRouter lo hace así): con código HTTP o sin él.
            let code = v["error"]["code"].as_u64().unwrap_or(0) as u16;
            return Err(if (400..600).contains(&code) {
                status_failure(&self.name, &self.model, code, &ev.data)
            } else {
                Failure::new(api_error(&ev.data).0.unwrap_or_else(|| format!("Error de {}.", self.name)))
            });
        }
        let mut step = Step::None;
        for c in v["choices"].as_array().into_iter().flatten() {
            let d = &c["delta"];
            if let Some(t) = d["content"].as_str() {
                if !t.is_empty() {
                    self.acc.push_str(t);
                    step = Step::Delta;
                }
            }
            if d["refusal"].as_str().is_some_and(|r| !r.is_empty()) {
                self.refused = true;
            }
            match c["finish_reason"].as_str() {
                Some("content_filter") => self.refused = true,
                Some("error") => return Err(Failure::new(format!("Error de {}.", self.name))),
                _ => {}
            }
        }
        Ok(step)
    }

    fn text(&self) -> &str {
        visible(&self.acc)
    }

    fn end(&mut self) -> Result<(), Failure> {
        if self.refused {
            return Err(Failure::refusal(&self.name));
        }
        Ok(())
    }
}

pub async fn stream(ep: &Endpoint, model: &str, system: &str, user: &str, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    let mut extras = extras(ep.provider, model);
    for _ in 0..3 {
        let res = ep
            .post("/chat/completions")
            .json(&body(model, system, user, &extras))
            .send()
            .await
            .map_err(|_| Failure::offline(&ep.name))?;
        let status = res.status().as_u16();
        if status == 200 {
            return super::drive(res, &ep.name, &mut Stream::new(&ep.name, model), on_delta).await;
        }
        let text = res.text().await.unwrap_or_default();
        let drop = if status == 400 { rejected_extras(&text, &extras) } else { Vec::new() };
        if drop.is_empty() {
            return Err(status_failure(&ep.name, model, status, &text));
        }
        crate::log::info("ai", &format!("{} rechaza {drop:?}: se reintenta sin ellos", ep.name));
        let had_completion = drop.contains(&"max_completion_tokens");
        extras.retain(|(k, _)| !drop.contains(k));
        if had_completion {
            extras.push(("max_tokens", json!(16000)));
        }
    }
    Err(Failure::new(format!("{} rechaza la petición.", ep.name)))
}

/// `GET /models` con la key: para validarla y para listar modelos.
pub async fn list_raw(ep: &Endpoint) -> Result<Value, Failure> {
    let res = ep.get("/models").send().await.map_err(|_| Failure::offline(&ep.name))?;
    let status = res.status().as_u16();
    let text = res.text().await.unwrap_or_default();
    if status != 200 {
        return Err(status_failure(&ep.name, "", status, &text));
    }
    serde_json::from_str(&text).map_err(|_| Failure::new(format!("Respuesta inesperada de {}.", ep.name)))
}

/// Comprueba la key. `/models` de OpenRouter es público: allí se usa `/key`.
pub async fn test_key(ep: &Endpoint) -> Result<(), Failure> {
    if ep.provider != "openrouter" {
        return list_raw(ep).await.map(|_| ());
    }
    let res = ep.get("/key").send().await.map_err(|_| Failure::offline(&ep.name))?;
    let status = res.status().as_u16();
    if status == 200 {
        Ok(())
    } else {
        Err(status_failure(&ep.name, "", status, &res.text().await.unwrap_or_default()))
    }
}

pub async fn models(ep: &Endpoint) -> Result<Vec<ModelInfo>, Failure> {
    let v = list_raw(ep).await?;
    Ok(filter_models(ep.provider, &v))
}

// ------------------------------------------------------------ filtrado de modelos

/// Números de versión tras un prefijo: «gpt-5.5-mini» con "gpt-" → (5, 5).
pub fn version_after(id: &str, prefix: &str) -> Option<(u32, u32)> {
    let rest = id.strip_prefix(prefix)?;
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let mut parts = num.split('.').filter(|p| !p.is_empty());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

/// «…-2025-08-07» o «…-0613»: instantáneas fechadas, repetidas con el alias sin fecha.
fn is_snapshot(id: &str) -> bool {
    let tail: Vec<&str> = id.rsplit('-').take(3).collect();
    let digits = |s: &str, n: usize| s.len() == n && s.chars().all(|c| c.is_ascii_digit());
    (tail.len() == 3 && digits(tail[0], 2) && digits(tail[1], 2) && digits(tail[2], 4)) || tail.first().is_some_and(|t| digits(t, 4))
}

const NOT_CHAT: [&str; 14] = [
    "audio", "realtime", "transcribe", "tts", "image", "dall-e", "embedding", "moderation", "whisper", "search", "instruct", "codex", "computer-use", "deep-research",
];

fn openai_chat(id: &str) -> bool {
    let chat_family = id.starts_with("gpt-") || id.starts_with("chatgpt-") || (id.starts_with('o') && id[1..].starts_with(|c: char| c.is_ascii_digit()));
    chat_family && !NOT_CHAT.iter().any(|w| id.contains(w)) && !is_snapshot(id)
}

/// El más reciente con ese patrón («gpt-N-mini»): rápido y barato.
fn newest<'a>(ids: &[&'a str], prefix: &str, suffix: &str) -> Option<&'a str> {
    ids.iter()
        .filter(|id| id.ends_with(suffix) && !id.contains("preview") && !id.contains(":free"))
        .filter_map(|id| Some((version_after(id, prefix)?, *id)))
        .filter(|(_, id)| {
            // Exactamente prefijo + versión + sufijo (sin variantes intermedias).
            let v: String = id[prefix.len()..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            &id[prefix.len() + v.len()..] == suffix
        })
        .max()
        .map(|(_, id)| id)
}

fn price(per_token: &Value) -> Option<f64> {
    per_token.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| per_token.as_f64())
}

/// «0,10 $ / 0,40 $ por millón de tokens» o «Gratis».
fn price_hint(pricing: &Value) -> Option<String> {
    let (p, c) = (price(&pricing["prompt"])?, price(&pricing["completion"])?);
    if p < 0.0 || c < 0.0 {
        return None;
    }
    if p == 0.0 && c == 0.0 {
        return Some("Gratis".into());
    }
    let fmt = |x: f64| format!("{:.2}", x * 1e6).replace('.', ",");
    Some(format!("{} $ / {} $ por millón de tokens", fmt(p), fmt(c)))
}

pub fn filter_models(provider: &str, v: &Value) -> Vec<ModelInfo> {
    let data: Vec<&Value> = v["data"].as_array().into_iter().flatten().collect();
    let mut out: Vec<ModelInfo> = Vec::new();
    for m in &data {
        let Some(id) = m["id"].as_str() else { continue };
        let keep = match provider {
            "openai" => openai_chat(id),
            "openrouter" => {
                let arch = &m["architecture"];
                let has = |k: &str| arch[k].as_array().is_none_or(|a| a.iter().any(|x| x == "text"));
                let chat = m["endpoints"].as_array().is_none_or(|e| e.iter().any(|x| x.as_str().is_some_and(|s| s.ends_with("/chat/completions"))));
                has("input_modalities") && has("output_modalities") && chat && !id.contains("embedding")
            }
            _ => !id.contains("embed") && !id.contains("rerank") && !id.contains("whisper") && !id.contains("tts"),
        };
        if !keep {
            continue;
        }
        let name = m["name"].as_str().filter(|_| provider == "openrouter").unwrap_or(id).to_string();
        let hint = if provider == "openrouter" { price_hint(&m["pricing"]) } else { None };
        out.push(ModelInfo { id: id.into(), name, hint, recommended: false });
    }
    let ids: Vec<&str> = out.iter().map(|m| m.id.as_str()).collect();
    let pick = match provider {
        "openai" => newest(&ids, "gpt-", "-mini").or_else(|| newest(&ids, "gpt-", "-nano")),
        "deepseek" => ids.iter().find(|i| i.contains("flash")).or_else(|| ids.iter().find(|i| i.contains("chat"))).copied(),
        "openrouter" => newest(&ids, "google/gemini-", "-flash-lite").or_else(|| newest(&ids, "openai/gpt-", "-mini")),
        _ => None,
    }
    .map(String::from);
    // Orden: el recomendado, luego por versión (lo nuevo arriba) y por nombre.
    let key = |m: &ModelInfo| {
        let base = m.id.rsplit('/').next().unwrap_or(&m.id).to_string();
        let ver = version_after(&base, "gpt-").or_else(|| version_after(&base, "gemini-")).or_else(|| version_after(&base, "deepseek-v")).unwrap_or((0, 0));
        (std::cmp::Reverse(ver), m.name.to_lowercase())
    };
    if provider == "openrouter" {
        out.sort_by_key(|m| m.name.to_lowercase());
    } else {
        out.sort_by_key(key);
    }
    if let Some(p) = pick {
        if let Some(i) = out.iter().position(|m| m.id == p) {
            let mut m = out.remove(i);
            m.recommended = true;
            m.hint = Some(match m.hint {
                Some(h) => format!("Recomendado · {h}"),
                None => "Recomendado: rápido y barato".into(),
            });
            out.insert(0, m);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(data: &str) -> sse::Event {
        sse::Event { event: None, data: data.into() }
    }

    #[test]
    fn lee_el_stream() {
        let mut s = Stream::new("OpenAI", "m");
        assert_eq!(s.event(&ev(r#"{"choices":[{"delta":{"role":"assistant","content":""}}]}"#)).unwrap(), Step::None);
        assert_eq!(s.event(&ev(r#"{"choices":[{"delta":{"content":"Hola"}}]}"#)).unwrap(), Step::Delta);
        // DeepSeek manda el razonamiento aparte: no se enseña.
        assert_eq!(s.event(&ev(r#"{"choices":[{"delta":{"reasoning_content":"hmm"}}]}"#)).unwrap(), Step::None);
        s.event(&ev(r#"{"choices":[{"delta":{"content":" mundo"},"finish_reason":"stop"}]}"#)).unwrap();
        assert_eq!(s.event(&ev("[DONE]")).unwrap(), Step::Done);
        assert_eq!(s.text(), "Hola mundo");
        assert!(s.end().is_ok());
    }

    #[test]
    fn errores_a_mitad_y_rechazos() {
        let mut s = Stream::new("OpenRouter", "m");
        s.event(&ev(r#"{"choices":[{"delta":{"content":"Ho"}}]}"#)).unwrap();
        let e = s.event(&ev(r#"{"error":{"code":429,"message":"Rate limited"},"choices":[{"finish_reason":"error"}]}"#)).unwrap_err();
        assert!(e.message.contains("límite de uso de OpenRouter"));
        let e = Stream::new("X", "m").event(&ev(r#"{"error":{"message":"Boom"}}"#)).unwrap_err();
        assert_eq!(e.message, "Boom");

        let mut s = Stream::new("OpenAI", "m");
        s.event(&ev(r#"{"choices":[{"delta":{"refusal":"I can't"}}]}"#)).unwrap();
        assert_eq!(s.end().unwrap_err(), Failure::refusal("OpenAI"));
        let mut s = Stream::new("Azure", "m");
        s.event(&ev(r#"{"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#)).unwrap();
        assert!(s.end().is_err());
    }

    #[test]
    fn oculta_el_razonamiento_en_el_texto() {
        assert_eq!(visible("<think>a ver"), "");
        assert_eq!(visible("<think>a ver</think>\n\nHola"), "Hola");
        assert_eq!(visible("Hola <think>"), "Hola <think>");
    }

    #[test]
    fn parametros_por_proveedor() {
        let e = extras("openai", "gpt-5.5-mini");
        assert!(e.iter().any(|(k, _)| *k == "max_completion_tokens") && e.iter().any(|(k, _)| *k == "reasoning_effort"));
        assert!(!extras("openai", "gpt-4.1").iter().any(|(k, _)| *k == "reasoning_effort"));
        assert!(openai_reasoning("o4-mini") && !openai_reasoning("omni-x"));
        let b = body("deepseek-flash", "S", "U", &extras("deepseek", "deepseek-flash"));
        assert_eq!(b["thinking"]["type"], "disabled");
        assert_eq!(b["messages"][0]["role"], "system");
        assert!(b.get("temperature").is_none());
        assert!(extras("custom", "x").is_empty());
        assert_eq!(extras("local", "x")[0].0, "temperature");
    }

    #[test]
    fn reintenta_sin_lo_que_rechaza() {
        let e = extras("openai", "gpt-5.5");
        let body = r#"{"error":{"message":"Unsupported parameter: 'reasoning_effort' is not supported with this model.","param":"reasoning_effort"}}"#;
        assert_eq!(rejected_extras(body, &e), ["reasoning_effort"]);
        assert!(rejected_extras(r#"{"error":{"message":"context length exceeded"}}"#, &e).is_empty());
    }

    #[test]
    fn filtra_y_ordena_los_de_openai() {
        let v = json!({ "data": [
            { "id": "gpt-4.1-mini" }, { "id": "gpt-5.5" }, { "id": "gpt-5.5-mini" }, { "id": "gpt-5.5-mini-2026-08-01" },
            { "id": "gpt-5-mini" }, { "id": "text-embedding-3-large" }, { "id": "gpt-realtime" }, { "id": "o4-mini" },
            { "id": "whisper-1" }, { "id": "gpt-5.5-mini-tts" }, { "id": "dall-e-3" }, { "id": "gpt-4o-0613" }
        ]});
        let m = filter_models("openai", &v);
        let ids: Vec<&str> = m.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids[0], "gpt-5.5-mini");
        assert!(m[0].recommended && !m[1].recommended);
        assert_eq!(&ids[1..], ["gpt-5.5", "gpt-5-mini", "gpt-4.1-mini", "o4-mini"]);
    }

    #[test]
    fn filtra_los_de_openrouter_con_precio() {
        let v = json!({ "data": [
            { "id": "google/gemini-3.5-flash-lite", "name": "Google: Gemini 3.5 Flash Lite", "architecture": { "input_modalities": ["text", "image"], "output_modalities": ["text"] }, "pricing": { "prompt": "0.0000001", "completion": "0.0000004" } },
            { "id": "x/img", "name": "X: Image", "architecture": { "input_modalities": ["text"], "output_modalities": ["image"] }, "pricing": { "prompt": "0", "completion": "0" } },
            { "id": "a/free:free", "name": "A: Free", "architecture": { "input_modalities": ["text"], "output_modalities": ["text"] }, "pricing": { "prompt": "0", "completion": "0" } }
        ]});
        let m = filter_models("openrouter", &v);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].id, "google/gemini-3.5-flash-lite");
        assert_eq!(m[0].hint.as_deref(), Some("Recomendado · 0,10 $ / 0,40 $ por millón de tokens"));
        assert_eq!(m[1].hint.as_deref(), Some("Gratis"));
    }

    #[test]
    fn deepseek_y_servidores_propios() {
        let v = json!({ "data": [{ "id": "deepseek-v4-pro" }, { "id": "deepseek-flash" }] });
        let m = filter_models("deepseek", &v);
        assert_eq!(m[0].id, "deepseek-flash");
        assert!(m[0].recommended);
        let v = json!({ "data": [{ "id": "llama-3" }, { "id": "bge-embed" }] });
        assert_eq!(filter_models("custom", &v).len(), 1);
    }

    #[test]
    fn versiones_y_fechas() {
        assert_eq!(version_after("gpt-5.5-mini", "gpt-"), Some((5, 5)));
        assert_eq!(version_after("gpt-4o", "gpt-"), Some((4, 0)));
        assert_eq!(version_after("gpt-x", "gpt-"), None);
        assert!(is_snapshot("gpt-4o-2024-08-06") && is_snapshot("gpt-4-0613") && !is_snapshot("gpt-5.5-mini"));
    }
}
