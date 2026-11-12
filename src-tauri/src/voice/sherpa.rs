//! Enlace mínimo con la API en C de sherpa-onnx (solo texto a voz), cargada
//! en tiempo de ejecución desde la carpeta que descarga Winkit. Las
//! estructuras copian `sherpa-onnx/c-api/c-api.h` de la versión fijada en
//! `install::SHERPA_VERSION`: si se cambia de versión hay que revisarlas.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use windows::core::{HSTRING, PCSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
};

type P = *const c_char;

#[repr(C)]
struct VitsConfig {
    model: P,
    lexicon: P,
    tokens: P,
    data_dir: P,
    noise_scale: f32,
    noise_scale_w: f32,
    length_scale: f32,
    dict_dir: P,
}

#[repr(C)]
struct MatchaConfig {
    acoustic_model: P,
    vocoder: P,
    lexicon: P,
    tokens: P,
    data_dir: P,
    noise_scale: f32,
    length_scale: f32,
    dict_dir: P,
}

#[repr(C)]
struct KokoroConfig {
    model: P,
    voices: P,
    tokens: P,
    data_dir: P,
    length_scale: f32,
    dict_dir: P,
    lexicon: P,
    lang: P,
}

#[repr(C)]
struct KittenConfig {
    model: P,
    voices: P,
    tokens: P,
    data_dir: P,
    length_scale: f32,
    lexicon: P,
}

#[repr(C)]
struct ZipvoiceConfig {
    tokens: P,
    encoder: P,
    decoder: P,
    vocoder: P,
    data_dir: P,
    lexicon: P,
    feat_scale: f32,
    t_shift: f32,
    target_rms: f32,
    guidance_scale: f32,
}

#[repr(C)]
struct PocketConfig {
    lm_flow: P,
    lm_main: P,
    encoder: P,
    decoder: P,
    text_conditioner: P,
    vocab_json: P,
    token_scores_json: P,
    voice_embedding_cache_capacity: i32,
}

#[repr(C)]
struct SupertonicConfig {
    duration_predictor: P,
    text_encoder: P,
    vector_estimator: P,
    vocoder: P,
    tts_json: P,
    unicode_indexer: P,
    voice_style: P,
}

#[repr(C)]
struct ModelConfig {
    vits: VitsConfig,
    num_threads: i32,
    debug: i32,
    provider: P,
    matcha: MatchaConfig,
    kokoro: KokoroConfig,
    kitten: KittenConfig,
    zipvoice: ZipvoiceConfig,
    pocket: PocketConfig,
    supertonic: SupertonicConfig,
}

#[repr(C)]
struct TtsConfig {
    model: ModelConfig,
    rule_fsts: P,
    max_num_sentences: i32,
    rule_fars: P,
    silence_scale: f32,
}

#[repr(C)]
struct GeneratedAudio {
    samples: *const f32,
    n: i32,
    sample_rate: i32,
}

#[repr(C)]
struct GenerationConfig {
    silence_scale: f32,
    speed: f32,
    sid: i32,
    reference_audio: *const f32,
    reference_audio_len: i32,
    reference_sample_rate: i32,
    reference_text: P,
    num_steps: i32,
    extra: P,
    phoneme_codepoints: *const i32,
    phoneme_codepoints_lens: *const i32,
    phoneme_codepoints_num_sentences: i32,
    tokens: *const P,
    tokens_lens: *const i32,
    tokens_num_sentences: i32,
}

type ProgressCallback = extern "C" fn(*const f32, i32, f32, *mut c_void) -> i32;

#[allow(non_snake_case)]
struct Api {
    GetVersionStr: unsafe extern "C" fn() -> P,
    CreateOfflineTts: unsafe extern "C" fn(*const TtsConfig) -> *const c_void,
    DestroyOfflineTts: unsafe extern "C" fn(*const c_void),
    OfflineTtsSampleRate: unsafe extern "C" fn(*const c_void) -> i32,
    OfflineTtsNumSpeakers: unsafe extern "C" fn(*const c_void) -> i32,
    OfflineTtsGenerateWithConfig:
        unsafe extern "C" fn(*const c_void, P, *const GenerationConfig, Option<ProgressCallback>, *mut c_void) -> *const GeneratedAudio,
    DestroyOfflineTtsGeneratedAudio: unsafe extern "C" fn(*const GeneratedAudio),
}

/// Las DLL de sherpa-onnx y ONNX Runtime cargadas.
pub struct Library {
    ort: HMODULE,
    lib: HMODULE,
    api: Api,
    pub version: String,
}

// Los HMODULE y los punteros a funciones se pueden usar desde cualquier hilo.
unsafe impl Send for Library {}

unsafe fn load(path: &Path) -> Result<HMODULE, String> {
    // Que las dependencias se busquen primero junto a la DLL: Windows trae su
    // propio onnxruntime.dll (otra versión) en System32.
    LoadLibraryExW(&HSTRING::from(path.as_os_str()), None, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)
        .map_err(|e| format!("no se ha podido cargar {}: {e}", path.file_name().unwrap_or_default().to_string_lossy()))
}

unsafe fn sym<T: Copy>(lib: HMODULE, name: &str) -> Result<T, String> {
    let c = CString::new(format!("SherpaOnnx{name}")).unwrap();
    let f = GetProcAddress(lib, PCSTR(c.as_ptr() as *const u8)).ok_or_else(|| format!("falta SherpaOnnx{name}"))?;
    Ok(std::mem::transmute_copy(&f))
}

impl Library {
    pub fn load(dir: &Path) -> Result<Self, String> {
        unsafe {
            let ort = load(&dir.join("onnxruntime.dll"))?;
            let lib = match load(&dir.join("sherpa-onnx-c-api.dll")) {
                Ok(l) => l,
                Err(e) => {
                    let _ = FreeLibrary(ort);
                    return Err(e);
                }
            };
            let api = (|| -> Result<Api, String> {
                Ok(Api {
                    GetVersionStr: sym(lib, "GetVersionStr")?,
                    CreateOfflineTts: sym(lib, "CreateOfflineTts")?,
                    DestroyOfflineTts: sym(lib, "DestroyOfflineTts")?,
                    OfflineTtsSampleRate: sym(lib, "OfflineTtsSampleRate")?,
                    OfflineTtsNumSpeakers: sym(lib, "OfflineTtsNumSpeakers")?,
                    OfflineTtsGenerateWithConfig: sym(lib, "OfflineTtsGenerateWithConfig")?,
                    DestroyOfflineTtsGeneratedAudio: sym(lib, "DestroyOfflineTtsGeneratedAudio")?,
                })
            })();
            let api = match api {
                Ok(a) => a,
                Err(e) => {
                    let _ = FreeLibrary(lib);
                    let _ = FreeLibrary(ort);
                    return Err(e);
                }
            };
            let v = (api.GetVersionStr)();
            let version = if v.is_null() { String::new() } else { CStr::from_ptr(v).to_string_lossy().to_string() };
            Ok(Self { ort, lib, api, version })
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.lib);
            let _ = FreeLibrary(self.ort);
        }
    }
}

/// Un modelo Supertonic cargado.
pub struct Tts {
    handle: *const c_void,
    pub sample_rate: u32,
    pub speakers: i32,
}

unsafe impl Send for Tts {}

/// Archivos de un modelo Supertonic en `dir`.
pub struct SupertonicFiles<'a> {
    pub dir: &'a Path,
    pub threads: i32,
}

impl Tts {
    pub fn supertonic(lib: &Library, files: SupertonicFiles) -> Result<Self, String> {
        let path = |name: &str| CString::new(files.dir.join(name).to_string_lossy().as_bytes()).unwrap();
        let (dp, te, ve, vo, tj, ui, vs) = (
            path("duration_predictor.int8.onnx"),
            path("text_encoder.int8.onnx"),
            path("vector_estimator.int8.onnx"),
            path("vocoder.int8.onnx"),
            path("tts.json"),
            path("unicode_indexer.bin"),
            path("voice.bin"),
        );
        let provider = CString::new("cpu").unwrap();
        unsafe {
            let mut config: TtsConfig = std::mem::zeroed();
            config.model.supertonic = SupertonicConfig {
                duration_predictor: dp.as_ptr(),
                text_encoder: te.as_ptr(),
                vector_estimator: ve.as_ptr(),
                vocoder: vo.as_ptr(),
                tts_json: tj.as_ptr(),
                unicode_indexer: ui.as_ptr(),
                voice_style: vs.as_ptr(),
            };
            config.model.num_threads = files.threads;
            config.model.provider = provider.as_ptr();
            config.max_num_sentences = 1;
            let handle = (lib.api.CreateOfflineTts)(&config);
            if handle.is_null() {
                return Err("sherpa-onnx no ha podido cargar el modelo".into());
            }
            let sample_rate = (lib.api.OfflineTtsSampleRate)(handle).max(1) as u32;
            let speakers = (lib.api.OfflineTtsNumSpeakers)(handle);
            Ok(Self { handle, sample_rate, speakers })
        }
    }

    /// Sintetiza un trozo. `cancel` se consulta mientras genera y lo corta.
    pub fn generate(&self, lib: &Library, text: &str, sid: i32, speed: f32, extra_json: &str, steps: i32, cancel: &dyn Fn() -> bool) -> Result<Vec<f32>, String> {
        let text = CString::new(text.replace('\0', " ")).unwrap();
        let extra = CString::new(extra_json).unwrap();
        extern "C" fn progress(_: *const f32, _: i32, _: f32, arg: *mut c_void) -> i32 {
            let cancel = unsafe { &*(arg as *const &dyn Fn() -> bool) };
            i32::from(!cancel())
        }
        unsafe {
            let mut cfg: GenerationConfig = std::mem::zeroed();
            cfg.sid = sid;
            cfg.speed = speed;
            cfg.num_steps = steps;
            cfg.silence_scale = 0.2;
            cfg.extra = extra.as_ptr();
            let arg: *const &dyn Fn() -> bool = &cancel;
            let audio = (lib.api.OfflineTtsGenerateWithConfig)(self.handle, text.as_ptr(), &cfg, Some(progress), arg as *mut c_void);
            if audio.is_null() {
                return Err("sherpa-onnx no ha generado audio".into());
            }
            let a = &*audio;
            let samples = if a.samples.is_null() || a.n <= 0 { Vec::new() } else { std::slice::from_raw_parts(a.samples, a.n as usize).to_vec() };
            (lib.api.DestroyOfflineTtsGeneratedAudio)(audio);
            Ok(samples)
        }
    }

    pub fn destroy(self, lib: &Library) {
        unsafe { (lib.api.DestroyOfflineTts)(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Los tamaños tienen que coincidir con los de c-api.h en x64 (punteros de 8 bytes).
    #[test]
    fn estructuras_como_en_c() {
        use std::mem::size_of;
        assert_eq!(size_of::<VitsConfig>(), 4 * 8 + 3 * 4 + 4 + 8);
        assert_eq!(size_of::<KokoroConfig>(), 8 * 8);
        assert_eq!(size_of::<SupertonicConfig>(), 7 * 8);
        assert_eq!(size_of::<GeneratedAudio>(), 16);
        // silence_scale, speed, sid, (relleno) ref_audio, len, sr, ref_text, steps, (relleno) extra,
        // codepoints, lens, n, (relleno) tokens, tokens_lens, n, (relleno).
        assert_eq!(size_of::<GenerationConfig>(), 4 + 4 + 4 + 4 + 8 + 4 + 4 + 8 + 4 + 4 + 8 + 8 + 8 + 4 + 4 + 8 + 8 + 4 + 4);
        assert_eq!(std::mem::offset_of!(GenerationConfig, extra), 48);
        assert_eq!(std::mem::offset_of!(ModelConfig, num_threads), size_of::<VitsConfig>());
    }
}
