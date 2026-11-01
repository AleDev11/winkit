import { contextBridge, ipcRenderer } from 'electron'
import type {
  KeyTestResult,
  LauncherInit,
  PopupInit,
  ReaderInit,
  RunEvent,
  RunRequest,
  Settings,
  ShortcutStatus
} from '../shared/types'

function on<T>(channel: string, cb: (payload: T) => void): () => void {
  const listener = (_e: unknown, payload: T): void => cb(payload)
  ipcRenderer.on(channel, listener)
  return () => ipcRenderer.removeListener(channel, listener)
}

const api = {
  popup: {
    onInit: (cb: (init: PopupInit) => void) => on('popup:init', cb),
    onRunEvent: (cb: (ev: RunEvent) => void) => on('run:event', cb),
    size: (width: number, height: number) => ipcRenderer.send('popup:size', { width, height }),
    run: (req: RunRequest) => ipcRenderer.send('popup:run', req),
    replace: (text: string) => ipcRenderer.send('popup:replace', text),
    copy: (text: string) => ipcRenderer.send('popup:copy', text),
    close: () => ipcRenderer.send('popup:close'),
    openSettings: () => ipcRenderer.send('popup:settings')
  },
  reader: {
    onInit: (cb: (init: ReaderInit) => void) => on('reader:init', cb),
    onEvent: (cb: (ev: RunEvent) => void) => on('reader:event', cb),
    translate: (text: string, language: string) => ipcRenderer.send('reader:translate', { text, language }),
    copy: (text: string) => ipcRenderer.send('reader:copy', text),
    close: () => ipcRenderer.send('reader:close')
  },
  launcher: {
    onInit: (cb: (init: LauncherInit) => void) => on('launcher:init', cb),
    onEvent: (cb: (ev: RunEvent) => void) => on('launcher:event', cb),
    size: (width: number, height: number) => ipcRenderer.send('launcher:size', { width, height }),
    icon: (id: string): Promise<string | null> => ipcRenderer.invoke('launcher:icon', id),
    openApp: (id: string) => ipcRenderer.send('launcher:open-app', id),
    web: (query: string) => ipcRenderer.send('launcher:web', query),
    url: (url: string) => ipcRenderer.send('launcher:url', url),
    copy: (text: string) => ipcRenderer.send('launcher:copy', text),
    ask: (question: string) => ipcRenderer.send('launcher:ask', question),
    close: () => ipcRenderer.send('launcher:close')
  },
  settings: {
    get: (): Promise<{ settings: Settings; hasKey: boolean; shortcuts: ShortcutStatus[]; version: string }> =>
      ipcRenderer.invoke('settings:get'),
    save: (patch: Partial<Settings>): Promise<{ settings: Settings; shortcuts: ShortcutStatus[] }> =>
      ipcRenderer.invoke('settings:save', patch),
    saveKey: (key: string): Promise<KeyTestResult> => ipcRenderer.invoke('key:save', key),
    testKey: (): Promise<KeyTestResult> => ipcRenderer.invoke('key:test'),
    suspendShortcuts: () => ipcRenderer.send('shortcuts:suspend'),
    resumeShortcuts: () => ipcRenderer.send('shortcuts:resume')
  }
}

export type WinkitApi = typeof api

contextBridge.exposeInMainWorld('winkit', api)
