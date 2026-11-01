import { BrowserWindow, clipboard, ipcMain, screen, shell } from 'electron'
import { join } from 'path'
import { MODELS, SEARCH_ENGINES, type LauncherInit, type RunEvent } from '../shared/types'
import { appIcon, launchApp, listApps } from './apps'
import { ask, WinkitError } from './claude'
import { getApiKey, getSettings } from './settings'

/**
 * El buscador (Alt+Espacio): una barra flotante centrada, como Spotlight. Igual
 * que la viñeta, es una ventana transparente que se ajusta a lo que mide el renderer.
 */
const PAD = 24 // margen transparente para la sombra
const WIDTH = 680

let win: BrowserWindow | null = null
let abort: AbortController | null = null
let top = { x: 0, y: 0 }

export function createLauncher(): void {
  win = new BrowserWindow({
    width: WIDTH + PAD * 2,
    height: 120,
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
  if (process.env['ELECTRON_RENDERER_URL']) win.loadURL(`${process.env['ELECTRON_RENDERER_URL']}/launcher.html`)
  else win.loadFile(join(__dirname, '../renderer/launcher.html'))
  win.on('blur', hide)

  ipcMain.on('launcher:size', (_e, size: { width: number; height: number }) => {
    win?.setBounds({ x: top.x, y: top.y, width: Math.ceil(size.width) + PAD * 2, height: Math.ceil(size.height) + PAD * 2 })
  })
  ipcMain.handle('launcher:icon', (_e, id: string) => appIcon(id))
  ipcMain.on('launcher:open-app', (_e, id: string) => {
    hide()
    launchApp(id)
  })
  ipcMain.on('launcher:web', (_e, query: string) => {
    hide()
    void shell.openExternal(SEARCH_ENGINES[getSettings().searchEngine].url + encodeURIComponent(query))
  })
  ipcMain.on('launcher:url', (_e, url: string) => {
    hide()
    void shell.openExternal(/^https?:\/\//i.test(url) ? url : `https://${url}`)
  })
  ipcMain.on('launcher:copy', (_e, text: string) => {
    void clipboard.writeText(text)
    hide()
  })
  ipcMain.on('launcher:ask', (_e, req: { question: string; id: number }) => void askClaude(req.question, req.id))
  ipcMain.on('launcher:close', hide)
}

function send(channel: string, payload: unknown): void {
  win?.webContents.send(channel, payload)
}

function hide(): void {
  abort?.abort()
  abort = null
  win?.hide()
}

export async function toggleLauncher(): Promise<void> {
  if (!win) return
  if (win.isVisible()) return hide()
  const { workArea: wa } = screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
  top = { x: Math.round(wa.x + (wa.width - WIDTH) / 2 - PAD), y: Math.round(wa.y + wa.height * 0.2 - PAD) }
  const s = getSettings()
  const init: LauncherInit = {
    apps: await listApps(),
    engineLabel: SEARCH_ENGINES[s.searchEngine].label,
    modelLabel: MODELS.find((m) => m.id === s.model)?.label ?? s.model
  }
  send('launcher:init', init)
  win.setPosition(top.x, top.y)
  win.show()
  win.focus()
}

async function askClaude(question: string, id: number): Promise<void> {
  abort?.abort()
  const current = new AbortController()
  abort = current
  const s = getSettings()
  try {
    const text = await ask({
      apiKey: getApiKey(),
      model: s.model,
      question,
      signal: current.signal,
      onDelta: (t) => send('launcher:event', { type: 'delta', text: t, id } satisfies RunEvent)
    })
    if (!current.signal.aborted) send('launcher:event', { type: 'done', text, id } satisfies RunEvent)
  } catch (err) {
    if (current.signal.aborted) return
    const e = err instanceof WinkitError ? err : new WinkitError(String(err))
    send('launcher:event', { type: 'error', message: e.message, openSettings: e.openSettings, id } satisfies RunEvent)
  }
}
