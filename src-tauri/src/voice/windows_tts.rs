//! Voces que trae Windows (WinRT `SpeechSynthesizer`, las mismas de Narrador
//! «clásicas»): sin descargas y sin conexión.

use super::pcm::{parse_wav, Audio};
use super::VoiceInfo;
use windows::core::HSTRING;
use windows::Media::SpeechSynthesis::{SpeechSynthesizer, VoiceGender, VoiceInformation};
use windows::Storage::Streams::DataReader;

/// WinRT necesita COM en el hilo (los hilos de tokio y el de la lectura no lo traen).
fn com() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn gender(g: VoiceGender) -> Option<String> {
    match g {
        VoiceGender::Female => Some("Mujer".into()),
        VoiceGender::Male => Some("Hombre".into()),
        _ => None,
    }
}

fn info(v: &VoiceInformation) -> windows::core::Result<VoiceInfo> {
    // "Microsoft Helena - Spanish (Spain)" → "Helena".
    let display = v.DisplayName()?.to_string();
    let name = display.split(" - ").next().unwrap_or(&display).trim_start_matches("Microsoft ").to_string();
    Ok(VoiceInfo { id: v.Id()?.to_string(), name, language: v.Language()?.to_string(), gender: v.Gender().ok().and_then(gender) })
}

/// Voces instaladas. La predeterminada de Windows primero.
pub fn voices() -> Result<Vec<VoiceInfo>, String> {
    com();
    let run = || -> windows::core::Result<Vec<VoiceInfo>> {
        let default = SpeechSynthesizer::DefaultVoice().and_then(|v| v.Id()).map(|s| s.to_string()).unwrap_or_default();
        let all = SpeechSynthesizer::AllVoices()?;
        let mut out = Vec::new();
        for i in 0..all.Size()? {
            out.push(info(&all.GetAt(i)?)?);
        }
        if let Some(pos) = out.iter().position(|v| v.id == default) {
            let d = out.remove(pos);
            out.insert(0, d);
        }
        Ok(out)
    };
    run().map_err(|e| {
        crate::log_warn!("voz", "no se han podido leer las voces de Windows: {e}");
        "No se han podido leer las voces de Windows.".to_string()
    })
}

/// Sintetizador con una voz y velocidad fijas para leer varios trozos seguidos.
pub struct Synth {
    synth: SpeechSynthesizer,
}

impl Synth {
    /// `voice`: id de `voices()` ("" = la predeterminada de Windows).
    pub fn new(voice: &str, speed: f32) -> Result<Self, String> {
        com();
        let make = || -> windows::core::Result<SpeechSynthesizer> {
            let synth = SpeechSynthesizer::new()?;
            if !voice.is_empty() {
                let all = SpeechSynthesizer::AllVoices()?;
                for i in 0..all.Size()? {
                    let v = all.GetAt(i)?;
                    if v.Id()?.to_string() == voice {
                        synth.SetVoice(&v)?;
                        break;
                    }
                }
            }
            // 0,5–6 en Windows; Ajustes usa 0,5–2.
            synth.Options()?.SetSpeakingRate(speed.clamp(0.5, 2.0) as f64)?;
            Ok(synth)
        };
        make().map(|synth| Self { synth }).map_err(|e| {
            crate::log_warn!("voz", "no se ha podido preparar la voz de Windows: {e}");
            "No se ha podido preparar la voz de Windows.".to_string()
        })
    }

    pub fn speak(&self, text: &str) -> Result<Audio, String> {
        let run = || -> windows::core::Result<Vec<u8>> {
            let stream = self.synth.SynthesizeTextToStreamAsync(&HSTRING::from(text))?.join()?;
            let size = stream.Size()? as u32;
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
            reader.LoadAsync(size)?.join()?;
            let mut bytes = vec![0u8; size as usize];
            reader.ReadBytes(&mut bytes)?;
            Ok(bytes)
        };
        let bytes = run().map_err(|e| {
            crate::log_warn!("voz", "la voz de Windows ha fallado: {e}");
            "La voz de Windows no ha podido leer el texto.".to_string()
        })?;
        parse_wav(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Voces reales de este PC y una frase en español a un búfer (sin sonar).
    #[test]
    #[ignore]
    fn voces_y_sintesis_reales() {
        let voices = voices().unwrap();
        println!("{} voces de Windows:", voices.len());
        for v in &voices {
            println!("  {} [{}] {:?} ({})", v.name, v.language, v.gender, v.id);
        }
        let es: Vec<_> = voices.iter().filter(|v| v.language.to_lowercase().starts_with("es")).collect();
        println!("españolas: {}", es.iter().map(|v| format!("{} ({})", v.name, v.language)).collect::<Vec<_>>().join(", "));
        let voice = es.first().map(|v| v.id.clone()).unwrap_or_default();
        let synth = Synth::new(&voice, 1.0).unwrap();
        let t = std::time::Instant::now();
        let audio = synth.speak("Hola, esto es una prueba de la voz de Windows.").unwrap();
        let ms = t.elapsed().as_millis();
        println!("{} Hz, {:.2} s de audio en {ms} ms", audio.sample_rate, audio.seconds());
        assert!(audio.seconds() > 1.0 && audio.seconds() < 8.0, "{}", audio.seconds());
        assert!(audio.samples.iter().any(|s| s.abs() > 0.05), "audio en silencio");
        // Más rápida = más corta.
        let fast = Synth::new(&voice, 2.0).unwrap().speak("Hola, esto es una prueba de la voz de Windows.").unwrap();
        assert!(fast.seconds() < audio.seconds() * 0.8, "{} vs {}", fast.seconds(), audio.seconds());
    }
}
