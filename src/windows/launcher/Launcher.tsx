import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import Markdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { G, Icon } from '../../components/Icon'
import { ProgressRing } from '../../components/ProgressRing'
import { useAutoResize } from '../../components/useAutoResize'
import { launcher, ui } from '../../lib/api'
import { MODELS, type AppEntry, type LauncherInit } from '../../shared/types'
import { AppIcon } from './AppIcon'
import { buildRows, type Group, type Row } from './results'

interface Answer {
  question: string
  text: string
  status: 'streaming' | 'done' | 'error'
  model: string | null
  error?: { message: string; openSettings: boolean }
}

const HINTS: { glyph: string; title: string; example: string; query: string }[] = [
  { glyph: G.app, title: 'Abrir una app', example: 'word, spotify, terminal', query: '' },
  { glyph: G.calculator, title: 'Calcular', example: '20% de 340', query: '20% de 340' },
  { glyph: G.ruler, title: 'Convertir unidades', example: '10 km a mi', query: '10 km a mi' },
  { glyph: G.power, title: 'Comandos del sistema', example: 'bloquear, papelera, apagar', query: 'bloquear' },
  { glyph: G.globe, title: 'Buscar en la web', example: 'o abrir una dirección', query: '' },
  { glyph: G.sparkle, title: 'Preguntar a Claude', example: 'Ctrl+Enter con lo que escribas', query: '' }
]

const GROUP_LABELS: Record<Group, string> = {
  calc: 'Calculadora',
  units: 'Unidades',
  url: 'Web',
  system: 'Sistema',
  apps: 'Aplicaciones',
  web: 'Web',
  claude: 'Claude'
}

// Los enlaces de las respuestas se abren en el navegador, no dentro de la ventana.
const markdownComponents: Components = {
  a: ({ href, children }) => (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault()
        if (href && /^https?:\/\//i.test(href)) void launcher.openUrl(href)
      }}
    >
      {children}
    </a>
  )
}

export function Launcher(): React.JSX.Element {
  const [init, setInit] = useState<LauncherInit | null>(null)
  const [apps, setApps] = useState<AppEntry[]>([])
  const [query, setQuery] = useState('')
  const [sel, setSel] = useState(0)
  // Fila de un comando peligroso a la espera del segundo Enter.
  const [armed, setArmed] = useState<string | null>(null)
  const [answer, setAnswer] = useState<Answer | null>(null)
  const [copied, setCopied] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLUListElement>(null)
  const answerRef = useRef<HTMLDivElement>(null)
  const session = useRef<number | null>(null)
  // La respuesta a una pregunta anterior no debe pisar la nueva.
  const askId = useRef(0)

  useEffect(() => {
    const focus = (): void => {
      inputRef.current?.focus()
      inputRef.current?.select()
    }
    const off = launcher.onInit((data) => {
      setInit(data)
      if (session.current !== data.session) {
        session.current = data.session
        askId.current++
        setQuery('')
        setSel(0)
        setArmed(null)
        setAnswer(null)
        setCopied(false)
        launcher
          .apps()
          .then(setApps)
          .catch(() => {})
      }
      requestAnimationFrame(focus)
    })
    // Esc cierra aunque el foco se haya ido del campo (p. ej. al seleccionar texto de una respuesta).
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape' && e.target !== inputRef.current) {
        e.preventDefault()
        void launcher.hide()
      }
    }
    window.addEventListener('focus', focus)
    window.addEventListener('keydown', onKey)
    return () => {
      off()
      window.removeEventListener('focus', focus)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  const rows = useMemo(
    () => (init ? buildRows(query, apps, init.engineLabel, init.modelLabel) : []),
    [query, apps, init]
  )

  useEffect(() => {
    setSel(0)
    setArmed(null)
  }, [query])

  useLayoutEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-index="${sel}"]`)
    el?.scrollIntoView({ block: 'nearest' })
  }, [sel])

  const ask = useCallback((question: string) => {
    const q = question.trim()
    if (!q) return
    const id = ++askId.current
    setCopied(false)
    setAnswer({ question: q, text: '', status: 'streaming', model: null })
    launcher
      .ask(q, (ev) => {
        if (id !== askId.current) return
        setAnswer((a) => {
          if (!a) return a
          switch (ev.type) {
            case 'start':
              return { ...a, model: ev.model }
            case 'delta':
              return { ...a, text: ev.text }
            case 'done':
              return { ...a, text: ev.text, status: 'done' }
            case 'error':
              return { ...a, status: 'error', error: { message: ev.message, openSettings: ev.openSettings } }
          }
        })
      })
      .catch((err: unknown) => {
        if (id !== askId.current) return
        setAnswer((a) => a && { ...a, status: 'error', error: { message: String(err), openSettings: false } })
      })
  }, [])

  const showingAnswer = answer !== null && answer.question === query.trim()

  const execute = (row: Row): void => {
    const a = row.action
    switch (a.kind) {
      case 'copy':
        void launcher.copyAndClose(a.text)
        break
      case 'system':
        if (a.confirm && armed !== row.key) {
          setArmed(row.key)
          return
        }
        void launcher.systemCommand(a.id)
        break
      case 'app':
        void launcher.openApp(a.id)
        break
      case 'url':
        void launcher.openUrl(a.url)
        break
      case 'web':
        void launcher.webSearch(a.query)
        break
      case 'ask':
        ask(a.question)
        break
    }
  }

  const copyAnswer = (): void => {
    if (answer?.status !== 'done') return
    setCopied(true)
    void launcher.copyAndClose(answer.text)
  }

  // Tab salta al primer resultado de la siguiente sección.
  const jumpGroup = (dir: 1 | -1): void => {
    if (!rows.length) return
    const starts = rows.map((r, i) => (i === 0 || rows[i - 1].group !== r.group ? i : -1)).filter((i) => i >= 0)
    const current = starts.filter((i) => i <= sel).pop() ?? 0
    const k = starts.indexOf(current)
    setSel(starts[(k + dir + starts.length) % starts.length])
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === 'Escape') {
      e.preventDefault()
      void launcher.hide()
      return
    }
    if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault()
      ask(query)
      return
    }
    if (showingAnswer) {
      if (e.key === 'Enter') {
        e.preventDefault()
        copyAnswer()
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'PageDown' || e.key === 'PageUp') {
        e.preventDefault()
        const step = e.key.startsWith('Page') ? 240 : 48
        answerRef.current?.scrollBy({ top: e.key === 'ArrowDown' || e.key === 'PageDown' ? step : -step })
      }
      return
    }
    if (!rows.length) return
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const d = e.key === 'ArrowDown' ? 1 : -1
      setSel((s) => (s + d + rows.length) % rows.length)
      setArmed(null)
    } else if (e.key === 'Tab') {
      e.preventDefault()
      jumpGroup(e.shiftKey ? -1 : 1)
      setArmed(null)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const row = rows[sel]
      if (row) execute(row)
    }
  }

  const rootRef = useAutoResize<HTMLDivElement>(launcher.resize, init?.session)

  return (
    <div ref={rootRef} className="popup launcher">
      <div className="search">
        <Icon glyph={G.search} className="search-icon" />
        <input
          ref={inputRef}
          className="search-input"
          value={query}
          placeholder="Busca apps, calcula, convierte o pregunta a Claude"
          spellCheck={false}
          autoFocus
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          aria-controls="results"
          aria-activedescendant={!showingAnswer && rows[sel] ? `row-${sel}` : undefined}
        />
        {query && (
          <button
            className="btn subtle sm icon-only search-clear"
            tabIndex={-1}
            onClick={() => {
              setQuery('')
              inputRef.current?.focus()
            }}
            aria-label="Borrar"
            title="Borrar"
          >
            <Icon glyph={G.cancel} size={10} />
          </button>
        )}
      </div>

      {showingAnswer && answer ? (
        <section className="answer fade-in" aria-live="polite">
          <header className="answer-head">
            <Icon glyph={G.sparkle} className="answer-icon" />
            <span className="body-strong">Claude</span>
            <span className="caption faint">{MODELS.find((m) => m.id === answer.model)?.label ?? answer.model ?? init?.modelLabel}</span>
            {answer.status === 'streaming' && <ProgressRing size={14} />}
            <span className="spacer" />
            <button className="btn sm" disabled={answer.status !== 'done'} onClick={copyAnswer}>
              <Icon glyph={copied ? G.check : G.copy} /> Copiar
            </button>
          </header>
          <div ref={answerRef} className="answer-body selectable">
            {answer.status === 'error' && answer.error ? (
              <div className="infobar error" role="alert">
                <Icon glyph={G.error} />
                <div className="infobar-body">
                  <div className="muted">{answer.error.message}</div>
                  {answer.error.openSettings && (
                    <button className="btn sm answer-settings" onClick={() => void ui.openSettings()}>
                      Abrir ajustes
                    </button>
                  )}
                </div>
              </div>
            ) : answer.text ? (
              <div className="markdown">
                <Markdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                  {answer.text}
                </Markdown>
              </div>
            ) : (
              <span className="faint">Pensando…</span>
            )}
          </div>
        </section>
      ) : rows.length > 0 ? (
        <ul id="results" ref={listRef} className="rows" role="listbox" aria-label="Resultados">
          {rows.map((r, i) => {
            const isArmed = armed === r.key
            const newGroup = i > 0 && rows[i - 1].group !== r.group
            return (
              <li
                key={r.key}
                id={`row-${i}`}
                data-index={i}
                role="option"
                aria-selected={i === sel}
                className={[
                  'row',
                  i === sel ? 'selected' : '',
                  isArmed ? 'armed' : '',
                  newGroup ? 'group-start' : '',
                  r.group === 'calc' || r.group === 'units' ? 'big' : ''
                ].join(' ')}
                onMouseMove={() => i !== sel && setSel(i)}
                onClick={() => execute(r)}
              >
                <span className="row-icon">{r.appId ? <AppIcon id={r.appId} /> : <Icon glyph={r.glyph} className="row-glyph" />}</span>
                <span className="row-text">
                  <span className="row-title">{r.title}</span>
                  <span className="row-sub">
                    {isArmed ? (
                      <span className="row-confirm">
                        <Icon glyph={G.warning} size={12} /> Pulsa Enter otra vez para confirmar
                      </span>
                    ) : (
                      r.subtitle
                    )}
                  </span>
                </span>
                <span className="row-hint">
                  {i === sel ? (
                    <>
                      <span>{isArmed ? 'Confirmar' : r.verb}</span>
                      <kbd>Enter</kbd>
                    </>
                  ) : r.group === 'claude' ? (
                    <kbd>Ctrl+Enter</kbd>
                  ) : (
                    i === 0 || newGroup ? <span className="row-group">{GROUP_LABELS[r.group]}</span> : null
                  )}
                </span>
              </li>
            )
          })}
        </ul>
      ) : (
        <div className="start">
          {HINTS.map((h) => (
            <button
              key={h.title}
              className="start-tile"
              tabIndex={-1}
              onClick={() => {
                if (h.query) setQuery(h.query)
                inputRef.current?.focus()
              }}
            >
              <Icon glyph={h.glyph} className="start-icon" />
              <span className="start-text">
                <span className="start-title">{h.title}</span>
                <span className="start-example">{h.example}</span>
              </span>
            </button>
          ))}
        </div>
      )}

      <footer className="launcher-foot hints">
        {showingAnswer ? (
          <>
            <span>
              <kbd>Enter</kbd> copiar y cerrar
            </span>
            <span>
              <kbd>↑</kbd>
              <kbd>↓</kbd> desplazar
            </span>
            <span>Escribe para volver a los resultados</span>
          </>
        ) : (
          <>
            <span>
              <kbd>↑</kbd>
              <kbd>↓</kbd> elegir
            </span>
            <span>
              <kbd>Tab</kbd> sección
            </span>
            <span>
              <kbd>Ctrl</kbd>
              <kbd>Enter</kbd> preguntar a Claude
            </span>
          </>
        )}
        <span className="spacer" />
        <span>
          <kbd>Esc</kbd> cerrar
        </span>
      </footer>
    </div>
  )
}
