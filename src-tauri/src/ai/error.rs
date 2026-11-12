//! Errores de los proveedores traducidos a avisos que se entienden.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub message: String,
    /// El arreglo está en Ajustes (key, modelo, servidor…): el aviso ofrece abrirlos.
    pub open_settings: bool,
}

impl Failure {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), open_settings: false }
    }
    pub fn settings(message: impl Into<String>) -> Self {
        Self { message: message.into(), open_settings: true }
    }
    pub fn offline(provider: &str) -> Self {
        Self::new(format!("No hay conexión con {provider}."))
    }
    pub fn cut(provider: &str) -> Self {
        Self::new(format!("Se ha cortado la conexión con {provider}."))
    }
    pub fn overloaded(provider: &str) -> Self {
        Self::new(format!("{provider} está saturado ahora mismo. Prueba de nuevo."))
    }
    pub fn refusal(provider: &str) -> Self {
        Self::new(format!("{provider} no ha querido procesar este texto."))
    }
}

/// Mensaje y código del error en el cuerpo, en los formatos de OpenAI,
/// Anthropic y Google: `{"error": {"message", "type"|"code"|"status"}}`.
pub fn api_error(body: &str) -> (Option<String>, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let e = if v["error"].is_object() { &v["error"] } else { &v };
    let message = e["message"].as_str().map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    // Todos los códigos posibles juntos: se busca dentro con `contains`.
    let mut code = String::new();
    for k in ["type", "code", "status"] {
        if let Some(s) = e[k].as_str() {
            code.push_str(s);
            code.push(' ');
        }
    }
    if let Some(details) = e["details"].as_array() {
        for d in details {
            if let Some(r) = d["reason"].as_str() {
                code.push_str(r);
                code.push(' ');
            }
        }
    }
    (message, code.to_lowercase())
}

/// Error HTTP → aviso. `provider` es el nombre visible («OpenAI», «Mi servidor»…).
pub fn status_failure(provider: &str, model: &str, status: u16, body: &str) -> Failure {
    let (message, code) = api_error(body);
    let msg_lower = message.as_deref().unwrap_or("").to_lowercase();
    let bad_key = code.contains("api_key_invalid")
        || code.contains("authentication")
        || msg_lower.contains("api key not valid")
        || msg_lower.contains("invalid api key")
        || msg_lower.contains("incorrect api key");
    let no_credit = code.contains("insufficient_quota") || code.contains("insufficient_balance") || msg_lower.contains("insufficient balance") || msg_lower.contains("insufficient credits");
    match status {
        _ if bad_key || status == 401 => Failure::settings(format!("La API key de {provider} no es válida.")),
        _ if no_credit || status == 402 => {
            Failure::settings(format!("Tu cuenta de {provider} no tiene saldo. Recárgala en su web o elige otro modelo."))
        }
        403 => Failure::settings(format!("Tu API key de {provider} no tiene acceso a este modelo.")),
        404 => Failure::settings(format!("{provider} no tiene el modelo «{model}». Elige otro en Ajustes → IA.")),
        408 | 504 => Failure::new(format!("{provider} ha tardado demasiado en responder. Prueba de nuevo.")),
        429 => Failure::new(format!("Has llegado al límite de uso de {provider}. Prueba en unos segundos.")),
        500..=599 => Failure::overloaded(provider),
        _ => Failure::new(message.unwrap_or_else(|| format!("Error {status} de {provider}."))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduce_los_errores_habituales() {
        let f = status_failure("OpenAI", "m", 401, r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#);
        assert_eq!(f, Failure::settings("La API key de OpenAI no es válida."));
        // Google responde 400 con API_KEY_INVALID.
        let gemini = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"reason":"API_KEY_INVALID"}]}}"#;
        assert!(status_failure("Gemini", "m", 400, gemini).open_settings);
        // OpenAI sin saldo responde 429, no 402.
        let quota = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#;
        assert!(status_failure("OpenAI", "m", 429, quota).message.contains("no tiene saldo"));
        assert!(status_failure("DeepSeek", "m", 402, r#"{"error":{"message":"Insufficient Balance"}}"#).message.contains("saldo"));
        assert!(status_failure("X", "m", 429, "").message.contains("límite de uso de X"));
        assert_eq!(status_failure("Anthropic", "m", 529, ""), Failure::overloaded("Anthropic"));
        assert!(status_failure("X", "gpt-9", 404, "").message.contains("«gpt-9»"));
        // Un 400 cualquiera enseña el mensaje del proveedor.
        assert_eq!(status_failure("X", "m", 400, r#"{"error":{"message":"context too long"}}"#).message, "context too long");
        assert_eq!(status_failure("X", "m", 418, "no json").message, "Error 418 de X.");
    }
}
