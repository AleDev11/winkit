import { G } from '../../components/Icon'
import { calculate, formatNumber } from '../../shared/calc'
import { matchCommands } from '../../shared/commands'
import { looksLikeUrl, matchApps } from '../../shared/search'
import type { AppEntry } from '../../shared/types'
import { CATEGORY_LABELS, convertUnits, formatAmount, formatShort } from '../../shared/units'

export type Group = 'calc' | 'units' | 'url' | 'system' | 'apps' | 'web' | 'claude'

export type RowAction =
  | { kind: 'copy'; text: string }
  | { kind: 'system'; id: string; confirm: boolean }
  | { kind: 'app'; id: string }
  | { kind: 'url'; url: string }
  | { kind: 'web'; query: string }
  | { kind: 'ask'; question: string }

export interface Row {
  key: string
  group: Group
  /** Glifo de Segoe Fluent Icons; las apps usan su propio icono (`appId`). */
  glyph: string
  appId?: string
  title: string
  subtitle: string
  /** Lo que hace Enter, para la pista de la derecha. */
  verb: string
  action: RowAction
}

// Una pregunta de verdad va antes a Claude; una palabra suelta, antes a la web.
const QUESTION =
  /^(qué|que|cómo|como|cuál|cual|cuáles|cuales|cuánto|cuanto|cuánta|cuanta|cuántos|cuantos|por qué|porque|dónde|donde|quién|quien|cuándo|cuando|explica|explícame|resume|dime|escribe|what|how|why|when|where|who|which|explain|write|summarize|tell me)\b/i

export function isQuestion(q: string): boolean {
  return /\?\s*$/.test(q) || QUESTION.test(q) || q.split(/\s+/).length >= 6
}

export function buildRows(query: string, apps: AppEntry[], engineLabel: string, modelLabel: string): Row[] {
  const q = query.trim()
  if (!q) return []
  const rows: Row[] = []

  const result = calculate(q)
  if (result !== null) {
    rows.push({
      key: 'calc',
      group: 'calc',
      glyph: G.calculator,
      title: `= ${formatNumber(result)}`,
      subtitle: 'Calculadora',
      verb: 'Copiar',
      action: { kind: 'copy', text: formatNumber(result, false) }
    })
  }

  const conv = result === null ? convertUnits(q) : null
  if (conv) {
    const out = `${formatShort(conv.result)} ${conv.to.symbol}`
    rows.push({
      key: 'units',
      group: 'units',
      glyph: G.ruler,
      title: out,
      subtitle: `${formatShort(conv.value)} ${conv.from.name} = ${formatShort(conv.result)} ${conv.to.name} · ${CATEGORY_LABELS[conv.from.category]}`,
      verb: 'Copiar',
      action: { kind: 'copy', text: formatAmount(conv.result, false) }
    })
  }

  if (looksLikeUrl(q)) {
    const url = /^https?:\/\//i.test(q) ? q : `https://${q}`
    rows.push({
      key: 'url',
      group: 'url',
      glyph: G.globe,
      title: q,
      subtitle: 'Abrir en el navegador',
      verb: 'Abrir',
      action: { kind: 'url', url }
    })
  }

  // Con una cuenta o una conversión, lo demás sobra.
  if (result === null && !conv) {
    for (const c of matchCommands(q)) {
      rows.push({
        key: `cmd:${c.id}`,
        group: 'system',
        glyph: c.glyph,
        title: c.name,
        subtitle: 'Comando del sistema',
        verb: c.confirm ? 'Ejecutar' : 'Abrir',
        action: { kind: 'system', id: c.id, confirm: !!c.confirm }
      })
    }
    for (const a of matchApps(apps, q, 6)) {
      rows.push({
        key: `app:${a.id}`,
        group: 'apps',
        glyph: G.app,
        appId: a.id,
        title: a.name,
        subtitle: 'Aplicación',
        verb: 'Abrir',
        action: { kind: 'app', id: a.id }
      })
    }
  }

  const web: Row = {
    key: 'web',
    group: 'web',
    glyph: G.search,
    title: `Buscar «${q}» en ${engineLabel}`,
    subtitle: 'Búsqueda web',
    verb: 'Buscar',
    action: { kind: 'web', query: q }
  }
  const claude: Row = {
    key: 'claude',
    group: 'claude',
    glyph: G.sparkle,
    title: 'Preguntar a Claude',
    subtitle: `«${q}» · ${modelLabel}`,
    verb: 'Preguntar',
    action: { kind: 'ask', question: q }
  }
  rows.push(...(isQuestion(q) ? [claude, web] : [web, claude]))
  return rows
}
