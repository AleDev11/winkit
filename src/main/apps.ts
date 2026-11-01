import { app, shell } from 'electron'
import { execFile, spawn } from 'child_process'
import { readdir } from 'fs/promises'
import { basename, extname, join } from 'path'
import type { AppEntry } from '../shared/types'

/**
 * Índice de aplicaciones para el buscador: los accesos directos del menú Inicio
 * (los de todos los usuarios y los tuyos) más las apps de la Tienda, que no
 * tienen acceso directo y solo se encuentran con Get-StartApps.
 */
const SHORTCUT_EXT = new Set(['.lnk', '.url', '.appref-ms'])
const NOISE = /uninstall|desinstal|readme|léame|leame|help|ayuda|documentation|documentación|release notes|website|sitio web|license|licencia/i
const TTL = 10 * 60 * 1000

let cache: AppEntry[] = []
let builtAt = 0
let building: Promise<AppEntry[]> | null = null
const icons = new Map<string, string | null>()

function startMenus(): string[] {
  return [
    join(process.env['ProgramData'] ?? 'C:\\ProgramData', 'Microsoft', 'Windows', 'Start Menu', 'Programs'),
    join(app.getPath('appData'), 'Microsoft', 'Windows', 'Start Menu', 'Programs')
  ]
}

async function walk(dir: string, out: AppEntry[]): Promise<void> {
  let entries
  try {
    entries = await readdir(dir, { withFileTypes: true })
  } catch {
    return
  }
  for (const e of entries) {
    const full = join(dir, e.name)
    if (e.isDirectory()) await walk(full, out)
    else if (SHORTCUT_EXT.has(extname(e.name).toLowerCase())) {
      const name = basename(e.name, extname(e.name))
      if (!NOISE.test(name)) out.push({ id: full, name, kind: 'shortcut' })
    }
  }
}

function storeApps(): Promise<AppEntry[]> {
  const script = '[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-StartApps | ConvertTo-Json -Compress'
  return new Promise((resolve) => {
    execFile(
      'powershell.exe',
      ['-NoProfile', '-NonInteractive', '-Command', script],
      { windowsHide: true, timeout: 20000, maxBuffer: 8 * 1024 * 1024, encoding: 'utf8' },
      (err, stdout) => {
        if (err || !stdout.trim()) return resolve([])
        try {
          const parsed = JSON.parse(stdout) as { Name: string; AppID: string } | { Name: string; AppID: string }[]
          const list = Array.isArray(parsed) ? parsed : [parsed]
          // Las de la Tienda llevan "!" en el AppID; el resto ya sale de los accesos directos.
          resolve(list.filter((a) => a.AppID?.includes('!')).map((a) => ({ id: a.AppID, name: a.Name, kind: 'store' as const })))
        } catch {
          resolve([])
        }
      }
    )
  })
}

async function build(): Promise<AppEntry[]> {
  const shortcuts: AppEntry[] = []
  for (const dir of startMenus()) await walk(dir, shortcuts)
  const store = await storeApps()
  const seen = new Set<string>()
  const all: AppEntry[] = []
  for (const a of [...shortcuts, ...store]) {
    const key = a.name.toLocaleLowerCase('es')
    if (seen.has(key)) continue
    seen.add(key)
    all.push(a)
  }
  all.sort((a, b) => a.name.localeCompare(b.name, 'es'))
  cache = all
  builtAt = Date.now()
  return all
}

/** Devuelve el índice al instante; si está viejo, lo renueva en segundo plano. */
export async function listApps(): Promise<AppEntry[]> {
  const stale = Date.now() - builtAt > TTL
  if (stale && !building) building = build().finally(() => (building = null))
  if (!cache.length && building) return building
  return cache
}

export async function appIcon(id: string): Promise<string | null> {
  if (icons.has(id)) return icons.get(id)!
  const entry = cache.find((a) => a.id === id)
  let url: string | null = null
  if (entry?.kind === 'shortcut') {
    try {
      url = (await app.getFileIcon(id, { size: 'normal' })).toDataURL()
    } catch {
      url = null
    }
  }
  icons.set(id, url)
  return url
}

export function launchApp(id: string): void {
  const entry = cache.find((a) => a.id === id)
  if (!entry) return
  if (entry.kind === 'shortcut') void shell.openPath(entry.id)
  else spawn('explorer.exe', [`shell:AppsFolder\\${entry.id}`], { detached: true, stdio: 'ignore' }).unref()
}
