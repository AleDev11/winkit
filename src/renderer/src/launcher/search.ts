import type { AppEntry } from '@shared/types'

/** Minúsculas y sin tildes: «Configuración» se encuentra escribiendo «configuracion». */
export function fold(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
}

/** Puntuación de una app para la búsqueda; 0 = no encaja. */
export function score(name: string, query: string): number {
  const n = fold(name)
  const q = fold(query.trim())
  if (!q) return 0
  if (n === q) return 1000
  if (n.startsWith(q)) return 500 - n.length
  const words = n.split(/[\s\-_.()]+/).filter(Boolean)
  if (words.some((w) => w.startsWith(q))) return 300 - n.length
  if (n.includes(q)) return 150 - n.length
  const initials = words.map((w) => w[0]).join('')
  if (q.length >= 2 && initials.startsWith(q)) return 120
  // Letras en orden, con huecos ("vsc" → Visual Studio Code).
  let i = 0
  for (const c of n) if (c === q[i]) i++
  return i === q.length && q.length >= 3 ? 30 : 0
}

export function matchApps(apps: AppEntry[], query: string, limit = 5): AppEntry[] {
  return apps
    .map((a) => ({ a, s: score(a.name, query) }))
    .filter((x) => x.s > 0)
    .sort((x, y) => y.s - x.s || x.a.name.localeCompare(y.a.name, 'es'))
    .slice(0, limit)
    .map((x) => x.a)
}

export function looksLikeUrl(query: string): boolean {
  const q = query.trim()
  return !/\s/.test(q) && /^(https?:\/\/)?(localhost|[\w-]+(\.[\w-]+)*\.[a-z]{2,})(:\d+)?(\/\S*)?$/i.test(q)
}
