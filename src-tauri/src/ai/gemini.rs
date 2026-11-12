//! Google Gemini con su API nativa: `streamGenerateContent?alt=sse`.

use super::error::{status_failure, Failure};
use super::openai::version_after;
use super::{http, sse, Decoder, ModelInfo, OnDelta, Step};
use serde_json::{json, Value};

const API: &str = "https://generativelanguage.googleapis.com/v1beta";
const NAME: &str = "Gemini";

/// Cuánto piensa el modelo. Los Gemini 3+ usan niveles; los Flash-Lite llegan
/// a «minimal». Los 2.5 se dejan con lo suyo (Flash-Lite ya no piensa).
fn thinking(model: &str) -> Option<Value> {
    let (major, _) = version_after(model, "gemini-")?;
    if major < 3 {
        return None;
    }
    let level = if model.contains("flash-lite") { "minimal" } else { "low" };
    Some(json!({ "thinkingLevel": level }))
}

pub fn body(model: &str, system: &str, user: &str, with_thinking: bool) -> Value {
    let mut config = json!({ "maxOutputTokens": 16000 });
    if with_thinking {
        if let Some(t) = thinking(model) {
            config["thinkingConfig"] = t;
        }
    }
    json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        "generationConfig": config,
    })
}

#[derive(Default)]
pub struct Stream {
    model: String,
    acc: String,
    blocked: bool,
}

impl Decoder for Stream {
    fn event(&mut self, ev: &sse::Event) -> Result<Step, Failure> {
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else { return Ok(Step::None) };
        if v["error"].is_object() {
            let code = v["error"]["code"].as_u64().unwrap_or(500) as u16;
            return Err(status_failure(NAME, &self.model, code, &ev.data));
        }
        if v["promptFeedback"]["blockReason"].is_string() {
            self.blocked = true;
        }
        let mut step = Step::None;
        for c in v["candidates"].as_array().into_iter().flatten() {
            for p in c["content"]["parts"].as_array().into_iter().flatten() {
                // Las partes de razonamiento (thought: true) no son la respuesta.
                if p["thought"] == true {
                    continue;
                }
                if let Some(t) = p["text"].as_str() {
                    self.acc.push_str(t);
                    step = Step::Delta;
                }
            }
            if let Some(r) = c["finishReason"].as_str() {
                if matches!(r, "SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII" | "RECITATION" | "IMAGE_SAFETY") {
                    self.blocked = true;
                }
            }
        }
        Ok(step)
    }

    fn text(&self) -> &str {
        &self.acc
    }

    fn end(&mut self) -> Result<(), Failure> {
        if self.blocked {
            return Err(Failure::refusal(NAME));
        }
        Ok(())
    }
}

pub async fn stream(key: &str, model: &str, system: &str, user: &str, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    let mut with_thinking = true;
    loop {
        let res = http::client()
            .post(format!("{API}/models/{model}:streamGenerateContent?alt=sse"))
            .header("x-goog-api-key", key)
            .json(&body(model, system, user, with_thinking))
            .send()
            .await
            .map_err(|_| Failure::offline(NAME))?;
        let status = res.status().as_u16();
        if status == 200 {
            let mut dec = Stream { model: model.into(), ..Default::default() };
            return super::drive(res, NAME, &mut dec, on_delta).await;
        }
        let text = res.text().await.unwrap_or_default();
        // Un modelo que no admite el nivel de razonamiento: otra vez, sin él.
        if status == 400 && with_thinking && thinking(model).is_some() && text.to_lowercase().contains("thinking") {
            with_thinking = false;
            continue;
        }
        return Err(status_failure(NAME, model, status, &text));
    }
}

async fn list_raw(key: &str, page_size: u32) -> Result<Value, Failure> {
    let res = http::client()
        .get(format!("{API}/models?pageSize={page_size}"))
        .header("x-goog-api-key", key)
        .send()
        .await
        .map_err(|_| Failure::offline(NAME))?;
    let status = res.status().as_u16();
    let text = res.text().await.unwrap_or_default();
    if status != 200 {
        return Err(status_failure(NAME, "", status, &text));
    }
    serde_json::from_str(&text).map_err(|_| Failure::new("Respuesta inesperada de Gemini."))
}

pub async fn test_key(key: &str) -> Result<(), Failure> {
    list_raw(key, 1).await.map(|_| ())
}

pub async fn models(key: &str) -> Result<Vec<ModelInfo>, Failure> {
    Ok(filter_models(&list_raw(key, 1000).await?))
}

const NOT_TEXT: [&str; 12] = ["tts", "image", "embedding", "live", "transcribe", "robotics", "native-audio", "computer-use", "deep-research", "antigravity", "translate", "aqa"];

pub fn filter_models(v: &Value) -> Vec<ModelInfo> {
    let mut out: Vec<(ModelInfo, (u32, u32))> = Vec::new();
    for m in v["models"].as_array().into_iter().flatten() {
        let Some(id) = m["name"].as_str().map(|n| n.trim_start_matches("models/")) else { continue };
        let generates = m["supportedGenerationMethods"].as_array().is_some_and(|a| a.iter().any(|x| x == "generateContent"));
        if !generates || !id.starts_with("gemini-") || NOT_TEXT.iter().any(|w| id.contains(w)) {
            continue;
        }
        let name = m["displayName"].as_str().unwrap_or(id).to_string();
        let ver = version_after(id, "gemini-").unwrap_or((0, 0));
        out.push((ModelInfo { id: id.into(), name, hint: None, recommended: false }, ver));
    }
    // Lo nuevo arriba; dentro de una versión, los estables antes que las «preview».
    out.sort_by_key(|(m, ver)| (std::cmp::Reverse(*ver), m.id.contains("preview") || m.id.contains("exp"), m.id.clone()));
    let mut out: Vec<ModelInfo> = out.into_iter().map(|(m, _)| m).collect();
    let stable = |m: &ModelInfo| !m.id.contains("preview") && !m.id.contains("exp");
    let pick = out
        .iter()
        .position(|m| stable(m) && m.id.ends_with("flash-lite"))
        .or_else(|| out.iter().position(|m| stable(m) && m.id.ends_with("flash")));
    if let Some(i) = pick {
        let mut m = out.remove(i);
        m.recommended = true;
        m.hint = Some("Recomendado: rápido y barato".into());
        out.insert(0, m);
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
        let mut s = Stream::default();
        s.event(&ev(r#"{"candidates":[{"content":{"parts":[{"text":"pensando","thought":true}],"role":"model"}}]}"#)).unwrap();
        assert_eq!(s.event(&ev(r#"{"candidates":[{"content":{"parts":[{"text":"Hola"}],"role":"model"}}]}"#)).unwrap(), Step::Delta);
        s.event(&ev(r#"{"candidates":[{"content":{"parts":[{"text":", mundo"}]},"finishReason":"STOP"}],"usageMetadata":{}}"#)).unwrap();
        assert_eq!(s.text(), "Hola, mundo");
        assert!(s.end().is_ok());
    }

    #[test]
    fn bloqueos_y_errores() {
        let mut s = Stream::default();
        s.event(&ev(r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#)).unwrap();
        assert_eq!(s.end().unwrap_err(), Failure::refusal("Gemini"));
        let mut s = Stream::default();
        s.event(&ev(r#"{"candidates":[{"finishReason":"PROHIBITED_CONTENT"}]}"#)).unwrap();
        assert!(s.end().is_err());
        let e = Stream::default().event(&ev(r#"{"error":{"code":503,"message":"The model is overloaded.","status":"UNAVAILABLE"}}"#)).unwrap_err();
        assert_eq!(e, Failure::overloaded("Gemini"));
    }

    #[test]
    fn razonamiento_segun_el_modelo() {
        assert_eq!(thinking("gemini-3.5-flash-lite").unwrap()["thinkingLevel"], "minimal");
        assert_eq!(thinking("gemini-3.8-flash").unwrap()["thinkingLevel"], "low");
        assert!(thinking("gemini-2.5-flash-lite").is_none());
        let b = body("gemini-3.8-flash", "S", "U", true);
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "S");
        assert_eq!(b["contents"][0]["parts"][0]["text"], "U");
        assert!(body("gemini-3.8-flash", "S", "U", false)["generationConfig"].get("thinkingConfig").is_none());
    }

    #[test]
    fn filtra_los_modelos() {
        let gen = json!(["generateContent", "countTokens"]);
        let v = json!({ "models": [
            { "name": "models/gemini-3.8-flash", "displayName": "Gemini 3.8 Flash", "supportedGenerationMethods": gen },
            { "name": "models/gemini-3.5-flash-lite", "displayName": "Gemini 3.5 Flash-Lite", "supportedGenerationMethods": gen },
            { "name": "models/gemini-3-flash-preview", "supportedGenerationMethods": gen },
            { "name": "models/gemini-3.8-flash-tts", "supportedGenerationMethods": gen },
            { "name": "models/gemini-embedding-2", "supportedGenerationMethods": ["embedContent"] },
            { "name": "models/imagen-4", "supportedGenerationMethods": ["predict"] }
        ]});
        let m = filter_models(&v);
        let ids: Vec<&str> = m.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-3-flash-preview"]);
        assert!(m[0].recommended);
        assert_eq!(m[1].name, "Gemini 3.8 Flash");
    }
}
