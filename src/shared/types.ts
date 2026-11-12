/**
 * Contrato entre el backend en Rust (src-tauri) y las ventanas en React.
 * Los nombres de campo coinciden con los structs de Rust (serde camelCase):
 * si cambias algo aquí, cámbialo también en `src-tauri/src/settings.rs` y `src-tauri/src/ipc.rs`.
 */

export const MODELS = [
  { id: 'claude-haiku-5-5', label: 'Haiku 5.5', hint: 'Rápido y barato. Ideal para corregir.' },
  { id: 'claude-sonnet-5-5', label: 'Sonnet 5.5', hint: 'Más fino con traducciones y textos largos.' },
  { id: 'claude-opus-5-5', label: 'Opus 5.5', hint: 'El más capaz. Más lento y caro.' }
] as const

export type ModelId = (typeof MODELS)[number]['id']

export const SEARCH_ENGINES = {
  google: { label: 'Google' },
  duckduckgo: { label: 'DuckDuckGo' },
  bing: { label: 'Bing' }
} as const

export type SearchEngine = keyof typeof SEARCH_ENGINES

export type ColorFormat = 'hex' | 'rgb' | 'hsl'

export type NoticePosition = 'bottomRight' | 'bottomLeft' | 'topRight' | 'topLeft' | 'cursor'

export interface CustomAction {
  id: string
  name: string
  prompt: string
}

/** Aceleradores con el formato "Control+Alt+X" (modificadores: Control, Alt, Shift, Super). Vacío = desactivado. */
export interface Shortcuts {
  /** Paleta sobre la selección: todas las acciones de escritura y «Entender». */
  palette: string
  /** Corrige la selección directamente. */
  correct: string
  /** Traduce la selección directamente (la dirección la decide la detección de idioma). */
  translate: string
  /** Abre la paleta directamente en «Entender» (el antiguo lector). */
  understand: string
  /** Buscador: apps, calculadora, unidades, comandos, web y Claude. */
  launcher: string
  /** Cuentagotas de color. */
  colorPicker: string
  /** Guarda un clip con los últimos minutos de pantalla. */
  saveClip: string
}

export type ClipQuality = 'low' | 'medium' | 'high'

/** Clips: la pantalla se graba en un búfer circular y el atajo guarda los últimos minutos. */
export interface Clips {
  /** Desactivado por defecto: graba la pantalla todo el rato. */
  enabled: boolean
  /** Minutos que se guardan (1–30). */
  minutes: number
  /** Carpeta de destino. Vacío = Vídeos\Winkit. */
  folder: string
  fps: 30 | 60
  quality: ClipQuality
  systemAudio: boolean
  microphone: boolean
  /** Monitor que se graba (id de `clips.monitors()`). Vacío = el principal. */
  monitor: string
  /** Salida cuyo sonido se graba (id de `clips.audioDevices()`). Vacío = la predeterminada de Windows. */
  systemAudioDevice: string
  /** Micrófono que se graba (id de `clips.audioDevices()`). Vacío = el predeterminado de Windows. */
  microphoneDevice: string
}

/** Un monitor conectado (para elegir cuál graban los clips). */
/** Un micrófono o una salida de audio activos. */
export interface AudioDevice {
  /** Id de WASAPI (estable). */
  id: string
  /** Nombre de Windows, p. ej. «Micrófono (Yeti Stereo Microphone)». */
  name: string
  isDefault: boolean
}

export interface AudioDevices {
  inputs: AudioDevice[]
  outputs: AudioDevice[]
}

export interface ClipMonitor {
  /** Id estable: la ruta de dispositivo del monitor. */
  id: string
  /** Nombre del monitor («DELL U2720Q») o «Pantalla 2». */
  name: string
  width: number
  height: number
  primary: boolean
}

export interface ClipsStatus {
  state: 'off' | 'starting' | 'recording' | 'error'
  /** Error legible o detalle (p. ej. el codificador en uso: "NVIDIA NVENC"). */
  message: string | null
  /** Segundos que hay ahora mismo en el búfer (lo que se guardaría). */
  bufferedSeconds: number
  /** Carpeta real donde se guardan (resuelta si `folder` está vacío). */
  folder: string
}

export interface Settings {
  shortcuts: Shortcuts
  /** Con los atajos directos, reemplaza el texto en cuanto Claude termina, sin vista previa. */
  directAutoReplace: boolean
  /** Si no hay nada seleccionado, selecciona todo el campo (Ctrl+A) antes de copiar. */
  selectAllFallback: boolean
  /** Idiomas que aparecen como destino, en español ("Inglés", "Francés"…). */
  languages: string[]
  /** Tu idioma: al que se traduce lo que te escriben («Entender») y lo que ya está en el idioma de destino. */
  myLanguage: string
  /** Idioma al que traduces lo que escribes tú. */
  translateTarget: string
  customActions: CustomAction[]
  launchAtLogin: boolean
  searchEngine: SearchEngine
  colorFormat: ColorFormat
  /** Dónde salen los avisos (clip guardado, color copiado…). "cursor" = junto al ratón. */
  noticePosition: NoticePosition
  clips: Clips
  ai: Ai
  voice: Voice
}

/** Acciones integradas de la paleta. Las personalizadas usan su id (`custom:...`). */
export type BuiltinAction = 'correct' | 'translate' | 'understand' | 'improve' | 'formal' | 'casual' | 'shorter'

export interface RunRequest {
  /** Acción integrada, id de una personalizada, o 'instruction' para una instrucción libre. */
  action: BuiltinAction | 'instruction' | string
  text: string
  /** Idioma de destino para 'translate' / 'understand'. Si falta, lo decide la detección. */
  language?: string
  /** Texto de la instrucción libre cuando `action` es 'instruction'. */
  instruction?: string
}

/** Resultado de detectar el idioma de un texto, ya con los destinos calculados. */
export interface LanguageInfo {
  /** Idioma detectado (nombre en español) o null si el texto es corto o ambiguo. */
  detected: string | null
  /** Destino de «Traducir»: tu idioma de destino, o tu idioma si el texto ya estaba en el de destino. */
  translateTo: string
  /** Destino de «Entender»: tu idioma, o el de destino si el texto ya estaba en el tuyo. */
  understandTo: string
}

export type RunEvent =
  /** Primer evento: modelo y, si es una traducción, de qué idioma a cuál. */
  | { type: 'start'; model: string; from: string | null; to: string | null }
  /** Texto acumulado hasta ahora (no solo el trozo nuevo). */
  | { type: 'delta'; text: string }
  | { type: 'done'; text: string }
  | { type: 'error'; message: string; openSettings: boolean }

export type PaletteMode =
  /** Lista de acciones sobre el texto capturado. */
  | 'menu'
  /** Atajo directo: la paleta solo muestra el progreso y el resultado de `request`. */
  | 'run'
  /** Panel grande: original y traducción lado a lado, sin tocar la app de origen. */
  | 'understand'
  /** Aviso breve (sin texto seleccionado, color copiado…). Se cierra solo salvo que `openSettings`. */
  | 'notice'

export interface PaletteInit {
  /** Cambia en cada apertura: sirve para reiniciar el estado de la ventana. */
  session: number
  mode: PaletteMode
  /** Texto capturado (vacío en «Entender» abierto desde la bandeja o el buscador). */
  text: string
  language: LanguageInfo
  /** Solo en mode 'run'. */
  request: RunRequest | null
  /** Solo en mode 'run': Rust pegará el resultado él solo al terminar. */
  autoReplace: boolean
  /** Hay una ventana de origen donde pegar. False si se abrió sin selección. */
  canReplace: boolean
  notice: { message: string; openSettings: boolean; color: string | null } | null
  languages: string[]
  myLanguage: string
  translateTarget: string
  customActions: CustomAction[]
  modelLabel: string
}

export interface AppEntry {
  /** Nombre de análisis del shell (AUMID o ruta): se le pasa tal cual a `launchApp`. */
  id: string
  name: string
}

export interface LauncherInit {
  session: number
  engineLabel: string
  modelLabel: string
}

export interface ShortcutStatus {
  key: keyof Shortcuts
  ok: boolean
}

export interface KeyTestResult {
  ok: boolean
  message: string
}

export interface SettingsState {
  settings: Settings
  hasKey: boolean
  /** La key viene de la variable de entorno ANTHROPIC_API_KEY: no se puede cambiar desde Ajustes. */
  keyFromEnv: boolean
  shortcuts: ShortcutStatus[]
  version: string
}

/** Entorno visual, igual para todas las ventanas. */
export interface UiEnv {
  /** 'acrylic' en Windows 11 (la ventana es translúcida); 'none' en Windows 10 (fondo sólido). */
  backdrop: 'acrylic' | 'none'
  /** Color de acento de Windows, "#rrggbb". */
  accent: string
}

export interface UpdateStatus {
  /**
   * idle: aún no se ha comprobado · checking · downloading · ready: descargada,
   * se instala al reiniciar · upToDate · error
   */
  state: 'idle' | 'checking' | 'downloading' | 'ready' | 'upToDate' | 'error'
  /** Versión nueva (en downloading/ready). */
  version: string | null
  /** 0–1 durante la descarga, si se conoce el tamaño. */
  progress: number | null
  /** Notas de la release o mensaje de error. */
  message: string | null
}

// ── IA: proveedores de texto ─────────────────────────────────────────────
// Rust: src-tauri/src/settings.rs (Ai, ModelRef…) y src-tauri/src/ai/.

/**
 * "anthropic" | "openai" | "deepseek" | "gemini" | "openrouter" | "custom"
 * (cualquier API compatible con OpenAI) | "local" (la IA local de Winkit).
 */
export type ProviderId = 'anthropic' | 'openai' | 'deepseek' | 'gemini' | 'openrouter' | 'custom' | 'local'

/** Herramientas que usan IA; cada una puede tener su propio modelo. */
export type AiTool = 'writing' | 'understand' | 'launcher' | 'assistant'

export interface ModelRef {
  provider: ProviderId
  /** Id del modelo en ese proveedor ("claude-haiku-5-5", "gpt-…", id del catálogo local…). */
  model: string
}

export interface CustomProvider {
  name: string
  /** Raíz de la API compatible con OpenAI, p. ej. "http://192.168.1.20:8000/v1". */
  baseUrl: string
}

export interface LocalAi {
  enabled: boolean
  /** Id del modelo del catálogo ("" = ninguno instalado). */
  model: string
  gpu: boolean
  context: number
  /** Minutos sin uso tras los que se libera la memoria (0 = nunca). */
  unloadAfterMinutes: number
}

export interface Ai {
  default: ModelRef
  /** Si una herramienta no está, usa `default`. */
  tools: Partial<Record<AiTool, ModelRef>>
  custom: CustomProvider
  local: LocalAi
}

export interface ProviderInfo {
  id: ProviderId
  /** "Claude (Anthropic)", "OpenAI", "Mi servidor"… */
  name: string
  needsKey: boolean
  hasKey: boolean
  /** La key viene de una variable de entorno y no se puede cambiar aquí. */
  keyFromEnv: boolean
  /** Dónde se consigue la key (consola del proveedor). */
  keyUrl: string | null
  /** Listo para usarse (key puesta, servidor configurado o IA local instalada). */
  ready: boolean
}

export interface ModelInfo {
  id: string
  name: string
  /** Nota corta: "Rápido y barato", "4,9 GB", "Recomendado"… */
  hint: string | null
}

export interface AiTestResult {
  ok: boolean
  /** Texto de error, o la respuesta de prueba. */
  message: string
  /** Latencia hasta la respuesta completa. */
  ms: number
}

/** Un modelo del catálogo de la IA local. */
export interface LocalModel {
  id: string
  name: string
  /** Para qué sirve, en una línea. */
  description: string
  sizeMb: number
  /** Memoria aproximada que necesita (VRAM con GPU, RAM sin ella). */
  memoryGb: number
  installed: boolean
  recommended: boolean
}

export interface LocalAiStatus {
  /** off · downloading · verifying · starting · ready (cargado) · idle (instalado, sin cargar) · error */
  state: 'off' | 'downloading' | 'verifying' | 'starting' | 'ready' | 'idle' | 'error'
  /** 0–1 durante la descarga. */
  progress: number | null
  /** Qué se descarga ("Motor llama.cpp", "Qwen…") o el error. */
  message: string | null
  /** "GPU (Vulkan): NVIDIA GeForce…" o "CPU". */
  backend: string | null
  /** Modelo cargado o instalado. */
  model: string | null
}

// ── Voz (texto a voz) ────────────────────────────────────────────────────
// Rust: src-tauri/src/voice/.

export type VoiceEngineId = 'windows' | 'kokoro' | 'elevenlabs' | 'openai'

export interface Voice {
  engine: VoiceEngineId
  /** Id de la voz en ese motor ("" = la predeterminada). */
  voice: string
  /** 1 = normal (0.5–2). */
  speed: number
  /** 0–1. */
  volume: number
}

export interface VoiceEngineInfo {
  id: VoiceEngineId
  name: string
  /** "Gratis, sin conexión", "De pago: ~0,04–0,08 $ / 1.000 caracteres"… */
  hint: string
  needsKey: boolean
  hasKey: boolean
  keyFromEnv: boolean
  keyUrl: string | null
  /** Hay que descargarlo antes de usarlo (Kokoro). */
  needsDownload: boolean
  installed: boolean
  downloadMb: number | null
  ready: boolean
}

export interface VoiceInfo {
  id: string
  name: string
  /** "es-ES", "en-US"… */
  language: string
  /** "Mujer", "Hombre" o null. */
  gender: string | null
}

export interface VoiceStatus {
  /** idle · downloading · loading · speaking · error */
  state: 'idle' | 'downloading' | 'loading' | 'speaking' | 'error'
  progress: number | null
  message: string | null
}
