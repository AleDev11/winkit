import { useEffect, useRef, useState } from 'react'
import { voice } from '../lib/api'
import { G, Icon } from './Icon'
import { ProgressRing } from './ProgressRing'

/**
 * Botón «Leer» que pasa a «Detener» mientras suena lo que ha pedido él (no lo
 * que pidan otras ventanas). Si la ventana se oculta o pierde el foco, se calla.
 */
export function SpeakButton(props: {
  text: string
  disabled?: boolean
  /** Clases del botón (por defecto "btn sm"). */
  className?: string
  /** Sin texto, solo el icono. */
  iconOnly?: boolean
}): React.JSX.Element {
  // La lectura en curso es nuestra. `pending`: pedida, aún sin noticias del backend
  // (el «idle» de cortar la anterior no debe apagar el botón).
  const [mine, setMine] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const pending = useRef(false)
  const mineRef = useRef(false)
  mineRef.current = mine

  useEffect(
    () =>
      voice.onStatus((s) => {
        if (s.state === 'loading' || s.state === 'speaking') {
          if (pending.current) setMine(true)
          setLoading(s.state === 'loading')
        } else if (!pending.current) {
          setMine(false)
          setLoading(false)
        }
      }),
    []
  )

  // Se calla al ocultar la ventana, al perder el foco o al desmontar.
  useEffect(() => {
    const quiet = (): void => {
      if (mineRef.current || pending.current) {
        pending.current = false
        setMine(false)
        void voice.stop()
      }
    }
    const onVisibility = (): void => {
      if (document.hidden) quiet()
    }
    window.addEventListener('blur', quiet)
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      window.removeEventListener('blur', quiet)
      document.removeEventListener('visibilitychange', onVisibility)
      quiet()
    }
  }, [])

  useEffect(() => {
    if (!error) return
    const t = window.setTimeout(() => setError(null), 5000)
    return () => window.clearTimeout(t)
  }, [error])

  const active = mine || pending.current
  const toggle = (): void => {
    if (active) {
      pending.current = false
      setMine(false)
      setLoading(false)
      void voice.stop()
      return
    }
    setError(null)
    pending.current = true
    setMine(true)
    setLoading(true)
    voice
      .speak(props.text)
      .then(() => {
        pending.current = false
        setLoading(false)
      })
      .catch((err: unknown) => {
        pending.current = false
        setMine(false)
        setLoading(false)
        setError(String(err))
      })
  }

  const label = active ? 'Detener' : 'Leer'
  const title = error ?? (active ? 'Detener la lectura' : 'Leer en voz alta')
  return (
    <button
      className={props.className ?? 'btn sm'}
      disabled={!active && (props.disabled || !props.text.trim())}
      onClick={toggle}
      title={title}
      aria-label={active ? 'Detener la lectura' : 'Leer en voz alta'}
      aria-pressed={active}
    >
      {active && loading ? <ProgressRing size={12} /> : <Icon glyph={error ? G.warning : active ? G.stop : G.volume} />}
      {!props.iconOnly && <span className="btn-label">{label}</span>}
    </button>
  )
}
