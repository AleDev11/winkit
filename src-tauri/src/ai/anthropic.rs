//! Claude por HTTP (no hay SDK oficial para Rust): Messages API con streaming SSE.

use super::error::{status_failure, Failure};
use super::{http, sse, Decoder, ModelInfo, OnDelta, Step};
use crate::settings::MODELS;
use serde_json::{json, Value};

const API: &str = "https://api.anthropic.com/v1";
const VERSION: &str = "2026-10-21";
/// Reintento automático en servidor si los clasificadores rechazan el texto (Sonnet y Opus).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const NAME: &str = "Anthropic";

/// Cuerpo de la petición. Transformaciones y preguntas rápidas, no problemas
/// que razonar: la latencia manda.
pub fn body(model: &str, system: &str, content: &str) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "stream": true,
        "output_config": { "effort": "low" },
        "system": system,
        "messages": [{ "role": "user", "content": content }],
    });
    if is_haiku(model) {
        body["thinking"] = json!({ "type": "disabled" });
    } else {
        // Haiku no admite fallbacks en servidor; Sonnet y Opus sí.
        body["fallbacks"] = json!("default");
    }
    body
}

fn is_haiku(model: &str) -> bool {
    model.starts_with("claude-haiku")
}

/// Lee los eventos de la Messages API.
#[derive(Default)]
pub struct Stream {
    acc: String,
    stop_reason: Option<String>,
}

impl Decoder for Stream {
    fn event(&mut self, ev: &sse::Event) -> Result<Step, Failure> {
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else { return Ok(Step::None) };
        match v["type"].as_str() {
            // Con un fallback a mitad, el modelo de respaldo continúa en otro bloque de texto:
            // basta con acumular todos los text_delta.
            Some("content_block_delta") if v["delta"]["type"] == "text_delta" => {
                self.acc.push_str(v["delta"]["text"].as_str().unwrap_or(""));
                return Ok(Step::Delta);
            }
            Some("message_delta") => {
                if let Some(r) = v["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(r.to_string());
                }
            }
            Some("message_stop") => return Ok(Step::Done),
            Some("error") => {
                let kind = v["error"]["type"].as_str().unwrap_or("");
                return Err(if kind == "overloaded_error" {
                    Failure::overloaded(NAME)
                } else {
                    Failure::new(v["error"]["message"].as_str().unwrap_or("Error de Anthropic.").to_string())
                });
            }
            _ => {}
        }
        Ok(Step::None)
    }

    fn text(&self) -> &str {
        &self.acc
    }

    fn end(&mut self) -> Result<(), Failure> {
        if self.stop_reason.as_deref() == Some("refusal") {
            return Err(Failure::new("Claude no ha querido procesar este texto."));
        }
        Ok(())
    }
}

pub async fn stream(key: &str, model: &str, system: &str, content: &str, on_delta: OnDelta<'_>) -> Result<String, Failure> {
    let mut req = http::client()
        .post(format!("{API}/messages"))
        .header("x-api-key", key)
        .header("anthropic-version", VERSION)
        .json(&body(model, system, content));
    if !is_haiku(model) {
        req = req.header("anthropic-beta", FALLBACK_BETA);
    }
    let res = req.send().await.map_err(|_| Failure::offline(NAME))?;
    let status = res.status().as_u16();
    if status != 200 {
        let text = res.text().await.unwrap_or_default();
        return Err(status_failure(NAME, model, status, &text));
    }
    super::drive(res, NAME, &mut Stream::default(), on_delta).await
}

async fn get_models(key: &str, limit: u32) -> Result<Value, Failure> {
    let res = http::client()
        .get(format!("{API}/models?limit={limit}"))
        .header("x-api-key", key)
        .header("anthropic-version", VERSION)
        .send()
        .await
        .map_err(|_| Failure::offline(NAME))?;
    let status = res.status().as_u16();
    let text = res.text().await.unwrap_or_default();
    if status != 200 {
        return Err(status_failure(NAME, "", status, &text));
    }
    serde_json::from_str(&text).map_err(|_| Failure::new("Respuesta inesperada de Anthropic."))
}

pub async fn test_key(key: &str) -> Result<(), Failure> {
    get_models(key, 1).await.map(|_| ())
}

/// Los tres modelos de Winkit que tu key puede usar. Solo estos: la petición
/// (esfuerzo bajo, sin razonamiento en Haiku, fallbacks en Sonnet y Opus) está
/// hecha a su medida y otros modelos la rechazarían.
pub async fn models(key: &str) -> Result<Vec<ModelInfo>, Failure> {
    let v = get_models(key, 100).await?;
    let live: Vec<&str> = v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str()).collect();
    Ok(filter_models(&live))
}

pub fn filter_models(live: &[&str]) -> Vec<ModelInfo> {
    let hints = ["Rápido y barato. Ideal para corregir.", "Más fino con traducciones y textos largos.", "El más capaz. Más lento y caro."];
    MODELS
        .iter()
        .zip(hints)
        .filter(|((id, _), _)| live.is_empty() || live.contains(id))
        .map(|((id, label), hint)| ModelInfo { id: id.to_string(), name: label.to_string(), hint: Some(hint.into()), recommended: *id == MODELS[0].0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_peticion_no_cambia() {
        let h = body("claude-haiku-5-5", "S", "U");
        assert_eq!(h["output_config"]["effort"], "low");
        assert_eq!(h["thinking"]["type"], "disabled");
        assert!(h.get("fallbacks").is_none());
        assert_eq!(h["max_tokens"], 16000);
        let s = body("claude-sonnet-5-5", "S", "U");
        assert_eq!(s["fallbacks"], "default");
        assert!(s.get("thinking").is_none());
        assert_eq!(s["messages"][0]["content"], "U");
        assert_eq!(s["system"], "S");
    }

    fn ev(data: &str) -> sse::Event {
        sse::Event { event: None, data: data.into() }
    }

    #[test]
    fn lee_el_stream_y_los_errores() {
        let mut s = Stream::default();
        assert_eq!(s.event(&ev(r#"{"type":"message_start"}"#)).unwrap(), Step::None);
        assert_eq!(s.event(&ev(r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hola"}}"#)).unwrap(), Step::Delta);
        // Los bloques de razonamiento no se muestran.
        assert_eq!(s.event(&ev(r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"…"}}"#)).unwrap(), Step::None);
        s.event(&ev(r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":", mundo"}}"#)).unwrap();
        assert_eq!(s.text(), "Hola, mundo");
        s.event(&ev(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#)).unwrap();
        assert!(s.end().is_ok());

        let mut s = Stream::default();
        let e = s.event(&ev(r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#)).unwrap_err();
        assert_eq!(e, Failure::overloaded("Anthropic"));

        let mut s = Stream::default();
        s.event(&ev(r#"{"type":"message_delta","delta":{"stop_reason":"refusal"}}"#)).unwrap();
        assert!(s.end().unwrap_err().message.contains("no ha querido"));
    }

    #[test]
    fn solo_los_tres_modelos() {
        let m = filter_models(&["claude-opus-5-5", "claude-haiku-5-5", "claude-haiku-4-5"]);
        assert_eq!(m.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["claude-haiku-5-5", "claude-opus-5-5"]);
        assert!(m[0].recommended);
        assert_eq!(filter_models(&[]).len(), 3);
    }
}
