//! Voces en la nube: ElevenLabs y OpenAI. Las dos devuelven PCM de 16 bits a
//! 24 kHz por streaming, así que el audio empieza a sonar mientras llega.

use super::pcm::Pcm16Stream;
use super::VoiceInfo;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;

pub const SAMPLE_RATE: u32 = 24_000;

/// El modelo de ElevenLabs: el de menos latencia (~100 ms) y el más barato
/// de los multilingües (incluye español).
pub const ELEVENLABS_MODEL: &str = "eleven_flash_v2_5";
const ELEVENLABS: &str = "https://api.elevenlabs.io/v1";
/// Idiomas de eleven_flash_v2_5 (ISO 639-1).
const ELEVENLABS_LANGS: &[&str] = &[
    "en", "ja", "zh", "de", "hi", "fr", "ko", "pt", "it", "es", "id", "nl", "tr", "fil", "pl", "sv", "bg", "ro", "ar", "cs", "el",
    "fi", "hr", "ms", "sk", "da", "ta", "uk", "ru", "hu", "no", "vi",
];

/// OpenAI anuncia el cierre de sus modelos de /v1/audio/speech el 6 de enero
/// de 2027 (recomienda pasar a la API Realtime); hasta entonces, este.
pub const OPENAI_MODEL: &str = "gpt-4o-mini-tts";
const OPENAI: &str = "https://api.openai.com/v1";

/// Voces de OpenAI (todas hablan varios idiomas; marin y cedar son las mejores).
pub const OPENAI_VOICES: &[(&str, &str)] = &[
    ("marin", "Mujer"),
    ("cedar", "Hombre"),
    ("coral", "Mujer"),
    ("sage", "Mujer"),
    ("nova", "Mujer"),
    ("shimmer", "Mujer"),
    ("alloy", "Mujer"),
    ("ballad", "Hombre"),
    ("ash", "Hombre"),
    ("echo", "Hombre"),
    ("fable", "Hombre"),
    ("onyx", "Hombre"),
    ("verse", "Hombre"),
];

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "El servicio de voz no responde.".into()
    } else {
        "No hay conexión con el servicio de voz.".into()
    }
}

/// Mensaje legible de una respuesta de error de ElevenLabs.
pub fn elevenlabs_error(status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let detail = &v["detail"];
    let code = detail["status"].as_str().or_else(|| detail["code"].as_str()).unwrap_or("");
    match (status, code) {
        (_, "quota_exceeded") => "Te has quedado sin créditos de ElevenLabs.".into(),
        (_, "missing_permissions") => "A esta API key de ElevenLabs le faltan permisos (texto a voz y leer voces).".into(),
        (401, _) | (_, "invalid_api_key") => "La API key de ElevenLabs no es válida.".into(),
        (402, _) => "Tu plan de ElevenLabs no permite esto.".into(),
        (429, _) => "ElevenLabs está saturado o has hecho demasiadas peticiones. Prueba en un momento.".into(),
        (404, _) => "Esa voz de ElevenLabs ya no existe. Elige otra en Ajustes.".into(),
        _ => match detail["message"].as_str().or_else(|| detail.as_str()) {
            Some(m) if !m.is_empty() => format!("ElevenLabs: {m}"),
            _ => format!("ElevenLabs ha fallado (HTTP {status})."),
        },
    }
}

/// Mensaje legible de una respuesta de error de OpenAI.
pub fn openai_error(status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let code = v["error"]["code"].as_str().unwrap_or("");
    match (status, code) {
        (_, "insufficient_quota") => "Tu cuenta de OpenAI no tiene saldo.".into(),
        (401, _) => "La API key de OpenAI no es válida.".into(),
        (429, _) => "OpenAI está saturado o has hecho demasiadas peticiones. Prueba en un momento.".into(),
        (404, _) => "OpenAI ya no ofrece este modelo de voz.".into(),
        _ => match v["error"]["message"].as_str() {
            Some(m) if !m.is_empty() => format!("OpenAI: {m}"),
            _ => format!("OpenAI ha fallado (HTTP {status})."),
        },
    }
}

/// Las voces de la cuenta de ElevenLabs (también sirve para validar la key).
pub async fn elevenlabs_voices(key: &str) -> Result<Vec<VoiceInfo>, String> {
    let resp = client()?.get(format!("{ELEVENLABS}/voices")).header("xi-api-key", key).send().await.map_err(net_error)?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(net_error)?;
    if status != 200 {
        return Err(elevenlabs_error(status, &body));
    }
    Ok(parse_elevenlabs_voices(&body))
}

pub fn parse_elevenlabs_voices(body: &str) -> Vec<VoiceInfo> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let mut out: Vec<VoiceInfo> = v["voices"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|voice| {
            let id = voice["voice_id"].as_str()?.to_string();
            let name = voice["name"].as_str().unwrap_or(&id).to_string();
            let labels = &voice["labels"];
            let gender = match labels["gender"].as_str().unwrap_or("").to_lowercase().as_str() {
                "female" => Some("Mujer".to_string()),
                "male" => Some("Hombre".to_string()),
                _ => None,
            };
            // Idioma: el verificado en español si lo tiene; si no, el de la etiqueta; si no, varios.
            let verified: Vec<String> = voice["verified_languages"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|l| l["locale"].as_str().or_else(|| l["language"].as_str()).map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let language = verified
                .iter()
                .find(|l| l.to_lowercase().starts_with("es"))
                .cloned()
                .or_else(|| labels["language"].as_str().map(str::to_string))
                .or_else(|| verified.first().cloned())
                .unwrap_or_else(|| "mul".into());
            Some(VoiceInfo { id, name, language, gender })
        })
        .collect();
    out.sort_by_key(|v| !v.language.to_lowercase().starts_with("es"));
    out
}

pub async fn openai_validate(key: &str) -> Result<(), String> {
    let resp = client()?.get(format!("{OPENAI}/models")).bearer_auth(key).send().await.map_err(net_error)?;
    let status = resp.status().as_u16();
    if status == 200 {
        return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    Err(openai_error(status, &body))
}

/// Pide la voz y va entregando muestras a `on_audio` según llegan. Para si
/// `stopped()` (se comprueba al menos cada 100 ms).
async fn stream_pcm(
    req: reqwest::RequestBuilder,
    on_error: fn(u16, &str) -> String,
    stopped: &(dyn Fn() -> bool + Sync),
    on_audio: &mut (dyn FnMut(Vec<f32>) + Send),
) -> Result<(), String> {
    let resp = req.send().await.map_err(net_error)?;
    let status = resp.status().as_u16();
    if status != 200 {
        let body = resp.text().await.unwrap_or_default();
        return Err(on_error(status, &body));
    }
    let mut pcm = Pcm16Stream::default();
    let mut stream = resp.bytes_stream();
    loop {
        if stopped() {
            return Ok(());
        }
        match tokio::time::timeout(Duration::from_millis(100), stream.next()).await {
            Err(_) => continue,
            Ok(None) => return Ok(()),
            Ok(Some(Err(e))) => return Err(net_error(e)),
            Ok(Some(Ok(bytes))) => {
                let samples = pcm.push(&bytes);
                if !samples.is_empty() {
                    on_audio(samples);
                }
            }
        }
    }
}

pub async fn elevenlabs_speak(
    key: &str,
    voice: &str,
    text: &str,
    lang: Option<&str>,
    speed: f32,
    stopped: &(dyn Fn() -> bool + Sync),
    on_audio: &mut (dyn FnMut(Vec<f32>) + Send),
) -> Result<(), String> {
    let mut body = json!({ "text": text, "model_id": ELEVENLABS_MODEL });
    // Solo los idiomas que admite el modelo (un código desconocido da error 400).
    if let Some(l) = lang.map(|l| if l == "nb" { "no" } else { l }).filter(|l| ELEVENLABS_LANGS.contains(l)) {
        body["language_code"] = json!(l);
    }
    // Solo si cambia: mandar voice_settings sustituye los ajustes guardados de la voz.
    if (speed - 1.0).abs() > 0.01 {
        body["voice_settings"] = json!({ "speed": speed.clamp(0.7, 1.2) });
    }
    let req = client()?
        .post(format!("{ELEVENLABS}/text-to-speech/{voice}/stream?output_format=pcm_24000"))
        .header("xi-api-key", key)
        .json(&body);
    stream_pcm(req, elevenlabs_error, stopped, on_audio).await
}

pub async fn openai_speak(
    key: &str,
    voice: &str,
    text: &str,
    lang: Option<&str>,
    speed: f32,
    stopped: &(dyn Fn() -> bool + Sync),
    on_audio: &mut (dyn FnMut(Vec<f32>) + Send),
) -> Result<(), String> {
    let mut body = json!({
        "model": OPENAI_MODEL,
        "input": text,
        "voice": voice,
        "response_format": "pcm",
        "speed": speed.clamp(0.25, 4.0),
    });
    if lang == Some("es") {
        body["instructions"] = json!("Lee el texto tal cual, en español de España, con un tono natural y claro.");
    }
    let req = client()?.post(format!("{OPENAI}/audio/speech")).bearer_auth(key).json(&body);
    stream_pcm(req, openai_error, stopped, on_audio).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voces_de_elevenlabs() {
        let body = r#"{"voices":[
            {"voice_id":"a","name":"Rachel","labels":{"gender":"female","accent":"american"},"verified_languages":[{"language":"en","locale":"en-US"}]},
            {"voice_id":"b","name":"Mateo","labels":{"gender":"male"},"verified_languages":[{"language":"en"},{"language":"es","locale":"es-ES"}]},
            {"voice_id":"c","name":"Sin datos"},
            {"name":"sin id"}
        ]}"#;
        let v = parse_elevenlabs_voices(body);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].name, "Mateo");
        assert_eq!(v[0].language, "es-ES");
        assert_eq!(v[0].gender.as_deref(), Some("Hombre"));
        assert_eq!(v[1].language, "en-US");
        assert_eq!(v[2].language, "mul");
        assert!(parse_elevenlabs_voices("no es json").is_empty());
    }

    #[test]
    fn errores_legibles() {
        assert_eq!(elevenlabs_error(401, r#"{"detail":{"status":"invalid_api_key","message":"x"}}"#), "La API key de ElevenLabs no es válida.");
        assert_eq!(elevenlabs_error(401, r#"{"detail":{"status":"quota_exceeded"}}"#), "Te has quedado sin créditos de ElevenLabs.");
        assert_eq!(elevenlabs_error(500, r#"{"detail":{"message":"Algo raro"}}"#), "ElevenLabs: Algo raro");
        assert_eq!(elevenlabs_error(503, ""), "ElevenLabs ha fallado (HTTP 503).");
        assert_eq!(openai_error(429, r#"{"error":{"code":"insufficient_quota"}}"#), "Tu cuenta de OpenAI no tiene saldo.");
        assert_eq!(openai_error(401, "{}"), "La API key de OpenAI no es válida.");
    }

    fn collect(f: impl std::future::Future<Output = Result<(), String>>) -> Result<(), String> {
        tauri::async_runtime::block_on(f)
    }

    /// Solo si hay key en ELEVENLABS_API_KEY.
    #[test]
    #[ignore]
    fn elevenlabs_real() {
        let Some(key) = crate::settings::provider_key_from_env("elevenlabs") else {
            println!("sin ELEVENLABS_API_KEY: se omite");
            return;
        };
        let voices = tauri::async_runtime::block_on(elevenlabs_voices(&key)).unwrap();
        println!("{} voces; primera: {:?}", voices.len(), voices.first().map(|v| (&v.name, &v.language)));
        let mut samples = 0usize;
        let mut first: Option<std::time::Duration> = None;
        let t = std::time::Instant::now();
        collect(elevenlabs_speak(&key, &voices[0].id, "Hola, esto es una prueba corta.", Some("es"), 1.0, &|| false, &mut |s| {
            first.get_or_insert(t.elapsed());
            samples += s.len();
        }))
        .unwrap();
        println!("primer audio en {:?}; {:.2} s", first.unwrap(), samples as f64 / SAMPLE_RATE as f64);
        assert!(samples > SAMPLE_RATE as usize / 2);
    }

    /// Solo si hay key en OPENAI_API_KEY.
    #[test]
    #[ignore]
    fn openai_real() {
        let Some(key) = crate::settings::provider_key_from_env("openai") else {
            println!("sin OPENAI_API_KEY: se omite");
            return;
        };
        let mut samples = 0usize;
        let mut first: Option<std::time::Duration> = None;
        let t = std::time::Instant::now();
        collect(openai_speak(&key, "marin", "Hola, esto es una prueba corta.", Some("es"), 1.0, &|| false, &mut |s| {
            first.get_or_insert(t.elapsed());
            samples += s.len();
        }))
        .unwrap();
        println!("primer audio en {:?}; {:.2} s", first.unwrap(), samples as f64 / SAMPLE_RATE as f64);
        assert!(samples > SAMPLE_RATE as usize / 2);
    }
}
