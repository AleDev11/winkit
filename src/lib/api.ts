import { Channel, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AudioDevices,
  ClipMonitor,
  ClipsStatus,
  UpdateStatus,
  AppEntry,
  KeyTestResult,
  LanguageInfo,
  LauncherInit,
  PaletteInit,
  RunEvent,
  RunRequest,
  Settings,
  SettingsState,
  ShortcutStatus,
  UiEnv
} from '../shared/types'

/**
 * Todo lo que las ventanas le piden al backend en Rust. Cada función es un
 * `#[tauri::command]` de src-tauri/src/ipc.rs con el mismo nombre en snake_case.
 *
 * Las ventanas se crean al arrancar y se reutilizan: Rust emite `<ventana>:init`
 * cada vez que se abre una. Como el evento puede llegar antes de que React monte,
 * `onInit` también pide el último estado al montar.
 */

function channel(onEvent: (ev: RunEvent) => void): Channel<RunEvent> {
  const ch = new Channel<RunEvent>()
  ch.onmessage = onEvent
  return ch
}

function onInit<T>(event: string, current: string, cb: (init: T) => void): () => void {
  let unlisten: UnlistenFn | null = null
  let disposed = false
  void listen<T>(event, (e) => cb(e.payload)).then((u) => {
    if (disposed) u()
    else unlisten = u
  })
  void invoke<T | null>(current).then((init) => {
    if (init && !disposed) cb(init)
  })
  return () => {
    disposed = true
    unlisten?.()
  }
}

export const ui = {
  env: (): Promise<UiEnv> => invoke('ui_env'),
  copy: (text: string): Promise<void> => invoke('copy_text', { text }),
  openSettings: (): Promise<void> => invoke('open_settings'),
  /** Abre una URL http(s) en el navegador predeterminado (mismo comando que `launcher.openUrl`). */
  openUrl: (url: string): Promise<void> => invoke('open_url', { url })
}

export const palette = {
  onInit: (cb: (init: PaletteInit) => void) => onInit<PaletteInit>('palette:init', 'palette_current', cb),
  /** Tamaño del contenido en px CSS; Rust coloca la ventana junto al cursor (o centrada). */
  resize: (width: number, height: number): Promise<void> => invoke('palette_resize', { width, height }),
  /** Lanza una acción. Cancela la anterior de la paleta. Los eventos llegan por `onEvent`. */
  run: (request: RunRequest, onEvent: (ev: RunEvent) => void): Promise<void> =>
    invoke('palette_run', { request, onEvent: channel(onEvent) }),
  cancel: (): Promise<void> => invoke('palette_cancel'),
  detect: (text: string): Promise<LanguageInfo> => invoke('detect_language', { text }),
  /** Pega `text` en la ventana de origen sobre la selección y oculta la paleta. */
  replace: (text: string): Promise<void> => invoke('palette_replace', { text }),
  /** Copia `text` y cierra. */
  copyAndClose: (text: string): Promise<void> => invoke('palette_copy', { text }),
  close: (): Promise<void> => invoke('palette_close')
}

export const launcher = {
  onInit: (cb: (init: LauncherInit) => void) => onInit<LauncherInit>('launcher:init', 'launcher_current', cb),
  resize: (width: number, height: number): Promise<void> => invoke('launcher_resize', { width, height }),
  apps: (): Promise<AppEntry[]> => invoke('list_apps'),
  /** Icono de la app como data URL PNG, o null. */
  icon: (id: string): Promise<string | null> => invoke('app_icon', { id }),
  openApp: (id: string): Promise<void> => invoke('launch_app', { id }),
  webSearch: (query: string): Promise<void> => invoke('web_search', { query }),
  openUrl: (url: string): Promise<void> => invoke('open_url', { url }),
  /** Ver SYSTEM_COMMANDS en shared/commands.ts. Oculta el buscador. */
  systemCommand: (id: string): Promise<void> => invoke('system_command', { id }),
  ask: (question: string, onEvent: (ev: RunEvent) => void): Promise<void> =>
    invoke('launcher_ask', { question, onEvent: channel(onEvent) }),
  /** Copia y oculta. */
  copyAndClose: (text: string): Promise<void> => invoke('launcher_copy', { text }),
  hide: (): Promise<void> => invoke('launcher_hide')
}

export const settings = {
  get: (): Promise<SettingsState> => invoke('settings_get'),
  save: (patch: Partial<Settings>): Promise<{ settings: Settings; shortcuts: ShortcutStatus[] }> =>
    invoke('settings_save', { patch }),
  /** Prueba la key contra la API y, si funciona, la guarda cifrada (DPAPI). */
  saveKey: (key: string): Promise<KeyTestResult> => invoke('api_key_save', { key }),
  testKey: (): Promise<KeyTestResult> => invoke('api_key_test'),
  /** Mientras se graba un atajo, los globales no deben tragarse la combinación. */
  suspendShortcuts: (): Promise<void> => invoke('shortcuts_suspend'),
  resumeShortcuts: (): Promise<ShortcutStatus[]> => invoke('shortcuts_resume')
}

export interface Capture {
  /** Píxeles físicos. */
  width: number
  height: number
  /** RGBA, fila a fila. */
  pixels: Uint8ClampedArray
}

export const picker = {
  onInit: (cb: (init: { session: number }) => void) =>
    onInit<{ session: number }>('picker:init', 'picker_current', cb),
  /** La captura del monitor (congelada al pulsar el atajo). */
  capture: async (): Promise<Capture> => {
    const buf = new Uint8Array(await invoke<ArrayBuffer>('picker_capture'))
    const view = new DataView(buf.buffer, buf.byteOffset, 8)
    return {
      width: view.getUint32(0, true),
      height: view.getUint32(4, true),
      pixels: new Uint8ClampedArray(buf.buffer, buf.byteOffset + 8)
    }
  },
  /** Copia el color en el formato de Ajustes, cierra y muestra un aviso. Null = cancelar. */
  done: (color: { r: number; g: number; b: number } | null): Promise<void> => invoke('picker_done', { color })
}

function subscribe<T>(event: string, cb: (payload: T) => void): () => void {
  let unlisten: UnlistenFn | null = null
  let disposed = false
  void listen<T>(event, (e) => cb(e.payload)).then((u) => {
    if (disposed) u()
    else unlisten = u
  })
  return () => {
    disposed = true
    unlisten?.()
  }
}

/** Clips (búfer de pantalla). Se activan y configuran con `settings.save({ clips })`. */
export const clips = {
  status: (): Promise<ClipsStatus> => invoke('clips_status'),
  /** Monitores conectados, el principal primero. */
  monitors: (): Promise<ClipMonitor[]> => invoke('clips_monitors'),
  /** Micrófonos y salidas de audio activos. */
  audioDevices: (): Promise<AudioDevices> => invoke('clips_audio_devices'),
  /** Pico (0–1) del micrófono desde la consulta anterior, para un medidor. Se cierra solo si se deja de preguntar. */
  micLevel: (device: string): Promise<number> => invoke('clips_mic_level', { device }),
  /** Cambios de estado (arranque, error, segundos en el búfer cada pocos segundos). */
  onStatus: (cb: (status: ClipsStatus) => void) => subscribe<ClipsStatus>('clips:status', cb),
  /** Selector de carpeta nativo. Devuelve la ruta elegida (ya guardada en ajustes) o null si se cancela. */
  chooseFolder: (): Promise<string | null> => invoke('clips_choose_folder'),
  openFolder: (): Promise<void> => invoke('clips_open_folder'),
  /** Lo mismo que el atajo: guarda los últimos minutos. Devuelve la ruta del vídeo. */
  save: (): Promise<string> => invoke('clips_save')
}

/** Actualizaciones desde las releases de GitHub. */
export const updates = {
  status: (): Promise<UpdateStatus> => invoke('update_status'),
  onStatus: (cb: (status: UpdateStatus) => void) => subscribe<UpdateStatus>('update:status', cb),
  /** Busca una versión nueva ahora (y la descarga si la hay). */
  check: (): Promise<UpdateStatus> => invoke('update_check'),
  /** Instala la versión descargada y reinicia Winkit. */
  restart: (): Promise<void> => invoke('update_restart')
}
