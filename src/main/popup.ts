import { BrowserWindow, clipboard, ipcMain, screen } from 'electron'
import { join } from 'path'
import { MODELS, type PopupInit, type PopupView, type RunEvent, type RunRequest } from '../shared/types'
import { captureSelection, pasteInto } from './capture'
import { run, WinkitError } from './claude'
import { getForegroundWindow, hwndOf, setForegroundWindow } from './native'
import { getApiKey, getSettings, hasApiKey } from './settings'

/**
 * La viñeta: una sola ventana sin marco, creada al arrancar y reutilizada, que
 * aparece junto al cursor. El renderer mide su tarjeta y nos pide el tamaño.
 */
const PAD = 16 // margen transparente para la sombra de la tarjeta
const GAP = 14 // separación con el cursor

let win: BrowserWindow | null = null
let anchor = { x: 0, y: 0 }
let focused = false
let busy = false
let noticeTimer: NodeJS.Timeout | null = null

interface Session {
  hwnd: number
  text: string
  abort?: AbortController
}
let session: Session | null = null

export type Trigger = 'menu' | 'correct' | 'translate'

export function createPopup(openSettings: () => void): void {
  win = new BrowserWindow({
    width: 460,
    height: 200,
    show: false,
    frame: false,
    transparent: true,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    hasShadow: false,
    title: 'Winkit',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: false,
      backgroundThrottling: false
    }
  })
  win.setAlwaysOnTop(true, 'pop-up-menu')
  if (process.env['ELECTRON_RENDERER_URL']) win.loadURL(`${process.env['ELECTRON_RENDERER_URL']}/popup.html`)
  else win.loadFile(join(__dirname, '../renderer/popup.html'))

  win.on('focus', () => (focused = true))
  // Clic fuera: se cierra, como un menú contextual.
  win.on('blur', () => {
    if (focused) close(false)
    focused = false
  })

  ipcMain.on('popup:size', (_e, size: { width: number; height: number }) => place(size))
  ipcMain.on('popup:run', (_e, req: RunRequest) => void start(req, false))
  ipcMain.on('popup:replace', (_e, text: string) => void replace(text))
  ipcMain.on('popup:copy', (_e, text: string) => {
    void clipboard.writeText(text)
    close(true)
  })
  ipcMain.on('popup:close', () => close(true))
  ipcMain.on('popup:settings', () => {
    close(false)
    openSettings()
  })
}

/** Coloca la tarjeta abajo a la derecha del cursor; si no cabe, por encima o a la izquierda. */
function place(size: { width: number; height: number }): void {
  if (!win) return
  const width = Math.ceil(size.width) + PAD * 2
  const height = Math.ceil(size.height) + PAD * 2
  const { workArea: wa } = screen.getDisplayNearestPoint(anchor)
  let x = anchor.x + GAP - PAD
  let y = anchor.y + GAP - PAD
  if (x + width > wa.x + wa.width) x = anchor.x - GAP - width + PAD
  if (y + height > wa.y + wa.height) y = anchor.y - GAP - height + PAD
  x = Math.max(wa.x, Math.min(x, wa.x + wa.width - width))
  y = Math.max(wa.y, Math.min(y, wa.y + wa.height - height))
  win.setBounds({ x: Math.round(x), y: Math.round(y), width, height })
}

function send(channel: string, payload: unknown): void {
  win?.webContents.send(channel, payload)
}

function show(view: PopupView, focus: boolean): void {
  if (!win) return
  if (noticeTimer) clearTimeout(noticeTimer)
  const s = getSettings()
  const init: PopupInit = {
    view,
    languages: s.languages,
    translateTarget: s.translateTarget,
    customActions: s.customActions,
    modelLabel: MODELS.find((m) => m.id === s.model)?.label ?? s.model
  }
  send('popup:init', init)
  // Hasta que el renderer mida la tarjeta, que al menos no asome en la posición anterior.
  if (!win.isVisible()) win.setPosition(anchor.x + GAP - PAD, anchor.y + GAP - PAD)
  if (focus) {
    win.show()
    win.focus()
  } else {
    // Sin foco, Windows no la sube en el orden Z: quedaría debajo de apps "siempre encima".
    win.showInactive()
    win.moveTop()
  }
}

function notice(message: string, openSettings = false): void {
  show({ kind: 'notice', message, openSettings }, openSettings)
  if (!openSettings) noticeTimer = setTimeout(() => close(false), 2200)
}

function close(restoreFocus: boolean): void {
  session?.abort?.abort()
  const hwnd = session?.hwnd
  session = null
  focused = false
  if (!win?.isVisible()) return
  if (restoreFocus && hwnd) setForegroundWindow(hwnd)
  win.hide()
}

export async function trigger(kind: Trigger): Promise<void> {
  if (busy || !win) return
  // El mismo atajo con la viñeta abierta la cierra.
  if (win.isVisible() && focused) return close(true)
  busy = true
  try {
    close(false)
    anchor = screen.getCursorScreenPoint()
    const s = getSettings()
    const cap = await captureSelection(s.selectAllFallback)
    if (cap.hwnd === hwndOf(win.getNativeWindowHandle())) return
    if (!cap.text.trim()) return notice('No hay texto seleccionado.')
    if (!hasApiKey()) return notice('Añade tu API key de Anthropic para empezar.', true)

    session = { hwnd: cap.hwnd, text: cap.text }
    if (kind === 'menu') return show({ kind: 'menu', text: cap.text }, true)

    const request: RunRequest = kind === 'correct' ? { action: 'correct' } : { action: 'translate', language: s.translateTarget }
    show({ kind: 'run', text: cap.text, request, autoReplace: s.directAutoReplace }, !s.directAutoReplace)
    void start(request, s.directAutoReplace)
  } finally {
    busy = false
  }
}

async function start(request: RunRequest, autoReplace: boolean): Promise<void> {
  const current = session
  if (!current) return
  current.abort?.abort()
  const abort = new AbortController()
  current.abort = abort
  const s = getSettings()
  try {
    const text = await run({
      apiKey: getApiKey(),
      model: s.model,
      text: current.text,
      request,
      customActions: s.customActions,
      translateFallback: request.action === 'translate' && autoReplace ? s.translateFallback : '',
      signal: abort.signal,
      onDelta: (t) => send('run:event', { type: 'delta', text: t } satisfies RunEvent)
    })
    if (abort.signal.aborted || session !== current) return
    send('run:event', { type: 'done', text } satisfies RunEvent)
    // Solo pegamos a ciegas si el usuario sigue en la ventana de la que salió el texto.
    if (autoReplace && getForegroundWindow() === current.hwnd) return replace(text)
    if (!focused) {
      win?.show()
      win?.focus()
    }
  } catch (err) {
    if (abort.signal.aborted || session !== current) return
    const e = err instanceof WinkitError ? err : new WinkitError(String(err))
    send('run:event', { type: 'error', message: e.message, openSettings: e.openSettings } satisfies RunEvent)
    if (!focused) {
      win?.show()
      win?.focus()
    }
  }
}

async function replace(text: string): Promise<void> {
  const current = session
  if (!current || !win) return
  session = null
  focused = false
  // Primero el foco a la app original y luego ocultar: así Windows no lo da a otra ventana.
  setForegroundWindow(current.hwnd)
  win.hide()
  await pasteInto(current.hwnd, text)
}
