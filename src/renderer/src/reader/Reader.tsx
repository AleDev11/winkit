import type { ReaderInit, RunEvent } from '@shared/types'
import { Check, Copy, Languages, LoaderCircle, TriangleAlert } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Logo } from '../Logo'

const api = window.winkit.reader

type Status = 'idle' | 'streaming' | 'done' | 'error'

export function Reader(): React.JSX.Element {
  const [init, setInit] = useState<ReaderInit | null>(null)
  const [text, setText] = useState('')
  const [language, setLanguage] = useState('Español')
  const [translation, setTranslation] = useState('')
  const [status, setStatus] = useState<Status>('idle')
  const [error, setError] = useState('')
  const [copied, setCopied] = useState(false)
  // El texto que se tradujo por última vez: si el original cambia, se ofrece volver a traducir.
  const [translatedFrom, setTranslatedFrom] = useState('')
  const inputRef = useRef<HTMLTextAreaElement>(null)
  // Id de la última traducción pedida: lo que llegue de una anterior se ignora.
  const requestId = useRef(0)

  const translate = useCallback((source: string, lang: string) => {
    if (!source.trim()) return
    setTranslation('')
    setError('')
    setStatus('streaming')
    setTranslatedFrom(source)
    api.translate(source.trim(), lang, ++requestId.current)
  }, [])

  useEffect(() => {
    const offInit = api.onInit((data) => {
      setInit(data)
      setText(data.text)
      setLanguage(data.language)
      setCopied(false)
      if (data.text) translate(data.text, data.language)
      else {
        setTranslation('')
        setStatus('idle')
        setTranslatedFrom('')
        inputRef.current?.focus()
      }
    })
    const offEvent = api.onEvent((ev: RunEvent) => {
      if (ev.id !== requestId.current) return
      if (ev.type === 'delta') setTranslation(ev.text)
      else if (ev.type === 'done') {
        setTranslation(ev.text)
        setStatus('done')
      } else {
        setError(ev.message)
        setStatus('error')
      }
    })
    return () => {
      offInit()
      offEvent()
    }
  }, [translate])

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') api.close()
      else if (e.key === 'Enter' && e.ctrlKey) {
        e.preventDefault()
        translate(text, language)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [text, language, translate])

  const copy = (): void => {
    api.copy(translation)
    setCopied(true)
    setTimeout(() => setCopied(false), 1400)
  }

  const stale = status !== 'streaming' && text.trim() !== '' && text !== translatedFrom

  return (
    <div className="reader">
      <header className="bar">
        <Logo size={22} />
        <span className="bar-title">Lector</span>
        {init && <span className="chip">{init.modelLabel}</span>}
      </header>

      <main className="panes">
        <section className="pane">
          <div className="pane-head">
            <span className="pane-label">Original</span>
            <span className="spacer" />
            {stale && (
              <button className="btn primary sm" onClick={() => translate(text, language)}>
                <Languages size={13} /> Traducir <kbd>Ctrl+Enter</kbd>
              </button>
            )}
          </div>
          <textarea
            ref={inputRef}
            className="pane-body source"
            value={text}
            placeholder="Pega o escribe aquí el texto que quieres entender."
            spellCheck={false}
            onChange={(e) => setText(e.target.value)}
            onPaste={(e) => {
              // Pegar en un lector vacío es pedir la traducción: no hace falta otro clic.
              const pasted = e.clipboardData.getData('text')
              if (!text.trim() && pasted.trim()) setTimeout(() => translate(pasted, language), 0)
            }}
          />
        </section>

        <section className="pane">
          <div className="pane-head">
            <select
              className="lang"
              value={language}
              onChange={(e) => {
                setLanguage(e.target.value)
                translate(text, e.target.value)
              }}
            >
              {(init?.languages ?? [language]).map((l) => (
                <option key={l}>{l}</option>
              ))}
            </select>
            {status === 'streaming' && <LoaderCircle size={14} className="spin accent" />}
            <span className="spacer" />
            <button className="btn sm" disabled={status !== 'done'} onClick={copy}>
              {copied ? <Check size={13} /> : <Copy size={13} />} {copied ? 'Copiado' : 'Copiar'}
            </button>
          </div>
          <div className="pane-body target" aria-live="polite">
            {status === 'error' ? (
              <div className="error">
                <TriangleAlert size={16} /> {error}
              </div>
            ) : status === 'idle' ? (
              <span className="muted">La traducción aparecerá aquí.</span>
            ) : status === 'streaming' && !translation ? (
              <span className="muted">Traduciendo…</span>
            ) : (
              <>
                {translation}
                {status === 'streaming' && <span className="caret" />}
              </>
            )}
          </div>
        </section>
      </main>

      <footer className="hints">
        <span>
          <kbd>Ctrl+Enter</kbd> traducir de nuevo
        </span>
        <span>
          <kbd>Esc</kbd> cerrar
        </span>
      </footer>
    </div>
  )
}
