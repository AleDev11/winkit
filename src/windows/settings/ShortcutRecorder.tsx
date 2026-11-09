import { useEffect, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { acceleratorParts, heldModifiers, toAccelerator } from './accelerator'

/**
 * Botón que muestra un atajo como teclas y, al pulsarlo, graba uno nuevo.
 * Mientras graba, los atajos globales están suspendidos (`onRecordingChange`).
 */
export function ShortcutRecorder(props: {
  value: string
  label: string
  invalid?: boolean
  onChange: (accel: string) => void
  onRecordingChange: (recording: boolean) => void
}): React.JSX.Element {
  const { value, label, invalid, onChange, onRecordingChange } = props
  const [recording, setRecording] = useState(false)
  const [held, setHeld] = useState<string[]>([])
  const [hint, setHint] = useState<string | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const latest = useRef({ onChange, onRecordingChange })
  latest.current = { onChange, onRecordingChange }

  useEffect(() => {
    if (!recording) return
    latest.current.onRecordingChange(true)
    setHeld([])
    setHint(null)

    const stop = (): void => setRecording(false)
    const onKeyDown = (e: KeyboardEvent): void => {
      e.preventDefault()
      e.stopPropagation()
      if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.metaKey) return stop()
      const accel = toAccelerator(e)
      if (accel) {
        latest.current.onChange(accel)
        stop()
        return
      }
      setHeld(heldModifiers(e))
      // Una tecla suelta (sin Ctrl, Alt ni Win) no vale.
      if (!['Control', 'Alt', 'Shift', 'Meta', 'AltGraph', 'OS'].includes(e.key)) {
        setHint('Añade Ctrl, Alt o Win')
      }
    }
    const onKeyUp = (e: KeyboardEvent): void => {
      e.preventDefault()
      setHeld(heldModifiers(e))
    }
    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('keyup', onKeyUp, true)
    window.addEventListener('blur', stop)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('keyup', onKeyUp, true)
      window.removeEventListener('blur', stop)
      latest.current.onRecordingChange(false)
    }
  }, [recording])

  const parts = recording ? acceleratorParts(held.join('+')) : acceleratorParts(value)

  return (
    <div className="shortcut">
      <button
        ref={buttonRef}
        className={['shortcut-btn', recording ? 'recording' : '', invalid ? 'invalid' : '', !value && !recording ? 'unset' : ''].join(' ')}
        onClick={() => setRecording((r) => !r)}
        onBlur={() => setRecording(false)}
        aria-label={`${label}: ${value ? parts.join(' + ') : 'sin atajo'}. Pulsa para cambiarlo.`}
        title={recording ? 'Esc cancela' : 'Cambiar el atajo'}
      >
        {parts.length ? (
          parts.map((p, i) => (
            <span key={i} className="keycap">
              {p}
            </span>
          ))
        ) : recording ? (
          <span className={hint ? 'shortcut-wait warn' : 'shortcut-wait'}>
            <Icon glyph={G.record} size={10} className="rec-dot" /> {hint ?? 'Pulsa la combinación'}
          </span>
        ) : (
          <span className="shortcut-none">Desactivado</span>
        )}
        {!recording && <Icon glyph={G.edit} size={12} className="shortcut-edit" />}
      </button>
      <button
        className="btn subtle icon-only"
        disabled={!value || recording}
        onClick={() => onChange('')}
        title="Quitar el atajo"
        aria-label={`Quitar el atajo de ${label}`}
      >
        <Icon glyph={G.delete} size={14} />
      </button>
    </div>
  )
}
