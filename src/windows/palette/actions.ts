import { G } from '../../components/Icon'
import { MODELS, type BuiltinAction, type CustomAction, type RunRequest } from '../../shared/types'

export interface BuiltinInfo {
  label: string
  glyph: string
  /** Otras palabras con las que encontrarla al filtrar. */
  keywords: string
}

export const BUILTINS: Record<BuiltinAction, BuiltinInfo> = {
  correct: { label: 'Corregir', glyph: G.spelling, keywords: 'ortografia gramatica faltas erratas fix correct' },
  translate: { label: 'Traducir', glyph: G.translate, keywords: 'idioma translate traduccion' },
  understand: { label: 'Entender', glyph: G.book, keywords: 'leer lector explicar understand read' },
  improve: { label: 'Mejorar', glyph: G.sparkle, keywords: 'redaccion claro reescribir improve rewrite' },
  formal: { label: 'Más formal', glyph: G.briefcase, keywords: 'profesional serio formal' },
  casual: { label: 'Más cercano', glyph: G.smile, keywords: 'informal amable casual friendly' },
  shorter: { label: 'Más corto', glyph: G.scissors, keywords: 'resumir acortar breve shorter' }
}

/** Orden de la lista del menú (y de las teclas 1–7). */
export const MENU_ORDER: BuiltinAction[] = ['correct', 'translate', 'understand', 'improve', 'formal', 'casual', 'shorter']

export function isBuiltin(action: string): action is BuiltinAction {
  return action in BUILTINS
}

export function actionTitle(req: RunRequest, custom: CustomAction[]): string {
  if (isBuiltin(req.action)) return BUILTINS[req.action].label
  if (req.action === 'instruction') return 'Instrucción'
  return custom.find((a) => a.id === req.action)?.name || 'Acción personalizada'
}

export function actionGlyph(req: RunRequest): string {
  if (isBuiltin(req.action)) return BUILTINS[req.action].glyph
  if (req.action === 'instruction') return G.message
  return G.lightning
}

/** El evento `start` trae el id del modelo; se enseña con su nombre. */
export function modelLabel(model: string | null | undefined, fallback: string): string {
  if (!model) return fallback
  return MODELS.find((m) => m.id === model)?.label ?? model
}

/** Texto en una sola línea para vistas previas. */
export function oneLine(text: string): string {
  return text.replace(/\s+/g, ' ').trim()
}
