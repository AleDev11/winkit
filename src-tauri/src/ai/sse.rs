//! Lector de Server-Sent Events común a todos los proveedores. Trabaja con
//! bytes: un carácter UTF-8 puede llegar partido entre dos trozos de la
//! respuesta y no debe salir como «�».

#[derive(Debug, PartialEq)]
pub struct Event {
    /// Campo `event:` (Anthropic lo usa; OpenAI y Gemini no).
    pub event: Option<String>,
    /// Las líneas `data:` unidas con saltos de línea.
    pub data: String,
}

#[derive(Default)]
pub struct Parser {
    buf: Vec<u8>,
}

impl Parser {
    /// Añade un trozo y devuelve los eventos que ya están completos.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Event> {
        // "\r\n" → "\n": así basta con buscar una línea en blanco.
        self.buf.extend(chunk.iter().copied().filter(|&b| b != b'\r'));
        let mut out = Vec::new();
        while let Some(end) = self.buf.windows(2).position(|w| w == b"\n\n") {
            let raw: Vec<u8> = self.buf.drain(..end + 2).collect();
            if let Some(ev) = parse(&raw) {
                out.push(ev);
            }
        }
        out
    }

    /// Lo que quede al cerrarse la conexión (un último evento sin línea en blanco).
    pub fn finish(&mut self) -> Option<Event> {
        let raw = std::mem::take(&mut self.buf);
        parse(&raw)
    }
}

fn parse(raw: &[u8]) -> Option<Event> {
    let text = String::from_utf8_lossy(raw);
    let mut event = None;
    let mut data: Vec<&str> = Vec::new();
    for line in text.lines() {
        // Las líneas que empiezan por «:» son comentarios (OpenRouter manda
        // «: OPENROUTER PROCESSING» mientras espera).
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => event = Some(value.to_string()),
            "data" => data.push(value),
            _ => {}
        }
    }
    (!data.is_empty()).then(|| Event { event, data: data.join("\n") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junta_trozos_y_respeta_el_utf8_partido() {
        let mut p = Parser::default();
        let full = "event: x\r\ndata: {\"t\":\"¿qué?\"}\r\n\r\ndata: dos\n\n".as_bytes();
        // Partimos en mitad de la «¿» (2 bytes) y de la «é».
        let cut = full.iter().position(|&b| b == 0xC2).unwrap() + 1;
        assert!(p.push(&full[..cut]).is_empty());
        let evs = p.push(&full[cut..]);
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0], Event { event: Some("x".into()), data: "{\"t\":\"¿qué?\"}".into() });
        assert_eq!(evs[1].data, "dos");
        assert!(p.finish().is_none());
    }

    #[test]
    fn ignora_comentarios_y_une_varias_lineas_data() {
        let mut p = Parser::default();
        let evs = p.push(b": OPENROUTER PROCESSING\n\ndata: a\ndata: b\n\n");
        assert_eq!(evs, vec![Event { event: None, data: "a\nb".into() }]);
        assert_eq!(p.push(b"data: [DONE]").len(), 0);
        assert_eq!(p.finish().unwrap().data, "[DONE]");
    }
}
