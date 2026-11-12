//! Voz local: Supertonic 3 (31 idiomas, español incluido) con sherpa-onnx,
//! sin conexión. El motor se llama «kokoro» en los ajustes porque era el
//! previsto, pero en sherpa-onnx 2.x Kokoro solo habla inglés y chino sin un
//! fonemizador externo (GPL): Supertonic lee el texto tal cual y suena mejor
//! en español. Ver el informe en voice/mod.rs.

use super::install::{self, ENGINE, MODEL};
use super::sherpa::{Library, SupertonicFiles, Tts};
use super::VoiceInfo;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Pasos de difusión: 5, lo de Supertone y sherpa-onnx por defecto. Con 8
/// suena apenas distinto y la primera frase tarda ~60 % más (0,7 s frente a
/// 0,43 s en un PC de 8 hilos).
pub const STEPS: i32 = 5;
/// Supertonic habla un pelín despacio a 1,0: sherpa-onnx usa 1,05 de base.
const BASE_SPEED: f32 = 1.05;
/// Sin usarse este rato, se libera la memoria del modelo.
const UNLOAD_AFTER: Duration = Duration::from_secs(5 * 60);

/// Idiomas que entiende Supertonic 3.
const LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr", "hu", "id", "it", "lt", "lv", "nl",
    "pl", "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi",
];

/// Las 10 voces de voice.bin, en su orden (sid): F1–F5 y M1–M5.
pub fn voices() -> Vec<VoiceInfo> {
    (0..10)
        .map(|i| {
            let (g, gender, n) = if i < 5 { ("F", "Mujer", i + 1) } else { ("M", "Hombre", i - 4) };
            VoiceInfo { id: format!("{g}{n}"), name: format!("{gender} {n}"), language: "mul".into(), gender: Some(gender.into()) }
        })
        .collect()
}

pub fn sid_of(voice: &str) -> i32 {
    voices().iter().position(|v| v.id.eq_ignore_ascii_case(voice)).unwrap_or(0) as i32
}

/// Código de idioma para Supertonic. Sin detectar: "na" (lo deduce el modelo).
/// El catalán no lo tiene: mejor con la fonética del español que con la del inglés.
pub fn lang_param(lang: Option<&str>) -> &'static str {
    match lang {
        Some("ca") => "es",
        Some("nb") => "da",
        Some(l) => LANGS.iter().find(|x| **x == l).copied().unwrap_or("na"),
        None => "na",
    }
}

struct Loaded {
    lib: Library,
    tts: Option<Tts>,
    last_used: Instant,
}

static ENGINE_STATE: Mutex<Option<Loaded>> = Mutex::new(None);

fn threads() -> i32 {
    let n = std::thread::available_parallelism().map_or(2, |n| n.get());
    (n / 2).clamp(1, 4) as i32
}

/// Carga (si hace falta) y ejecuta `f` con el modelo. `on_loading` avisa si va a tardar.
fn with_tts<T>(root: &Path, on_loading: &dyn Fn(), f: impl FnOnce(&Library, &Tts) -> T) -> Result<T, String> {
    let mut guard = ENGINE_STATE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.as_ref().is_none_or(|l| l.tts.is_none()) {
        on_loading();
        let t = Instant::now();
        if guard.is_none() {
            let lib = Library::load(&root.join(ENGINE.dir)).map_err(|e| {
                crate::log_warn!("voz", "{e}");
                "No se ha podido cargar el motor de voz local. Prueba a quitarlo y descargarlo otra vez.".to_string()
            })?;
            if lib.version != install::SHERPA_VERSION {
                crate::log_warn!("voz", "sherpa-onnx {} (se esperaba {})", lib.version, install::SHERPA_VERSION);
                return Err("La versión del motor de voz local no es la esperada. Quítalo y descárgalo otra vez.".into());
            }
            *guard = Some(Loaded { lib, tts: None, last_used: Instant::now() });
        }
        let loaded = guard.as_mut().unwrap();
        let tts = Tts::supertonic(&loaded.lib, SupertonicFiles { dir: &root.join(MODEL.dir), threads: threads() }).map_err(|e| {
            crate::log_warn!("voz", "{e}");
            "No se ha podido cargar la voz local. Prueba a quitarla y descargarla otra vez.".to_string()
        })?;
        crate::log_info!(
            "voz",
            "voz local cargada en {} ms ({} Hz, {} voces, {} hilos)",
            t.elapsed().as_millis(),
            tts.sample_rate,
            tts.speakers,
            threads()
        );
        loaded.tts = Some(tts);
        schedule_unload();
    }
    let loaded = guard.as_mut().unwrap();
    loaded.last_used = Instant::now();
    let result = f(&loaded.lib, loaded.tts.as_ref().unwrap());
    loaded.last_used = Instant::now();
    Ok(result)
}

/// Libera el modelo cuando lleva un rato sin usarse (las DLL se quedan cargadas).
fn schedule_unload() {
    let _ = std::thread::Builder::new().name("voz-descarga-memoria".into()).spawn(|| loop {
        std::thread::sleep(Duration::from_secs(30));
        let mut guard = ENGINE_STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(loaded) = guard.as_mut() else { return };
        if loaded.tts.is_none() {
            return;
        }
        if loaded.last_used.elapsed() >= UNLOAD_AFTER {
            if let Some(tts) = loaded.tts.take() {
                tts.destroy(&loaded.lib);
                crate::log_info!("voz", "voz local liberada de la memoria");
            }
            return;
        }
    });
}

/// Descarga todo (modelo y DLL) de la memoria: antes de borrar los archivos.
pub fn unload_all() {
    let mut guard = ENGINE_STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut loaded) = guard.take() {
        if let Some(tts) = loaded.tts.take() {
            tts.destroy(&loaded.lib);
        }
        drop(loaded);
    }
}

/// Sintetiza un trozo de texto. Devuelve (frecuencia, muestras).
pub fn synth(
    root: &Path,
    text: &str,
    voice: &str,
    lang: Option<&str>,
    speed: f32,
    on_loading: &dyn Fn(),
    cancel: &dyn Fn() -> bool,
) -> Result<(u32, Vec<f32>), String> {
    let extra = format!(r#"{{"lang":"{}"}}"#, lang_param(lang));
    let sid = sid_of(voice);
    with_tts(root, on_loading, |lib, tts| {
        tts.generate(lib, text, sid, BASE_SPEED * speed.clamp(0.5, 2.0), &extra, STEPS, cancel).map(|s| (tts.sample_rate, s))
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::install::{Error, Progress};
    use std::sync::atomic::AtomicBool;

    #[test]
    fn voces_y_parametros() {
        let v = voices();
        assert_eq!(v.len(), 10);
        assert_eq!(v[0].id, "F1");
        assert_eq!(v[9].id, "M5");
        assert_eq!(sid_of("m1"), 5);
        assert_eq!(sid_of("??"), 0);
        assert_eq!(lang_param(Some("es")), "es");
        assert_eq!(lang_param(Some("ca")), "es");
        assert_eq!(lang_param(Some("zh")), "na");
        assert_eq!(lang_param(None), "na");
    }

    /// Frecuencia fundamental aproximada (autocorrelación) de las partes con voz.
    fn pitch(samples: &[f32], rate: u32) -> f32 {
        let win = (rate as f32 * 0.04) as usize;
        let (lo, hi) = ((rate / 400) as usize, (rate / 70) as usize);
        let mut found = Vec::new();
        for frame in samples.chunks(win * 2).filter(|f| f.len() == win * 2) {
            let energy: f32 = frame.iter().map(|x| x * x).sum::<f32>() / frame.len() as f32;
            if energy < 1e-3 {
                continue;
            }
            let (mut best, mut best_lag) = (0.0f32, 0usize);
            for lag in lo..hi {
                let c: f32 = (0..win).map(|i| frame[i] * frame[i + lag]).sum();
                if c > best {
                    best = c;
                    best_lag = lag;
                }
            }
            if best_lag > 0 {
                found.push(rate as f32 / best_lag as f32);
            }
        }
        found.sort_by(|a, b| a.partial_cmp(b).unwrap());
        found.get(found.len() / 2).copied().unwrap_or(0.0)
    }

    /// Instala de verdad en una carpeta temporal, sintetiza en español y lo borra.
    #[test]
    #[ignore]
    fn instalar_y_sintetizar_real() {
        let root = std::env::temp_dir().join(format!("winkit-voz-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cancel = AtomicBool::new(false);
        let report = |done: Option<u64>, total: u64, what: &str| {
            if let Some(d) = done {
                if d % (16 << 20) < (256 << 10) {
                    println!("  {what}: {} / {} MB", d >> 20, total >> 20);
                }
            } else {
                println!("  {what}: descomprimiendo…");
            }
        };
        let t = Instant::now();
        let r = tauri::async_runtime::block_on(install::install(&root, &install::ASSETS, &Progress { cancel: &cancel, report: &report }));
        assert_eq!(r, Ok::<(), Error>(()));
        let size: u64 = walk(&root);
        println!("descarga de {} MB e instalación en {:.1} s; ocupa {} MB", install::download_mb(), t.elapsed().as_secs_f32(), size / 1_000_000);
        assert!(install::all_installed(&root));

        let text = "Hola, soy la voz local de Winkit. Puedo leerte las respuestas sin conexión a internet.";
        let t = Instant::now();
        let (rate, first) = synth(&root, "Hola, soy la voz local de Winkit.", "F1", Some("es"), 1.0, &|| {}, &|| false).unwrap();
        println!("carga + primera frase: {:?} ({:.2} s de audio, {rate} Hz)", t.elapsed(), first.len() as f32 / rate as f32);
        // Ya cargado: tiempo hasta el primer audio y factor de tiempo real.
        let t = Instant::now();
        let (_, short) = synth(&root, "Hola, soy la voz local de Winkit.", "F1", Some("es"), 1.0, &|| {}, &|| false).unwrap();
        let ttfa = t.elapsed();
        let t = Instant::now();
        let (_, all) = synth(&root, text, "F1", Some("es"), 1.0, &|| {}, &|| false).unwrap();
        let secs = all.len() as f32 / rate as f32;
        let rtf = t.elapsed().as_secs_f32() / secs;
        println!("primer audio (frase corta, modelo cargado): {ttfa:?} para {:.2} s", short.len() as f32 / rate as f32);
        println!("texto completo: {secs:.2} s de audio en {:?} → RTF {rtf:.3}", t.elapsed());
        assert!(secs > 3.0 && secs < 15.0, "{secs}");
        assert!(rtf < 1.0, "más lento que tiempo real: {rtf}");
        // Comprobación del orden de voces: F1 aguda, M1 grave.
        let (_, m) = synth(&root, "Hola, soy la voz local de Winkit.", "M1", Some("es"), 1.0, &|| {}, &|| false).unwrap();
        let (pf, pm) = (pitch(&short, rate), pitch(&m, rate));
        println!("tono F1 ≈ {pf:.0} Hz, M1 ≈ {pm:.0} Hz");
        assert!(pf > pm, "F1 debería sonar más aguda que M1");
        // Con 5 pasos (lo de sherpa-onnx por defecto) en vez de 8.
        let t = Instant::now();
        let five = with_tts(&root, &|| {}, |lib, tts| tts.generate(lib, "Hola, soy la voz local de Winkit.", 0, 1.05, r#"{"lang":"es"}"#, 5, &|| false))
            .unwrap()
            .unwrap();
        println!("5 pasos: {:?} para {:.2} s", t.elapsed(), five.len() as f32 / rate as f32);
        // Cancelar: sherpa-onnx solo lo mira entre trozos suyos; no debe fallar.
        assert!(synth(&root, text, "F1", Some("es"), 1.0, &|| {}, &|| true).is_ok());
        // Recarga con el disco ya en caché (la primera carga lee 145 MB).
        unload_all();
        let t = Instant::now();
        synth(&root, "Hola.", "F1", Some("es"), 1.0, &|| {}, &|| false).unwrap();
        println!("recarga del modelo + «Hola.»: {:?}", t.elapsed());

        unload_all();
        install::remove_all(&root).unwrap();
        assert!(!root.exists());
    }

    fn walk(dir: &Path) -> u64 {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| if e.path().is_dir() { walk(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) })
                    .sum()
            })
            .unwrap_or(0)
    }
}
