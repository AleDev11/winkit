//! Preparar el texto antes de leerlo: quitar el Markdown (las respuestas del
//! buscador vienen en Markdown), partirlo en frases para que la voz empiece
//! enseguida y saber en qué idioma está.

use crate::settings::Settings;

/// Quita la sintaxis de Markdown y deja texto leíble. Los bloques de código se
/// omiten (leídos en voz alta solo son ruido); el código en línea se queda.
/// Los títulos y elementos de lista terminan en punto para que la voz haga pausa.
pub fn strip_markdown(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut in_fence = false;
    let flush = |paragraph: &mut Vec<String>, out: &mut Vec<String>| {
        if !paragraph.is_empty() {
            out.push(paragraph.join(" "));
            paragraph.clear();
        }
    };
    for raw in md.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            flush(&mut paragraph, &mut out);
            continue;
        }
        if in_fence {
            continue;
        }
        if line.is_empty() {
            flush(&mut paragraph, &mut out);
            continue;
        }
        // Reglas horizontales y separadores de tablas.
        if is_rule(line) || is_table_separator(line) {
            flush(&mut paragraph, &mut out);
            continue;
        }
        let mut block = false;
        let mut text = line;
        // Citas (pueden ir anidadas: "> > ").
        while let Some(rest) = text.strip_prefix('>') {
            text = rest.trim_start();
        }
        if let Some(rest) = heading(text) {
            text = rest;
            block = true;
        } else if let Some(rest) = list_item(text) {
            text = rest;
            block = true;
        }
        let mut clean = inline(text);
        if text.starts_with('|') || text.ends_with('|') {
            // Fila de tabla: las celdas, separadas por comas.
            clean = clean.trim_matches('|').split('|').map(str::trim).filter(|c| !c.is_empty()).collect::<Vec<_>>().join(", ");
            block = true;
        }
        let clean = clean.trim().to_string();
        if clean.is_empty() {
            continue;
        }
        if block {
            flush(&mut paragraph, &mut out);
            out.push(with_stop(&clean));
        } else {
            paragraph.push(clean);
        }
    }
    flush(&mut paragraph, &mut out);
    out.join("\n")
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && (compact.chars().all(|c| c == '-') || compact.chars().all(|c| c == '*') || compact.chars().all(|c| c == '_'))
}

fn is_table_separator(line: &str) -> bool {
    line.contains('-') && line.contains('|') && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) {
        let rest = &line[hashes..];
        if rest.is_empty() || rest.starts_with(' ') {
            return Some(rest.trim().trim_end_matches('#').trim());
        }
    }
    None
}

fn list_item(line: &str) -> Option<&str> {
    for p in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(p) {
            return Some(rest);
        }
    }
    // "1. " o "1) "
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        let rest = &line[digits..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some(r);
        }
    }
    None
}

/// Añade un punto si la línea no termina ya en un signo de puntuación.
fn with_stop(s: &str) -> String {
    match s.chars().last() {
        Some(c) if ".!?…:;,".contains(c) => s.to_string(),
        _ => format!("{s}."),
    }
}

/// Marcas en línea: énfasis, código, enlaces, imágenes, etiquetas HTML y URLs.
fn inline(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\\' if next.is_some_and(|n| n.is_ascii_punctuation()) => {
                out.push(next.unwrap());
                i += 2;
            }
            '`' => {
                // Código en línea: se queda el contenido.
                let ticks = chars[i..].iter().take_while(|c| **c == '`').count();
                i += ticks;
            }
            '!' if next == Some('[') => {
                // Imagen: su texto alternativo, si lo tiene.
                if let Some((label, end)) = link_at(&chars, i + 1) {
                    out.push_str(&inline(&label));
                    i = end;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            '[' => {
                if let Some((label, end)) = link_at(&chars, i) {
                    out.push_str(&inline(&label));
                    i = end;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            '<' => {
                // <https://…> o etiquetas HTML sencillas (<br>, <b>…).
                if let Some(close) = chars[i..].iter().position(|c| *c == '>') {
                    let inner: String = chars[i + 1..i + close].iter().collect();
                    if inner.starts_with("http://") || inner.starts_with("https://") {
                        out.push_str(&url_host(&inner));
                        i += close + 1;
                        continue;
                    }
                    let name: String =
                        inner.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
                    if HTML_TAGS.contains(&name.as_str()) {
                        out.push(' ');
                        i += close + 1;
                        continue;
                    }
                }
                out.push(c);
                i += 1;
            }
            '*' | '_' | '~' => {
                let run = chars[i..].iter().take_while(|x| **x == c).count();
                let prev = if i == 0 { None } else { Some(chars[i - 1]) };
                let after = chars.get(i + run).copied();
                let word = |x: Option<char>| x.is_some_and(|x| x.is_alphanumeric());
                let space = |x: Option<char>| x.is_none_or(|x| x.is_whitespace());
                let emphasis = if c == '_' {
                    // snake_case se respeta: solo en el borde de una palabra.
                    !(word(prev) && word(after))
                } else if c == '~' {
                    run >= 2
                } else {
                    // "2 * 3" se queda; "*hola*" no.
                    !(space(prev) && space(after))
                };
                if emphasis {
                    i += run;
                } else {
                    for _ in 0..run {
                        out.push(c);
                    }
                    i += run;
                }
            }
            'h' if starts_with(&chars, i, "http://") || starts_with(&chars, i, "https://") => {
                let end = chars[i..].iter().position(|c| c.is_whitespace()).map_or(chars.len(), |p| i + p);
                let url: String = chars[i..end].iter().collect();
                let trailing: String = url.chars().rev().take_while(|c| ".,;:)!?".contains(*c)).collect();
                out.push_str(&url_host(url.trim_end_matches(|c| ".,;:)!?".contains(c))));
                out.extend(trailing.chars().rev());
                i = end;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    // Espacios repetidos (al quitar marcas) a uno solo.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Etiquetas HTML que a veces aparecen en Markdown y se quitan.
const HTML_TAGS: &[&str] =
    &["br", "b", "i", "u", "em", "strong", "span", "div", "p", "sub", "sup", "kbd", "code", "small", "mark", "details", "summary"];

fn starts_with(chars: &[char], i: usize, pat: &str) -> bool {
    pat.chars().enumerate().all(|(k, p)| chars.get(i + k) == Some(&p))
}

/// `[texto](url)` empezando en `i` (el corchete): devuelve el texto y dónde acaba.
fn link_at(chars: &[char], i: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    let mut j = i;
    while j < chars.len() {
        match chars[j] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    if j >= chars.len() {
        return None;
    }
    let label: String = chars[i + 1..j].iter().collect();
    if chars.get(j + 1) == Some(&'(') {
        let close = chars[j + 1..].iter().position(|c| *c == ')')?;
        return Some((label, j + 1 + close + 1));
    }
    if chars.get(j + 1) == Some(&'[') {
        // Enlace por referencia: [texto][ref].
        let close = chars[j + 1..].iter().position(|c| *c == ']')?;
        return Some((label, j + 1 + close + 1));
    }
    None
}

/// De una URL solo se lee el dominio: «github.com».
fn url_host(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

// ---------------------------------------------------------------- frases

/// Abreviaturas que no cierran frase aunque lleven punto.
const ABBREVIATIONS: &[&str] = &[
    "sr", "sra", "srta", "dr", "dra", "ud", "uds", "etc", "ej", "p", "pág", "núm", "aprox", "av", "vs", "mr", "mrs", "ms",
    "st", "e.g", "i.e", "approx",
];

/// Parte el texto en frases (por los signos de fin de frase y los saltos de línea).
pub fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let chars: Vec<char> = para.chars().collect();
        let mut start = 0;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if matches!(c, '.' | '!' | '?' | '…' | ';') {
                // Varios seguidos («?!», «...») y comillas o paréntesis de cierre.
                let mut end = i + 1;
                while end < chars.len() && matches!(chars[end], '.' | '!' | '?' | '…' | '"' | '»' | ')' | '\'' | '”') {
                    end += 1;
                }
                let at_break = end >= chars.len() || chars[end].is_whitespace();
                let abbreviation = c == '.' && end == i + 1 && {
                    let word: String =
                        chars[start..i].iter().rev().take_while(|c| !c.is_whitespace()).collect::<Vec<_>>().into_iter().rev().collect();
                    let w = word.trim_start_matches(['(', '"', '«', '¿', '¡']).to_lowercase();
                    ABBREVIATIONS.contains(&w.as_str()) || (w.chars().count() == 1 && w.chars().all(char::is_alphabetic))
                };
                if at_break && !abbreviation {
                    push_trimmed(&mut out, &chars[start..end]);
                    start = end;
                }
                i = end;
            } else {
                i += 1;
            }
        }
        push_trimmed(&mut out, &chars[start..]);
    }
    out
}

fn push_trimmed(out: &mut Vec<String>, chars: &[char]) {
    let s: String = chars.iter().collect();
    let s = s.trim();
    if !s.is_empty() {
        out.push(s.to_string());
    }
}

/// Trozos para sintetizar uno detrás de otro: el primero corto (para que la
/// voz empiece enseguida) y los demás de hasta `max` caracteres, sin partir
/// frases salvo que una sola ya sea más larga (entonces por comas o espacios).
pub fn chunks(text: &str, first_max: usize, max: usize) -> Vec<String> {
    let mut pieces: Vec<String> = Vec::new();
    for s in sentences(text) {
        let limit = if pieces.is_empty() { first_max } else { max };
        if s.chars().count() <= limit {
            pieces.push(s);
        } else {
            pieces.extend(split_long(&s, if pieces.is_empty() { first_max } else { max }, max));
        }
    }
    // Junta frases cortas hasta `max` (menos llamadas y una entonación más natural).
    let mut out: Vec<String> = Vec::new();
    for p in pieces {
        // El primer trozo no se agranda.
        let merge = out.len() >= 2 && out.last().is_some_and(|last| last.chars().count() + 1 + p.chars().count() <= max);
        match out.last_mut() {
            Some(last) if merge => {
                last.push(' ');
                last.push_str(&p);
            }
            _ => out.push(p),
        }
    }
    out
}

/// Parte una frase larga: primero por comas, después por espacios.
fn split_long(s: &str, first_max: usize, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let limit = |out: &Vec<String>| if out.is_empty() { first_max } else { max };
    for clause in s.split_inclusive([',', ';', ':']) {
        if current.chars().count() + clause.chars().count() <= limit(&out) {
            current.push_str(clause);
            continue;
        }
        if !current.trim().is_empty() {
            out.push(current.trim().to_string());
            current.clear();
        }
        if clause.chars().count() <= limit(&out) {
            current.push_str(clause);
            continue;
        }
        for word in clause.split_whitespace() {
            if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > limit(&out) {
                out.push(current.trim().to_string());
                current.clear();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        current.push(' ');
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

// ---------------------------------------------------------------- idioma

/// Nombre en español (el de `crate::lang`) → código ISO 639-1.
const CODES: &[(&str, &str)] = &[
    ("Español", "es"),
    ("Inglés", "en"),
    ("Catalán", "ca"),
    ("Francés", "fr"),
    ("Italiano", "it"),
    ("Alemán", "de"),
    ("Portugués", "pt"),
    ("Neerlandés", "nl"),
    ("Ruso", "ru"),
    ("Ucraniano", "uk"),
    ("Polaco", "pl"),
    ("Checo", "cs"),
    ("Rumano", "ro"),
    ("Griego", "el"),
    ("Turco", "tr"),
    ("Sueco", "sv"),
    ("Danés", "da"),
    ("Noruego", "nb"),
    ("Finés", "fi"),
    ("Húngaro", "hu"),
    ("Árabe", "ar"),
    ("Hebreo", "he"),
    ("Hindi", "hi"),
    ("Japonés", "ja"),
    ("Coreano", "ko"),
    ("Chino", "zh"),
    ("Vietnamita", "vi"),
    ("Tailandés", "th"),
    ("Indonesio", "id"),
];

pub fn code_of(name: &str) -> Option<&'static str> {
    CODES.iter().find(|(n, _)| crate::lang::same(n, name)).map(|(_, c)| *c)
}

/// Idioma del texto («es», «en»…). `None` si es corto o ambiguo.
pub fn detect(text: &str, s: &Settings) -> Option<&'static str> {
    crate::lang::detect(text, s).and_then(|name| code_of(&name))
}

/// Primera parte de una etiqueta BCP 47 en minúsculas: "es-ES" → "es".
pub fn primary(tag: &str) -> String {
    tag.split(['-', '_']).next().unwrap_or("").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quita_el_markdown() {
        let md = "# Resumen\n\nEsto es **muy** importante y _bastante_ claro.\n\n- Primero\n- Segundo: `cargo test`\n\n```rust\nfn main() {}\n```\n\nMira [la guía](https://ejemplo.com/guia) o <https://www.github.com/x>.";
        let t = strip_markdown(md);
        assert_eq!(
            t,
            "Resumen.\nEsto es muy importante y bastante claro.\nPrimero.\nSegundo: cargo test.\nMira la guía o github.com."
        );
    }

    #[test]
    fn respeta_lo_que_no_es_markdown() {
        assert_eq!(strip_markdown("usa mi_variable_larga y 2 * 3 = 6"), "usa mi_variable_larga y 2 * 3 = 6");
        assert_eq!(strip_markdown("~~tachado~~ y ~1 km"), "tachado y ~1 km");
        assert_eq!(strip_markdown("> cita\n> > anidada"), "cita anidada");
        assert_eq!(strip_markdown("1. uno\n2) dos!"), "uno.\ndos!");
        assert_eq!(strip_markdown("***\n---"), "");
    }

    #[test]
    fn tablas_e_imagenes() {
        let md = "| Moneda | Valor |\n|---|---:|\n| Euro | 1 |\n\n![Un gato](gato.png)";
        assert_eq!(strip_markdown(md), "Moneda, Valor.\nEuro, 1.\nUn gato");
    }

    #[test]
    fn urls_sueltas_solo_dominio() {
        assert_eq!(strip_markdown("Ve a https://docs.rs/tokio/latest, ahí está."), "Ve a docs.rs, ahí está.");
    }

    #[test]
    fn parte_en_frases() {
        let s = sentences("Hola, Sr. Pérez. ¿Qué tal? ¡Bien!... Vale; seguimos\nOtra línea sin punto");
        assert_eq!(s, vec!["Hola, Sr. Pérez.", "¿Qué tal?", "¡Bien!...", "Vale;", "seguimos", "Otra línea sin punto"]);
        // Decimales y siglas no cortan.
        assert_eq!(sentences("Cuesta 3.5 euros. Vale."), vec!["Cuesta 3.5 euros.", "Vale."]);
        assert_eq!(sentences("p. ej. esto. Y aquello."), vec!["p. ej. esto.", "Y aquello."]);
    }

    #[test]
    fn trozos_primero_corto_y_luego_juntos() {
        let text = "Hola. Esto es una prueba. Otra frase más. Y una última frase para terminar.";
        let c = chunks(text, 20, 60);
        assert_eq!(c[0], "Hola.");
        assert_eq!(c[1], "Esto es una prueba. Otra frase más.");
        assert_eq!(c[2], "Y una última frase para terminar.");
        for piece in &c {
            assert!(piece.chars().count() <= 60, "{piece}");
        }
    }

    #[test]
    fn trozos_parten_frases_largas() {
        let long = "palabra ".repeat(60);
        let c = chunks(&long, 40, 100);
        assert!(c.len() >= 5);
        assert!(c[0].chars().count() <= 40);
        assert!(c.iter().all(|p| p.chars().count() <= 100));
        assert_eq!(c.join(" ").split_whitespace().count(), 60);

        let commas = "Primero esto, después aquello, luego lo otro, y al final lo de más allá.";
        assert_eq!(chunks(commas, 30, 200), vec!["Primero esto, después aquello,", "luego lo otro, y al final lo de más allá."]);
    }

    #[test]
    fn idioma_a_codigo() {
        assert_eq!(code_of("Español"), Some("es"));
        assert_eq!(code_of("ingles"), Some("en"));
        assert_eq!(primary("es-ES"), "es");
        let s = Settings::default();
        assert_eq!(detect("Oye, ¿podemos mover la reunión al jueves por la tarde? Me ha surgido algo.", &s), Some("es"));
        assert_eq!(detect("Hey, can we move the meeting to Thursday afternoon? Something came up.", &s), Some("en"));
    }
}
