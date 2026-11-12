//! Lo que descarga la IA local, fijado a versiones concretas y verificado con
//! SHA-256. Los hashes se calcularon descargando cada archivo una vez (y
//! coinciden con los que publican GitHub y Hugging Face).

/// Una compilación oficial de llama.cpp para Windows x64.
pub struct Engine {
    /// "vulkan" o "cpu".
    pub variant: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// La compilación de la versión estable v0.9.0 de llama.cpp (el tag b12110).
pub const ENGINE_TAG: &str = "b12110";

/// Vulkan: cualquier GPU moderna (NVIDIA, AMD, Intel) sin instalar CUDA, y trae
/// también los motores de CPU por si la GPU falla.
pub const ENGINE_VULKAN: Engine = Engine {
    variant: "vulkan",
    url: "https://github.com/ggml-org/llama.cpp/releases/download/b12110/llama-b12110-bin-win-vulkan-x64.zip",
    size: 37_661_146,
    sha256: "9732a5e48887ca54d0bfe4af4324dc85fd56cd8cda19b52df3542fa6ebe6874f",
};

/// Solo CPU: para equipos sin GPU o si Vulkan no arranca.
pub const ENGINE_CPU: Engine = Engine {
    variant: "cpu",
    url: "https://github.com/ggml-org/llama.cpp/releases/download/b12110/llama-b12110-bin-win-cpu-x64.zip",
    size: 24_708_445,
    sha256: "98901e327c252875accaf2d1bf364e10e868843b0b63468baad1aa945cdc5da2",
};

pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Nombre del archivo en disco.
    pub file: &'static str,
    /// Repositorio de Hugging Face fijado a un commit (no a `main`, que cambia).
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    /// Memoria aproximada con el contexto por defecto (VRAM con GPU, RAM sin ella).
    pub memory_gb: f32,
    pub recommended: bool,
}

/// Elegidos probándolos aquí con las tareas de Winkit en español (corregir,
/// traducir, entender jerga, cambiar el tono). Gemma 4 es la familia más fina en
/// español a este tamaño; Qwen 3.5 razona mejor y va bien para preguntas.
pub const MODELS: [Model; 4] = [
    Model {
        id: "gemma-4-e2b",
        name: "Gemma 4 E2B",
        description: "Rápido y ligero. Corrige y traduce muy bien; va bien incluso sin GPU.",
        file: "gemma-4-E2B-it-Q4_0.gguf",
        url: "https://huggingface.co/ggml-org/gemma-4-E2B-it-GGUF/resolve/2a8538c2e2d4687953a86c422c7c623cfd022880/gemma-4-E2B-it-Q4_0.gguf",
        size: 2_841_481_184,
        sha256: "8e30dff3ac4c8434c49a7036fa15564bdbb6044e42bf04550bf1a096ad7e6a52",
        memory_gb: 2.5,
        recommended: true,
    },
    Model {
        id: "gemma-4-e4b",
        name: "Gemma 4 E4B",
        description: "Más fino con textos largos, tono y traducciones. Pide una GPU con 6 GB o más.",
        file: "gemma-4-E4B-it-Q4_0.gguf",
        url: "https://huggingface.co/ggml-org/gemma-4-E4B-it-GGUF/resolve/8903cb408c7df39cde2ed15fac397e6d7be92951/gemma-4-E4B-it-Q4_0.gguf",
        size: 4_590_807_392,
        sha256: "a555b900214b477d8880e7832e0b8925e139b0159640036b09fe472b6f2097f2",
        memory_gb: 3.5,
        recommended: false,
    },
    Model {
        id: "qwen3.5-4b",
        name: "Qwen 3.5 4B",
        description: "Bueno respondiendo preguntas en el buscador; algo menos natural en español.",
        file: "Qwen3.5-4B-Q4_K_M.gguf",
        url: "https://huggingface.co/ggml-org/Qwen3.5-4B-GGUF/resolve/8d43d974efc48399cb58957ad49c8b26bf5a109c/Qwen3.5-4B-Q4_K_M.gguf",
        size: 3_032_091_584,
        sha256: "217002fa8008ef4fc6d548353a3aebef055a96e6ff8e0e2c0953ef8254a89c14",
        memory_gb: 3.5,
        recommended: false,
    },
    Model {
        id: "gemma-4-12b",
        name: "Gemma 4 12B",
        description: "El más capaz, de Google. Necesita una GPU de 10–12 GB; con menos, va lento.",
        file: "gemma-4-12b-it-qat-q4_0.gguf",
        url: "https://huggingface.co/google/gemma-4-12B-it-qat-q4_0-gguf/resolve/29d097773436b69ff9feafd636ab4cf873786537/gemma-4-12b-it-qat-q4_0.gguf",
        size: 6_975_879_296,
        sha256: "93567e57a8fe10b23569b9d9ec38cd005deedf71e29477c421a4b83f418a538b",
        memory_gb: 8.5,
        recommended: false,
    },
];

pub fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sha256(s: &str) -> bool {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    #[test]
    fn el_catalogo_esta_completo_y_fijado() {
        for e in [&ENGINE_VULKAN, &ENGINE_CPU] {
            assert!(e.url.starts_with("https://github.com/ggml-org/llama.cpp/releases/download/"), "{}", e.url);
            assert!(e.url.contains(ENGINE_TAG) && e.url.ends_with(".zip") && e.url.contains(e.variant));
            assert!(is_sha256(e.sha256) && e.size > 1_000_000);
        }
        let mut ids = std::collections::HashSet::new();
        for m in &MODELS {
            assert!(ids.insert(m.id), "id repetido: {}", m.id);
            assert!(m.url.starts_with("https://huggingface.co/") && m.url.ends_with(m.file), "{}", m.url);
            // Fijado a un commit de 40 caracteres, no a una rama.
            let rev = m.url.split("/resolve/").nth(1).and_then(|r| r.split('/').next()).unwrap();
            assert!(rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit()), "{}", m.url);
            assert!(is_sha256(m.sha256), "{}", m.id);
            assert!(m.size > 500_000_000 && m.memory_gb > 0.0 && !m.description.is_empty());
            assert!(m.file.ends_with(".gguf"));
        }
        assert_eq!(MODELS.iter().filter(|m| m.recommended).count(), 1);
        // El recomendado es el más pequeño: el que se prueba de punta a punta.
        let smallest = MODELS.iter().min_by_key(|m| m.size).unwrap();
        assert!(smallest.recommended);
    }
}
