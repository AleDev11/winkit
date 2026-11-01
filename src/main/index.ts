import { app, BrowserWindow, globalShortcut, ipcMain, Menu, nativeImage, Notification, Tray } from 'electron'
import { join } from 'path'
import type { Settings, Shortcuts, ShortcutStatus } from '../shared/types'
import { listApps } from './apps'
import { testKey } from './claude'
import { createLauncher, toggleLauncher } from './launcher'
import { createPopup, trigger } from './popup'
import { openReader, registerReader } from './reader'
import { getApiKey, getSettings, hasApiKey, migrateFromTilde, saveSettings, setApiKey } from './settings'

const resource = (name: string): string => join(app.getAppPath(), 'resources', name)

let tray: Tray | null = null
let settingsWin: BrowserWindow | null = null
let paused = false
let shortcutStatus: ShortcutStatus[] = []

const ACTIONS: Record<keyof Shortcuts, () => void> = {
  menu: () => void trigger('menu'),
  correct: () => void trigger('correct'),
  translate: () => void trigger('translate'),
  reader: () => void openReader(resource('icon.png')),
  launcher: () => void toggleLauncher()
}

function registerShortcuts(): ShortcutStatus[] {
  globalShortcut.unregisterAll()
  if (paused) return (shortcutStatus = [])
  const { shortcuts } = getSettings()
  shortcutStatus = (Object.keys(ACTIONS) as (keyof Shortcuts)[]).map((key) => {
    const accel = shortcuts[key]
    let ok = false
    try {
      ok = !!accel && globalShortcut.register(accel, ACTIONS[key])
    } catch {
      ok = false
    }
    return { key, ok }
  })
  return shortcutStatus
}

function applyLoginItem(s: Settings): void {
  // En desarrollo registraría electron.exe: solo en la app instalada.
  if (app.isPackaged) app.setLoginItemSettings({ openAtLogin: s.launchAtLogin })
}

function openSettings(): void {
  if (settingsWin) {
    if (settingsWin.isMinimized()) settingsWin.restore()
    return settingsWin.focus()
  }
  settingsWin = new BrowserWindow({
    width: 780,
    height: 680,
    minWidth: 620,
    minHeight: 480,
    title: 'Winkit',
    icon: resource('icon.png'),
    backgroundColor: '#121214',
    autoHideMenuBar: true,
    show: false,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: false
    }
  })
  settingsWin.once('ready-to-show', () => settingsWin?.show())
  if (process.env['ELECTRON_RENDERER_URL']) settingsWin.loadURL(`${process.env['ELECTRON_RENDERER_URL']}/settings.html`)
  else settingsWin.loadFile(join(__dirname, '../renderer/settings.html'))
  settingsWin.on('closed', () => {
    settingsWin = null
    // Por si se cerró mientras se grababa un atajo.
    registerShortcuts()
  })
}

function buildTrayMenu(): void {
  tray?.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Buscador', click: ACTIONS.launcher },
      { label: 'Lector', click: ACTIONS.reader },
      { label: 'Ajustes…', click: openSettings },
      { type: 'separator' },
      {
        label: 'Pausar atajos',
        type: 'checkbox',
        checked: paused,
        click: (item) => {
          paused = item.checked
          registerShortcuts()
          tray?.setToolTip(paused ? 'Winkit (en pausa)' : 'Winkit')
        }
      },
      { type: 'separator' },
      { label: 'Salir', click: () => app.quit() }
    ])
  )
}

function registerIpc(): void {
  ipcMain.handle('settings:get', () => ({
    settings: getSettings(),
    hasKey: hasApiKey(),
    shortcuts: shortcutStatus,
    version: app.getVersion()
  }))
  ipcMain.handle('settings:save', (_e, patch: Partial<Settings>) => {
    const s = saveSettings(patch)
    if (patch.launchAtLogin !== undefined) applyLoginItem(s)
    return { settings: s, shortcuts: patch.shortcuts ? registerShortcuts() : shortcutStatus }
  })
  ipcMain.handle('key:save', async (_e, key: string) => {
    const result = await testKey(key.trim())
    if (result.ok) setApiKey(key)
    return result
  })
  ipcMain.handle('key:test', () => {
    const key = getApiKey()
    return key ? testKey(key) : { ok: false, message: 'No hay ninguna API key guardada.' }
  })
  // Mientras se graba un atajo nuevo, los globales no deben tragarse la combinación.
  ipcMain.on('shortcuts:suspend', () => globalShortcut.unregisterAll())
  ipcMain.on('shortcuts:resume', () => registerShortcuts())
}

if (!app.requestSingleInstanceLock()) {
  app.quit()
} else {
  app.on('second-instance', openSettings)
  // La app vive en la bandeja: cerrar ventanas no la termina.
  app.on('window-all-closed', () => {})
  app.on('will-quit', () => globalShortcut.unregisterAll())

  app.whenReady().then(() => {
    app.setAppUserModelId('com.alejandrofont.winkit')
    migrateFromTilde()
    registerIpc()
    registerReader()
    createPopup(openSettings)
    createLauncher()
    // El índice de apps tarda unos segundos (Get-StartApps): que esté listo antes del primer Alt+Espacio.
    void listApps()

    tray = new Tray(nativeImage.createFromPath(resource('tray.png')))
    tray.setToolTip('Winkit')
    tray.on('click', openSettings)
    buildTrayMenu()

    const taken = registerShortcuts().filter((st) => !st.ok)
    applyLoginItem(getSettings())
    if (!hasApiKey()) openSettings()
    else if (taken.length) {
      const names = taken.map((st) => getSettings().shortcuts[st.key].replace('Control', 'Ctrl')).join(', ')
      const n = new Notification({ title: 'Winkit', body: `Otra app ya usa ${names}. Elige otro atajo en Ajustes.`, icon: resource('icon.png') })
      n.on('click', openSettings)
      n.show()
    }
  })
}
