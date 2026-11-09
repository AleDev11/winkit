import { useEffect, useMemo, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { ProgressRing } from '../../components/ProgressRing'
import { palette, ui } from '../../lib/api'
import { hasChanges, wordDiff } from '../../shared/diff'
import type { PaletteInit, RunRequest } from '../../shared/types'
import { actionGlyph, actionTitle, modelLabel } from './actions'
import { useRun } from './useRun'

export function RunView(props: {
  init: PaletteInit
  request: RunRequest
  onBack: (() => void) | null
  /** Atajo directo con reemplazo automático: Rust pega el resultado y oculta la ventana. */
  autoReplace: boolean
}): React.JSX.Element {
  const { init, request, onBack, autoReplace } = props
  const { state, start, cancel } = useRun()
  // Las traducciones se leen mejor limpias; el resto, con los cambios marcados.
  const [showDiff, setShowDiff] = useState(request.action !== 'translate')
  const [copied, setCopied] = useState(false)
  const bodyRef = useRef<HTMLDivElement>(null)

  useEffect(() => start(request), [start, request])

  const { status, text: output } = state
  const done = status === 'done'
  const streaming = status === 'streaming'

  const diff = useMemo(() => (done ? wordDiff(request.text, output) : null), [done, request.text, output])
  const unchanged = done && (diff ? !hasChanges(diff) : output.trim() === request.text.trim())

  // Mientras llega el texto, la vista sigue al final.
  useEffect(() => {
    const el = bodyRef.current
    if (el && streaming) el.scrollTop = el.scrollHeight
  }, [output, streaming])

  const back = (): void => {
    if (!onBack) return
    cancel()
    onBack()
  }
  const replace = (): void => {
    if (done && init.canReplace) void palette.replace(output)
  }
  const copy = (): void => {
    if (!done) return
    setCopied(true)
    void palette.copyAndClose(output)
  }
  const regenerate = (): void => {
    if (streaming) return
    setCopied(false)
    start(request)
  }

  const handler = useRef<(e: KeyboardEvent) => void>(() => {})
  handler.current = (e) => {
    const k = e.key.toLowerCase()
    if (e.key === 'Escape') {
      e.preventDefault()
      void palette.close()
    } else if ((e.altKey && e.key === 'ArrowLeft') || (e.key === 'Backspace' && !streaming && !isEditable(e.target))) {
      e.preventDefault()
      back()
    } else if (e.ctrlKey && k === 'r') {
      e.preventDefault()
      regenerate()
    } else if (e.ctrlKey && k === 'c') {
      // Si hay algo seleccionado en el resultado, se copia eso como siempre.
      if (window.getSelection()?.toString()) return
      e.preventDefault()
      copy()
    } else if (e.key === 'Enter' && !e.ctrlKey && !e.altKey && !e.shiftKey) {
      if (isButton(e.target)) return
      e.preventDefault()
      if (status === 'error') regenerate()
      else if (autoReplace) return
      else if (init.canReplace) replace()
      else copy()
    }
  }
  useEffect(() => {
    const on = (e: KeyboardEvent): void => handler.current(e)
    window.addEventListener('keydown', on)
    return () => window.removeEventListener('keydown', on)
  }, [])

  const title = actionTitle(request, init.customActions)
  const fromTo = state.to ? (state.from ? `${state.from} → ${state.to}` : `→ ${state.to}`) : null

  return (
    <div className="run-view">
      <header className="run-head">
        {onBack && (
          <button className="btn subtle sm icon-only" onClick={back} title="Volver al menú (Alt+←)" aria-label="Volver al menú">
            <Icon glyph={G.back} size={12} />
          </button>
        )}
        <Icon glyph={actionGlyph(request)} className="run-head-icon" />
        <span className="run-title body-strong">{title}</span>
        {request.action === 'instruction' && request.instruction && (
          <span className="run-instruction" title={request.instruction}>
            {request.instruction}
          </span>
        )}
        {fromTo && <span className="chip">{fromTo}</span>}
        <span className="spacer" />
        {streaming && <ProgressRing size={16} />}
        <span className="caption faint">{modelLabel(state.model, init.modelLabel)}</span>
      </header>
      <div className={streaming ? 'progress-bar' : 'progress-bar idle'} aria-hidden="true" />

      {status === 'error' && state.error ? (
        <div className="run-body">
          <div className="infobar error fade-in" role="alert">
            <Icon glyph={G.error} />
            <div className="infobar-body">
              <div className="body-strong">No se ha podido completar</div>
              <div className="muted">{state.error.message}</div>
            </div>
          </div>
        </div>
      ) : (
        <div ref={bodyRef} className="run-body run-text selectable" aria-live="polite">
          {unchanged && (
            <div className="infobar success run-unchanged fade-in">
              <Icon glyph={G.checkCircle} />
              <div className="infobar-body">Sin cambios: el texto ya estaba bien.</div>
            </div>
          )}
          {streaming && !output ? (
            <span className="faint">Pensando…</span>
          ) : done && showDiff && diff && !unchanged ? (
            diff.map((s, i) =>
              s.type === 'same' ? (
                <span key={i}>{s.text}</span>
              ) : s.type === 'add' ? (
                <ins key={i}>{s.text}</ins>
              ) : (
                <del key={i}>{s.text}</del>
              )
            )
          ) : (
            <>
              {output}
              {streaming && <span className="caret" />}
            </>
          )}
        </div>
      )}

      <footer className="run-foot">
        {status === 'error' ? (
          <>
            <span className="spacer" />
            {state.error?.openSettings && (
              <button className="btn" onClick={() => void ui.openSettings()}>
                <Icon glyph={G.settings} /> Abrir ajustes
              </button>
            )}
            <button className="btn accent" onClick={regenerate}>
              <Icon glyph={G.refresh} /> Reintentar
            </button>
          </>
        ) : autoReplace ? (
          <span className="run-auto caption muted">
            {done ? (
              <>
                <Icon glyph={G.check} /> Reemplazando…
              </>
            ) : (
              'Se reemplazará en cuanto termine'
            )}
          </span>
        ) : (
          <>
            {done && diff && !unchanged ? (
              <label className="run-toggle caption">
                <input type="checkbox" className="switch" checked={showDiff} onChange={(e) => setShowDiff(e.target.checked)} />
                Ver cambios
              </label>
            ) : (
              <span className="hints">
                <span>
                  <kbd>Esc</kbd> cerrar
                </span>
              </span>
            )}
            <span className="spacer" />
            <button
              className="btn subtle icon-only"
              disabled={streaming}
              onClick={regenerate}
              title="Regenerar (Ctrl+R)"
              aria-label="Regenerar"
            >
              <Icon glyph={G.refresh} />
            </button>
            <button
              className={init.canReplace ? 'btn' : 'btn accent'}
              disabled={!done}
              onClick={copy}
              title={init.canReplace ? 'Copiar (Ctrl+C)' : 'Copiar (Enter)'}
            >
              <Icon glyph={copied ? G.check : G.copy} /> Copiar
            </button>
            {init.canReplace && (
              <button className="btn accent" disabled={!done} onClick={replace} title="Reemplazar (Enter)">
                Reemplazar <Icon glyph={G.enter} />
              </button>
            )}
          </>
        )}
      </footer>
    </div>
  )
}

function isEditable(t: EventTarget | null): boolean {
  return t instanceof HTMLElement && (t.isContentEditable || t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')
}
function isButton(t: EventTarget | null): boolean {
  return t instanceof HTMLElement && (t.tagName === 'BUTTON' || (t.tagName === 'INPUT' && (t as HTMLInputElement).type === 'checkbox'))
}
