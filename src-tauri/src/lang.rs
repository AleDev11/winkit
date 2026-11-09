//! Detección de idioma en local (whatlang, sin red) para decidir la dirección
//! de las traducciones antes de llamar a Claude.

use crate::settings::Settings;
use serde::Serialize;
use whatlang::{Detector, Lang};

/// Idiomas que sabemos nombrar en español. El resto se trata como desconocido.
const NAMES: &[(Lang, &str)] = &[
    (Lang::Spa, "Español"),
    (Lang::Eng, "Inglés"),
    (Lang::Cat, "Catalán"),
    (Lang::Fra, "Francés"),
    (Lang::Ita, "Italiano"),
    (Lang::Deu, "Alemán"),
    (Lang::Por, "Portugués"),
    (Lang::Nld, "Neerlandés"),
    (Lang::Rus, "Ruso"),
    (Lang::Ukr, "Ucraniano"),
    (Lang::Pol, "Polaco"),
    (Lang::Ces, "Checo"),
    (Lang::Ron, "Rumano"),
    (Lang::Ell, "Griego"),
    (Lang::Tur, "Turco"),
    (Lang::Swe, "Sueco"),
    (Lang::Dan, "Danés"),
    (Lang::Nob, "Noruego"),
    (Lang::Fin, "Finés"),
    (Lang::Hun, "Húngaro"),
    (Lang::Ara, "Árabe"),
    (Lang::Heb, "Hebreo"),
    (Lang::Hin, "Hindi"),
    (Lang::Jpn, "Japonés"),
    (Lang::Kor, "Coreano"),
    (Lang::Cmn, "Chino"),
    (Lang::Vie, "Vietnamita"),
    (Lang::Tha, "Tailandés"),
    (Lang::Ind, "Indonesio"),
    (Lang::Lat, "Latín"),
];

fn fold(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            c => c,
        })
        .collect()
}

fn lang_of(name: &str) -> Option<Lang> {
    let n = fold(name);
    NAMES.iter().find(|(_, es)| fold(es) == n).map(|(l, _)| *l)
}

pub fn same(a: &str, b: &str) -> bool {
    fold(a) == fold(b)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageInfo {
    pub detected: Option<String>,
    pub translate_to: String,
    pub understand_to: String,
}

/// Detecta el idioma del texto. Solo considera los idiomas que el usuario usa
/// (su lista, su idioma y el de destino): así no confunde español con gallego
/// o catalán con occitano en textos cortos.
pub fn detect(text: &str, s: &Settings) -> Option<String> {
    let text = text.trim();
    if text.chars().filter(|c| c.is_alphabetic()).count() < 12 {
        return None;
    }
    let mut allow: Vec<Lang> = s
        .languages
        .iter()
        .chain([&s.my_language, &s.translate_target])
        .filter_map(|n| lang_of(n))
        .collect();
    allow.sort_by_key(|l| l.code());
    allow.dedup();
    let detector = if allow.len() >= 2 { Detector::with_allowlist(allow) } else { Detector::new() };
    let info = detector.detect(text)?;
    if !info.is_reliable() && info.confidence() < 0.5 {
        return None;
    }
    NAMES.iter().find(|(l, _)| *l == info.lang()).map(|(_, es)| es.to_string())
}

/// Detecta y calcula los destinos de «Traducir» y «Entender».
pub fn info(text: &str, s: &Settings) -> LanguageInfo {
    let detected = detect(text, s);
    let is = |lang: &str| detected.as_deref().is_some_and(|d| same(d, lang));
    LanguageInfo {
        translate_to: if is(&s.translate_target) { s.my_language.clone() } else { s.translate_target.clone() },
        understand_to: if is(&s.my_language) { s.translate_target.clone() } else { s.my_language.clone() },
        detected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_la_direccion() {
        let s = Settings::default(); // tu idioma: español; destino: inglés
        let en = info("Hey, can we move the meeting to Thursday afternoon? Something came up.", &s);
        assert_eq!(en.detected.as_deref(), Some("Inglés"));
        assert_eq!(en.translate_to, "Español");
        assert_eq!(en.understand_to, "Español");

        let es = info("Oye, ¿podemos mover la reunión al jueves por la tarde? Me ha surgido algo.", &s);
        assert_eq!(es.detected.as_deref(), Some("Español"));
        assert_eq!(es.translate_to, "Inglés");
        assert_eq!(es.understand_to, "Inglés");
    }

    #[test]
    fn texto_corto_no_se_detecta() {
        assert_eq!(detect("ok", &Settings::default()), None);
    }
}
