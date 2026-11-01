import { BrowserWindow, clipboard, ipcMain, screen } from 'electron'
import { join } from 'path'
import { MODELS, type ReaderInit, type RunEvent } from '../shared/types'
import { captureSelection } from './capture'
import { run, WinkitError } from './claude'
import { getApiKey, getSettings } from './settings'

/**
 * El lector: para entender lo que te escriben. Una ventana grande con el texto
 * original a un lado y la traducción al otro. No toca la app de origen.
 */
let win: BrowserWindow | null = null
let ready: Promise<void> | null = null
let abort: AbortController | null = null
let busy = false

function create(icon: string): BrowserWindow {
  const { workArea: wa } = screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
  const width = Math.min(1180, wa.width - 80)
  const height = Math.min(720, wa.height - 80)
  const w = new BrowserWindow({
    width,
    height,
    x: Math.round(wa.x + (wa.width - width) / 2),
    y: Math.round(wa.y + (wa.height - height) / 2),
    minWidth: 640,
    minHeight: 360,
    show: false,
    title: 'Winkit · Lector',
    icon,
    backgroundColor: '#121214',
    titleBarStyle: 'hidden',
    titleBarOverlay: { color: '#121214', symbolColor: '#a8a8b0', height: 44 },
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: false
    }
  })
  ready = new Promise((resolve) => w.webContents.once('did-finish-load', () => resolve()))
  if (process.env['ELECTRON_RENDERER_URL']) w.loadURL(`${process.env['ELECTRON_RENDERER_URL']}/reader.html`)
  else w.loadFile(join(__dirname, '../renderer/reader.html'))
  w.on('closed', () => {
    abort?.abort()
    win = null
  })
  return w
}

function send(channel: string, payload: unknown): void {
  win?.webContents.send(channel, payload)
}

/** Abre el lector con la selección actual; sin selección se abre vacío para pegar texto. */
export async function openReader(icon: string): Promise<void> {
  if (busy) return
  busy = true
  try {
    const { text } = await captureSelection(false)
    if (!win) win = create(icon)
    await ready
    const s = getSettings()
    const init: ReaderInit = {
      text: text.trim(),
      language: s.readerLanguage,
      languages: s.languages,
      modelLabel: MODELS.find((m) => m.id === s.model)?.label ?? s.model
    }
    send('reader:init', init)
    if (win.isMinimized()) win.restore()
    win.show()
    win.focus()
  } finally {
    busy = false
  }
}

export function registerReader(): void {
  ipcMain.on('reader:translate', async (_e, req: { text: string; language: string }) => {
    abort?.abort()
    const current = new AbortController()
    abort = current
    const s = getSettings()
    try {
      const text = await run({
        apiKey: getApiKey(),
        model: s.model,
        text: req.text,
        request: { action: 'read', language: req.language },
        customActions: [],
        translateFallback: req.language === s.translateTarget ? s.readerLanguage : s.translateTarget,
        signal: current.signal,
        onDelta: (t) => send('reader:event', { type: 'delta', text: t } satisfies RunEvent)
      })
      if (!current.signal.aborted) send('reader:event', { type: 'done', text } satisfies RunEvent)
    } catch (err) {
      if (current.signal.aborted) return
      const e = err instanceof WinkitError ? err : new WinkitError(String(err))
      send('reader:event', { type: 'error', message: e.message, openSettings: e.openSettings } satisfies RunEvent)
    }
  })
  ipcMain.on('reader:copy', (_e, text: string) => void clipboard.writeText(text))
  ipcMain.on('reader:close', () => win?.close())
}
