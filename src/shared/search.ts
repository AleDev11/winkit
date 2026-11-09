import type { AppEntry } from './types'

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
  // Letras en orden, con huecos ("vsc" → Visual Studio Code), empezando al
  // principio de una palabra: si no, «apag» encontraría «Windows Media Player».
  if (q.length < 3) return 0
  for (let start = 0; start < n.length; start++) {
    if (n[start] !== q[0] || (start > 0 && /[a-z0-9]/.test(n[start - 1]))) continue
    let i = 0
    for (const c of n.slice(start)) if (c === q[i]) i++
    if (i === q.length) return 30
  }
  return 0
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
