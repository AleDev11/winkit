/**
 * Aceleradores con el formato de Tauri ("Control+Alt+X"): modificadores
 * Control, Alt, Shift y Super, y una tecla.
 */

export const MODIFIERS = ['Control', 'Alt', 'Shift', 'Super'] as const

const NAMED_CODES: Record<string, string> = {
  Space: 'Space',
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Period: '.',
  Comma: ',',
  Minus: '-',
  Equal: '=',
  Slash: '/',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  BracketLeft: '[',
  BracketRight: ']',
  Backquote: '`',
  PrintScreen: 'PrintScreen',
  Pause: 'Pause'
}

/** Tecla principal a partir de `KeyboardEvent.code` (independiente de la distribución del teclado). */
export function keyFromCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3)
  if (/^Digit\d$/.test(code)) return code.slice(5)
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code
  return NAMED_CODES[code] ?? null
}

/** Modificadores pulsados, en el orden canónico. */
export function heldModifiers(e: KeyboardEvent): string[] {
  const mods: string[] = []
  if (e.ctrlKey) mods.push('Control')
  if (e.altKey) mods.push('Alt')
  if (e.shiftKey) mods.push('Shift')
  if (e.metaKey) mods.push('Super')
  return mods
}

/**
 * KeyboardEvent → acelerador. Null si aún no es una combinación válida:
 * hace falta Ctrl, Alt o Win, o el atajo se comería teclas normales al escribir.
 */
export function toAccelerator(e: KeyboardEvent): string | null {
  const key = keyFromCode(e.code)
  if (!key) return null
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null
  return [...heldModifiers(e), key].join('+')
}

const PRETTY: Record<string, string> = {
  Control: 'Ctrl',
  Super: 'Win',
  Shift: 'Mayús',
  Space: 'Espacio',
  Enter: 'Enter',
  Backspace: 'Retroceso',
  Delete: 'Supr',
  Insert: 'Insert',
  Home: 'Inicio',
  End: 'Fin',
  PageUp: 'RePág',
  PageDown: 'AvPág',
  Up: '↑',
  Down: '↓',
  Left: '←',
  Right: '→',
  Escape: 'Esc'
}

/** Partes del acelerador para pintarlas como teclas. */
export function acceleratorParts(accel: string): string[] {
  if (!accel) return []
  // "Control++" no existe en Tauri; se separa por "+" sin más.
  return accel.split('+').filter(Boolean).map((k) => PRETTY[k] ?? k)
}

/** "Control+Alt+X" → "Ctrl + Alt + X". */
export function prettyAccelerator(accel: string): string {
  return acceleratorParts(accel).join(' + ')
}
