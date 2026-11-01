import { useEffect, useState } from 'react'

const MODIFIER_KEYS = new Set(['Control', 'Alt', 'Shift', 'Meta', 'AltGraph'])

/** KeyboardEvent → acelerador de Electron ("Control+Alt+C"). Null si aún no es una combinación válida. */
function toAccelerator(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null
  const mods: string[] = []
  if (e.ctrlKey) mods.push('Control')
  if (e.altKey) mods.push('Alt')
  if (e.shiftKey) mods.push('Shift')
  if (e.metaKey) mods.push('Super')
  // Sin Ctrl/Alt/Win, el atajo se comería teclas normales mientras escribes.
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null

  const c = e.code
  let key: string | null = null
  if (/^Key[A-Z]$/.test(c)) key = c.slice(3)
  else if (/^Digit\d$/.test(c)) key = c.slice(5)
  else if (/^F\d{1,2}$/.test(c)) key = c
  else if (c === 'Space') key = 'Space'
  else if (c.startsWith('Arrow')) key = c.slice(5)
  else if (['Enter', 'Tab', 'Backspace', 'Delete', 'Insert', 'Home', 'End', 'PageUp', 'PageDown'].includes(c)) key = c
  else if (c === 'Period') key = '.'
  else if (c === 'Comma') key = ','
  else if (c === 'Minus') key = '-'
  else if (c === 'Slash') key = '/'
  if (!key) return null
  return [...mods, key].join('+')
}

export function prettyAccelerator(accel: string): string {
  return accel
    .split('+')
    .map((k) => ({ Control: 'Ctrl', Super: 'Win', Space: 'Espacio' })[k] ?? k)
    .join(' + ')
}

export function ShortcutInput({ value, onChange }: { value: string; onChange: (accel: string) => void }): React.JSX.Element {
  const [recording, setRecording] = useState(false)

  useEffect(() => {
    if (!recording) return
    window.winkit.settings.suspendShortcuts()
    const onKey = (e: KeyboardEvent): void => {
      e.preventDefault()
      if (e.key === 'Escape') return setRecording(false)
      const accel = toAccelerator(e)
      if (accel) {
        onChange(accel)
        setRecording(false)
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      window.winkit.settings.resumeShortcuts()
    }
  }, [recording, onChange])

  return (
    <button className={recording ? 'shortcut recording' : 'shortcut'} onClick={() => setRecording((r) => !r)} onBlur={() => setRecording(false)}>
      {recording ? 'Pulsa la combinación…' : prettyAccelerator(value)}
    </button>
  )
}
