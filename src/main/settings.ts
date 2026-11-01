import { app, safeStorage } from 'electron'
import { createDecipheriv } from 'crypto'
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'fs'
import { dirname, join } from 'path'
import { DEFAULT_SETTINGS, type Settings } from '../shared/types'
import { dpapiUnprotect } from './native'

/**
 * Ajustes en userData/settings.json. La API key va aparte, cifrada con
 * safeStorage (DPAPI en Windows), y nunca sale del proceso principal.
 */
const file = (): string => join(app.getPath('userData'), 'settings.json')
const keyFile = (): string => join(app.getPath('userData'), 'anthropic.key')

let cache: Settings | null = null

export function getSettings(): Settings {
  if (cache) return cache
  let stored: Partial<Settings> = {}
  try {
    if (existsSync(file())) stored = JSON.parse(readFileSync(file(), 'utf8'))
  } catch {
    // Un archivo corrupto no debe impedir que la app arranque: se vuelve a los valores por defecto.
  }
  cache = {
    ...DEFAULT_SETTINGS,
    ...stored,
    shortcuts: { ...DEFAULT_SETTINGS.shortcuts, ...stored.shortcuts }
  }
  return cache
}

export function saveSettings(patch: Partial<Settings>): Settings {
  const next = { ...getSettings(), ...patch }
  mkdirSync(dirname(file()), { recursive: true })
  writeFileSync(file(), JSON.stringify(next, null, 2))
  cache = next
  return next
}

export function getApiKey(): string | null {
  if (process.env['ANTHROPIC_API_KEY']) return process.env['ANTHROPIC_API_KEY']
  try {
    if (!existsSync(keyFile())) return null
    return safeStorage.decryptString(readFileSync(keyFile()))
  } catch {
    return null
  }
}

export function setApiKey(key: string): void {
  mkdirSync(dirname(keyFile()), { recursive: true })
  writeFileSync(keyFile(), safeStorage.encryptString(key.trim()))
}

export function hasApiKey(): boolean {
  return !!getApiKey()
}

/**
 * La app se llamó Tilde y guardaba todo en %APPDATA%\tilde. Al primer arranque
 * como Winkit se copian los ajustes y se recupera la key: safeStorage la cifró
 * con una clave AES guardada (protegida con DPAPI) en el "Local State" de Tilde.
 */
export function migrateFromTilde(): void {
  const legacy = join(app.getPath('appData'), 'tilde')
  if (legacy.toLowerCase() === app.getPath('userData').toLowerCase() || !existsSync(legacy)) return
  try {
    if (!existsSync(file()) && existsSync(join(legacy, 'settings.json'))) {
      mkdirSync(dirname(file()), { recursive: true })
      copyFileSync(join(legacy, 'settings.json'), file())
      cache = null
    }
    if (existsSync(keyFile()) || !existsSync(join(legacy, 'anthropic.key'))) return
    const state = JSON.parse(readFileSync(join(legacy, 'Local State'), 'utf8'))
    const aesKey = dpapiUnprotect(Buffer.from(state.os_crypt.encrypted_key, 'base64').subarray('DPAPI'.length))
    if (!aesKey) return
    // Formato de safeStorage en Windows: "v10" + nonce (12) + cifrado + etiqueta GCM (16).
    const blob = readFileSync(join(legacy, 'anthropic.key'))
    const decipher = createDecipheriv('aes-256-gcm', aesKey, blob.subarray(3, 15))
    decipher.setAuthTag(blob.subarray(blob.length - 16))
    const key = Buffer.concat([decipher.update(blob.subarray(15, blob.length - 16)), decipher.final()]).toString('utf8')
    if (key) setApiKey(key)
  } catch {
    // Si no se puede recuperar, se pide la key de nuevo en Ajustes.
  }
}
