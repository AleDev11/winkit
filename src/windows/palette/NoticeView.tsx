import { useCallback, useEffect, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { ProgressRing } from '../../components/ProgressRing'
import { palette } from '../../lib/api'
import type { Notice, NoticeKind, PaletteInit } from '../../shared/types'

/**
 * Aviso de la esquina: icono según el tipo, título, detalle, botones y una
 * barrita con lo que le queda. Entra deslizándose desde su esquina; si el
 * anterior era del mismo grupo («Guardando clip…» → «Clip guardado»), se
 * transforma en el sitio. Pasar el ratón por encima pausa la cuenta.
 */

const GLYPH: Record<Exclude<NoticeKind, 'progress' | 'color'>, string> = {
  info: G.info,
  success: G.check,
  warning: G.warning,
  error: G.warning
}

/** Grupo del aviso anterior (la ventana se reutiliza entre avisos). */
let lastGroup: string | null = null
let lastShownAt = 0

const EMPTY: Notice = {
  kind: 'info',
  title: '',
  detail: null,
  color: null,
  actions: [],
  durationMs: 4000,
  placement: 'bottomRight',
  group: null
}

export function NoticeView({ init }: { init: PaletteInit }): React.JSX.Element {
  const n = init.notice ?? EMPTY

  // ¿Entra (desde su esquina) o se transforma (mismo grupo que el anterior, hace poco)?
  const [morph] = useState(() => {
    const same = !!n.group && n.group === lastGroup && Date.now() - lastShownAt < 15_000
    lastGroup = n.group
    lastShownAt = Date.now()
    return same
  })

  const from = n.placement.startsWith('top') ? 'top' : n.placement === 'cursor' ? 'cursor' : 'bottom'

  // Cuenta atrás con pausa: lo que queda, y desde cuándo corre.
  const total = n.durationMs
  const [paused, setPaused] = useState(false)
  const left = useRef(total)
  const since = useRef(performance.now())
  const [leaving, setLeaving] = useState(false)

  const dismiss = useCallback(() => {
    setLeaving(true)
    window.setTimeout(() => void palette.close(), 140)
  }, [])

  useEffect(() => {
    if (!total || paused) return
    since.current = performance.now()
    const t = window.setTimeout(dismiss, left.current)
    return () => {
      window.clearTimeout(t)
      left.current = Math.max(0, left.current - (performance.now() - since.current))
    }
  }, [total, paused, dismiss])

  useEffect(() => {
    const on = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') dismiss()
    }
    window.addEventListener('keydown', on)
    return () => window.removeEventListener('keydown', on)
  }, [dismiss])

  const act = (action: string, arg: string | null) => (e: React.MouseEvent) => {
    e.stopPropagation()
    void palette.noticeAction(action, arg)
  }

  const classes = ['notice', `notice-${n.kind}`, morph ? 'notice-morph' : `notice-enter-${from}`, leaving ? 'notice-leaving' : '']
  return (
    <div
      className={classes.join(' ')}
      role={n.kind === 'error' || n.kind === 'warning' ? 'alert' : 'status'}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <div className="notice-row">
        {n.kind === 'color' && n.color ? (
          <span className="notice-swatch" style={{ background: n.color }} aria-hidden="true" />
        ) : (
          <span className="notice-badge" aria-hidden="true">
            {n.kind === 'progress' ? <ProgressRing size={16} /> : <Icon glyph={GLYPH[n.kind as keyof typeof GLYPH]} />}
          </span>
        )}
        <div className="notice-text">
          <span className="notice-title">{n.title}</span>
          {n.detail && <span className="notice-detail">{n.detail}</span>}
        </div>
        <button className="btn subtle sm icon-only notice-close" aria-label="Cerrar" title="Cerrar" onClick={dismiss}>
          <Icon glyph={G.close} size={10} />
        </button>
      </div>
      {n.actions.length > 0 && (
        <div className="notice-actions">
          {n.actions.map((a) => (
            <button key={a.label} className={a.primary ? 'btn accent sm' : 'btn sm'} onClick={act(a.action, a.arg)}>
              {a.label}
            </button>
          ))}
        </div>
      )}
      {n.kind === 'progress' ? (
        <span className="notice-bar indeterminate" aria-hidden="true" />
      ) : total > 0 ? (
        <span
          className="notice-bar"
          aria-hidden="true"
          style={{ animationDuration: `${total}ms`, animationPlayState: paused ? 'paused' : 'running' }}
        />
      ) : null}
    </div>
  )
}
