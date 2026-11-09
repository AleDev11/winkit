import { fold } from './search'

/**
 * Comandos del sistema para el buscador. El `id` es lo que se le pasa a
 * `systemCommand` en Rust (src-tauri/src/win/system.rs).
 */
export interface SystemCommand {
  id: string
  name: string
  /** Otras palabras con las que encontrarlo. */
  aliases: string[]
  /** Glifo de Segoe Fluent Icons (con Segoe MDL2 Assets de respaldo). */
  glyph: string
  /** Hay que pulsar Enter dos veces: apagar, reiniciar, vaciar la papelera… */
  confirm?: boolean
}

export const SYSTEM_COMMANDS: SystemCommand[] = [
  { id: 'lock', name: 'Bloquear el equipo', aliases: ['bloquear', 'lock'], glyph: '' },
  { id: 'sleep', name: 'Suspender', aliases: ['suspender', 'dormir', 'sleep'], glyph: '' },
  { id: 'shutdown', name: 'Apagar', aliases: ['apagar', 'shutdown'], glyph: '', confirm: true },
  { id: 'restart', name: 'Reiniciar', aliases: ['reiniciar', 'restart', 'reboot'], glyph: '', confirm: true },
  { id: 'signout', name: 'Cerrar sesión', aliases: ['cerrar sesion', 'logout', 'sign out'], glyph: '', confirm: true },
  { id: 'emptyRecycleBin', name: 'Vaciar la papelera', aliases: ['vaciar papelera', 'empty trash'], glyph: '', confirm: true },
  { id: 'recycleBin', name: 'Abrir la papelera', aliases: ['papelera', 'trash', 'recycle'], glyph: '' },
  { id: 'colorPicker', name: 'Selector de color', aliases: ['color', 'cuentagotas', 'picker', 'pipeta'], glyph: '' },
  { id: 'understand', name: 'Lector: entender un texto', aliases: ['lector', 'entender', 'traducir texto', 'reader'], glyph: '' },
  { id: 'folder:downloads', name: 'Descargas', aliases: ['descargas', 'downloads'], glyph: '' },
  { id: 'folder:documents', name: 'Documentos', aliases: ['documentos', 'documents'], glyph: '' },
  { id: 'folder:desktop', name: 'Escritorio', aliases: ['escritorio', 'desktop'], glyph: '' },
  { id: 'folder:pictures', name: 'Imágenes', aliases: ['imagenes', 'fotos', 'pictures'], glyph: '' },
  { id: 'folder:appdata', name: 'AppData', aliases: ['appdata', '%appdata%'], glyph: '' },
  { id: 'folder:temp', name: 'Archivos temporales', aliases: ['temp', 'temporales', '%temp%'], glyph: '' },
  { id: 'settings', name: 'Configuración de Windows', aliases: ['configuracion', 'ajustes de windows', 'settings'], glyph: '' },
  { id: 'taskmgr', name: 'Administrador de tareas', aliases: ['administrador de tareas', 'task manager', 'taskmgr'], glyph: '' },
  { id: 'controlPanel', name: 'Panel de control', aliases: ['panel de control', 'control panel'], glyph: '' },
  { id: 'winkitSettings', name: 'Ajustes de Winkit', aliases: ['winkit', 'ajustes', 'preferencias'], glyph: '' }
]

/** Comandos que encajan con la búsqueda, mejores primero. Requiere al menos 2 letras. */
export function matchCommands(query: string, limit = 4): SystemCommand[] {
  const q = fold(query.trim())
  if (q.length < 2) return []
  const scored = SYSTEM_COMMANDS.map((c) => {
    const words = [c.name, ...c.aliases].map(fold)
    let s = 0
    for (const w of words) {
      if (w === q) s = Math.max(s, 100)
      else if (w.startsWith(q)) s = Math.max(s, 60)
      else if (w.split(/\s+/).some((p) => p.startsWith(q))) s = Math.max(s, 40)
    }
    return { c, s }
  })
  return scored
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .slice(0, limit)
    .map((x) => x.c)
}
