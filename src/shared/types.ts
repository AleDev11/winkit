export const MODELS = [
  { id: 'claude-haiku-5-5', label: 'Haiku 5.5', hint: 'Rápido y barato. Ideal para corregir.' },
  { id: 'claude-sonnet-5-5', label: 'Sonnet 5.5', hint: 'Más fino con traducciones y textos largos.' },
  { id: 'claude-opus-5-5', label: 'Opus 5.5', hint: 'El más capaz. Más lento y caro.' }
] as const

export type ModelId = (typeof MODELS)[number]['id']

/** Acciones integradas. Las personalizadas usan su propio id (`custom:...`). */
export type BuiltinActionId = 'correct' | 'translate' | 'read' | 'formal' | 'casual' | 'shorter' | 'clearer'

export const SEARCH_ENGINES = {
  google: { label: 'Google', url: 'https://www.google.com/search?q=' },
  duckduckgo: { label: 'DuckDuckGo', url: 'https://duckduckgo.com/?q=' },
  bing: { label: 'Bing', url: 'https://www.bing.com/search?q=' }
} as const

export type SearchEngine = keyof typeof SEARCH_ENGINES

export interface CustomAction {
  id: string
  name: string
  prompt: string
}

export interface Shortcuts {
  /** Abre la viñeta con el menú de acciones. */
  menu: string
  /** Corrige la selección directamente. */
  correct: string
  /** Traduce la selección al idioma principal (o al secundario si ya está en el principal). */
  translate: string
  /** Abre el lector: el texto seleccionado y su traducción, lado a lado. */
  reader: string
  /** Abre el buscador (calculadora, apps, web, Claude). */
  launcher: string
}

export interface Settings {
  model: ModelId
  shortcuts: Shortcuts
  /** Con los atajos directos, reemplaza el texto en cuanto Claude termina, sin vista previa. */
  directAutoReplace: boolean
  /** Si no hay nada seleccionado, selecciona todo el campo (Ctrl+A) antes de copiar. */
  selectAllFallback: boolean
  languages: string[]
  translateTarget: string
  translateFallback: string
  customActions: CustomAction[]
  launchAtLogin: boolean
  /** Idioma al que el lector traduce lo que te escriben. */
  readerLanguage: string
  searchEngine: SearchEngine
}

export const DEFAULT_SETTINGS: Settings = {
  model: 'claude-haiku-5-5',
  shortcuts: {
    menu: 'Control+Alt+X',
    correct: 'Control+Alt+C',
    translate: 'Control+Alt+T',
    reader: 'Control+Alt+L',
    launcher: 'Alt+Space'
  },
  directAutoReplace: true,
  selectAllFallback: true,
  languages: ['Inglés', 'Español', 'Catalán', 'Francés', 'Italiano', 'Alemán', 'Portugués'],
  translateTarget: 'Inglés',
  translateFallback: 'Español',
  customActions: [],
  launchAtLogin: true,
  readerLanguage: 'Español',
  searchEngine: 'google'
}

/** Lo que se le pide a Claude: una acción y, si es traducir, el idioma. */
export interface RunRequest {
  action: BuiltinActionId | string
  language?: string
}

export type PopupView =
  | { kind: 'menu'; text: string }
  /** Atajo directo: la viñeta solo muestra el progreso y el resultado. */
  | { kind: 'run'; text: string; request: RunRequest; autoReplace: boolean }
  | { kind: 'notice'; message: string; openSettings?: boolean }

export interface PopupInit {
  view: PopupView
  languages: string[]
  translateTarget: string
  customActions: CustomAction[]
  modelLabel: string
}

export type RunEvent = (
  | { type: 'delta'; text: string }
  | { type: 'done'; text: string }
  | { type: 'error'; message: string; openSettings?: boolean }
) & {
  /** Petición a la que pertenece, en las ventanas que pueden lanzar varias seguidas. */
  id?: number
}

export interface ReaderInit {
  text: string
  language: string
  languages: string[]
  modelLabel: string
}

/** Una app del menú Inicio: acceso directo (.lnk/.url) o app de la Tienda (AppID). */
export interface AppEntry {
  id: string
  name: string
  kind: 'shortcut' | 'store'
}

export interface LauncherInit {
  apps: AppEntry[]
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
