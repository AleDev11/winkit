import { useCallback, useEffect, useRef, useState } from 'react'
import { Dropdown } from '../../components/Dropdown'
import { G, Icon } from '../../components/Icon'
import { useDragHandle } from '../../components/useDragHandle'
import { ProgressRing } from '../../components/ProgressRing'
import { SpeakButton } from '../../components/SpeakButton'
import { palette, ui } from '../../lib/api'
import type { PaletteInit } from '../../shared/types'
import { modelLabel } from './actions'
import { useRun } from './useRun'

const DETECT_DELAY = 600

/**
 * Panel grande para leer lo que te escriben: el original (editable) a la
 * izquierda y la traducción a la derecha. Nunca toca la app de origen.
 */
export function UnderstandView(props: { init: PaletteInit; text: string; language: string }): React.JSX.Element {
  const drag = useDragHandle()
  const { init } = props
  const { state, start } = useRun()
  const [text, setText] = useState(props.text)
  const [language, setLanguage] = useState(props.language)
  const [detected, setDetected] = useState<string | null>(props.text.trim() ? init.language.detected : null)
  const [detecting, setDetecting] = useState(false)
  // El texto que se tradujo por última vez: si cambia, se ofrece traducir otra vez.
  const [translatedFrom, setTranslatedFrom] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  // Si eliges el idioma a mano, la detección ya no lo cambia.
  const pickedByUser = useRef(false)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const outRef = useRef<HTMLDivElement>(null)
  const detectTimer = useRef<number | undefined>(undefined)
  const detectSeq = useRef(0)

  const translate = useCallback(
    (source: string, lang: string) => {
      if (!source.trim()) return
      setCopied(false)
      setTranslatedFrom(source)
      start({ action: 'understand', text: source, language: lang })
    },
    [start]
  )

  // Al abrir: traduce lo que venga; si no hay nada, foco en el original.
  useEffect(() => {
    if (props.text.trim()) translate(props.text, props.language)
    else inputRef.current?.focus()
    return () => window.clearTimeout(detectTimer.current)
  }, [])

  const languageRef = useRef(language)
  languageRef.current = language
  // La ráfaga de cambios en curso empezó con el panel vacío (pegar o escribir desde cero).
  const fromEmpty = useRef(false)

  const onTextChange = (value: string): void => {
    if (!text.trim()) fromEmpty.current = true
    setText(value)
    window.clearTimeout(detectTimer.current)
    if (!value.trim()) {
      setDetected(null)
      return
    }
    const seq = ++detectSeq.current
    detectTimer.current = window.setTimeout(() => {
      const autoRun = fromEmpty.current
      fromEmpty.current = false
      setDetecting(true)
      palette
        .detect(value)
        .then((info) => {
          if (seq !== detectSeq.current) return
          setDetected(info.detected)
          const lang = pickedByUser.current ? languageRef.current : info.understandTo
          setLanguage(lang)
          // Pegar en el panel vacío ya es pedir la traducción.
          if (autoRun) translate(value, lang)
        })
        .catch(() => {
          if (seq === detectSeq.current && autoRun) translate(value, languageRef.current)
        })
        .finally(() => {
          if (seq === detectSeq.current) setDetecting(false)
        })
    }, DETECT_DELAY)
  }

  const onLanguageChange = (lang: string): void => {
    pickedByUser.current = true
    setLanguage(lang)
    translate(text, lang)
  }

  const copy = (): void => {
    if (!state.text) return
    void ui.copy(state.text)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1500)
  }

  const handler = useRef<(e: KeyboardEvent) => void>(() => {})
  handler.current = (e) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      void palette.close()
    } else if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault()
      window.clearTimeout(detectTimer.current)
      translate(text, language)
    }
  }
  useEffect(() => {
    const on = (e: KeyboardEvent): void => handler.current(e)
    window.addEventListener('keydown', on)
    return () => window.removeEventListener('keydown', on)
  }, [])

  const streaming = state.status === 'streaming'
  useEffect(() => {
    const el = outRef.current
    if (el && streaming) el.scrollTop = el.scrollHeight
  }, [state.text, streaming])

  const languages = init.languages.includes(language) ? init.languages : [language, ...init.languages]
  const stale = !streaming && text.trim() !== '' && translatedFrom !== null && text !== translatedFrom
  const empty = !text.trim()

  return (
    <div className="understand-view">
      <header className="u-head" onMouseDown={drag}>
        <Icon glyph={G.book} className="u-head-icon" />
        <span className="body-strong u-title">Entender</span>
        <span className="u-lang">
          {detecting ? (
            <ProgressRing size={14} />
          ) : (
            <span className={detected ? 'chip neutral' : 'chip neutral u-unknown'} title="Idioma detectado">
              <Icon glyph={G.globe} />
              {detected ?? 'Sin detectar'}
            </span>
          )}
          <Icon glyph={G.chevronRight} size={10} className="faint" />
          <Dropdown<string>
            size="sm"
            className="sm"
            label="Traducir a"
            value={language}
            options={languages.map((l) => ({ value: l, label: l }))}
            onChange={onLanguageChange}
          />
        </span>
        <span className="spacer" />
        <span className="caption faint u-model" title={modelLabel(state.model, init.modelLabel)}>
          {modelLabel(state.model, init.modelLabel)}
        </span>
        <SpeakButton text={state.text} disabled={state.status !== 'done'} />
        <button className="btn sm" disabled={state.status !== 'done'} onClick={copy} title="Copiar la traducción">
          <Icon glyph={copied ? G.check : G.copy} />
          <span className="btn-label">{copied ? 'Copiado' : 'Copiar'}</span>
        </button>
        <button className="btn subtle sm icon-only u-close" onClick={() => void palette.close()} title="Cerrar (Esc)" aria-label="Cerrar">
          <Icon glyph={G.close} size={10} />
        </button>
      </header>

      <main className="u-panes">
        <section className="u-pane">
          <div className="u-pane-head">
            <span className="u-pane-label">Original</span>
          </div>
          <textarea
            ref={inputRef}
            className="u-source"
            value={text}
            placeholder="Pega aquí el texto que quieres entender"
            spellCheck={false}
            onChange={(e) => onTextChange(e.target.value)}
          />
        </section>

        <section className="u-pane u-pane-target">
          <div className="u-pane-head">
            <span className="u-pane-label">Traducción</span>
            {streaming && <ProgressRing size={14} />}
            <span className="spacer" />
            {stale && (
              <button className="btn sm accent" onClick={() => translate(text, language)} title="Ctrl+Enter">
                <Icon glyph={G.refresh} /> Traducir de nuevo
              </button>
            )}
          </div>
          <div ref={outRef} className="u-target selectable" aria-live="polite">
            {state.status === 'error' && state.error ? (
              <div className="infobar error" role="alert">
                <Icon glyph={G.error} />
                <div className="infobar-body">
                  <div className="muted">{state.error.message}</div>
                  {state.error.openSettings && (
                    <button className="btn sm u-error-btn" onClick={() => void ui.openSettings()}>
                      Abrir ajustes
                    </button>
                  )}
                </div>
              </div>
            ) : empty && state.status === 'idle' ? (
              <div className="u-empty">
                <Icon glyph={G.translate} size={32} />
                <span>La traducción aparecerá aquí</span>
              </div>
            ) : streaming && !state.text ? (
              <span className="faint">Traduciendo…</span>
            ) : (
              <>
                {state.text}
                {streaming && <span className="caret" />}
              </>
            )}
          </div>
        </section>
      </main>

      <footer className="palette-foot hints">
        <span className="u-hint">
          <kbd>Ctrl</kbd>
          <kbd>Enter</kbd> traducir
        </span>
        <span className="u-hint">
          <kbd>Esc</kbd> cerrar
        </span>
        <span className="spacer" />
        <span className="u-note">No se reemplaza nada: solo es para leer.</span>
      </footer>
    </div>
  )
}
