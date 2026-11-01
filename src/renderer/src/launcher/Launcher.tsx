import { calculate, formatNumber } from '@shared/calc'
import type { AppEntry, LauncherInit, RunEvent } from '@shared/types'
import { AppWindow, Calculator, Check, Copy, Globe, LoaderCircle, Search, Sparkles, TriangleAlert, type LucideIcon } from 'lucide-react'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Logo } from '../Logo'
import { looksLikeUrl, matchApps } from './search'

const api = window.winkit.launcher

interface Row {
  key: string
  icon: LucideIcon
  appId?: string
  title: string
  subtitle: string
  run: () => void
}

interface Answer {
  question: string
  text: string
  status: 'streaming' | 'done' | 'error'
  error?: string
}

const iconCache = new Map<string, string | null>()

function AppIcon({ id }: { id: string }): React.JSX.Element {
  const [src, setSrc] = useState(iconCache.get(id) ?? null)
  useEffect(() => {
    if (iconCache.has(id)) return setSrc(iconCache.get(id) ?? null)
    let alive = true
    api.icon(id).then((url) => {
      iconCache.set(id, url)
      if (alive) setSrc(url)
    })
    return () => {
      alive = false
    }
  }, [id])
  return src ? <img className="row-img" src={src} alt="" /> : <AppWindow size={18} className="row-icon" />
}

export function Launcher(): React.JSX.Element {
  const [init, setInit] = useState<LauncherInit | null>(null)
  const [query, setQuery] = useState('')
  const [sel, setSel] = useState(0)
  const [answer, setAnswer] = useState<Answer | null>(null)
  const [copied, setCopied] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  // Id de la última pregunta: la respuesta a una anterior no debe pisar la nueva.
  const requestId = useRef(0)

  useEffect(() => {
    const offInit = api.onInit((data) => {
      setInit(data)
      setQuery('')
      setSel(0)
      setAnswer(null)
      requestAnimationFrame(() => inputRef.current?.focus())
    })
    const offEvent = api.onEvent((ev: RunEvent) => {
      if (ev.id !== requestId.current) return
      setAnswer((a) => {
        if (!a) return a
        if (ev.type === 'delta') return { ...a, text: ev.text }
        if (ev.type === 'done') return { ...a, text: ev.text, status: 'done' }
        return { ...a, status: 'error', error: ev.message }
      })
    })
    return () => {
      offInit()
      offEvent()
    }
  }, [])

  useLayoutEffect(() => {
    const panel = panelRef.current
    if (!panel) return
    const report = (): void => {
      const r = panel.getBoundingClientRect()
      api.size(r.width, r.height)
    }
    report()
    const ro = new ResizeObserver(report)
    ro.observe(panel)
    return () => ro.disconnect()
  }, [])

  const askClaude = (q: string): void => {
    if (!q.trim()) return
    setAnswer({ question: q, text: '', status: 'streaming' })
    setCopied(false)
    api.ask(q.trim(), ++requestId.current)
  }

  const rows = useMemo<Row[]>(() => {
    const q = query.trim()
    if (!q || !init) return []
    const out: Row[] = []
    const result = calculate(q)
    if (result !== null) {
      out.push({
        key: 'calc',
        icon: Calculator,
        title: `= ${formatNumber(result)}`,
        subtitle: 'Calculadora · Enter para copiar',
        run: () => api.copy(formatNumber(result, false))
      })
    }
    if (looksLikeUrl(q)) {
      out.push({ key: 'url', icon: Globe, title: q, subtitle: 'Abrir en el navegador', run: () => api.url(q) })
    }
    const apps: AppEntry[] = result === null ? matchApps(init.apps, q) : []
    for (const a of apps) {
      out.push({
        key: `app:${a.id}`,
        icon: AppWindow,
        appId: a.kind === 'shortcut' ? a.id : undefined,
        title: a.name,
        subtitle: 'Aplicación',
        run: () => api.openApp(a.id)
      })
    }
    const claude: Row = { key: 'claude', icon: Sparkles, title: q, subtitle: `Preguntar a Claude · ${init.modelLabel}`, run: () => askClaude(q) }
    const web: Row = { key: 'web', icon: Search, title: q, subtitle: `Buscar en ${init.engineLabel}`, run: () => api.web(q) }
    // Una pregunta de verdad va antes a Claude; una palabra suelta, antes a la web.
    const isQuestion = /\?\s*$/.test(q) || /^(qué|que|cómo|como|cuál|cual|cuánto|cuanto|por qué|dónde|donde|quién|quien|what|how|why|when|who|explica|explain)\b/i.test(q)
    out.push(...(isQuestion ? [claude, web] : [web, claude]))
    return out
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, init])

  useEffect(() => setSel(0), [query])

  const showingAnswer = answer !== null && answer.question === query

  const copyAnswer = (): void => {
    if (!answer || answer.status !== 'done') return
    api.copy(answer.text)
    setCopied(true)
  }

  const onKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Escape') {
      e.preventDefault()
      return api.close()
    }
    if (e.key === 'Enter' && e.ctrlKey) {
      e.preventDefault()
      return askClaude(query)
    }
    if (showingAnswer) {
      if (e.key === 'Enter') {
        e.preventDefault()
        copyAnswer()
      }
      return
    }
    if (!rows.length) return
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const d = e.key === 'ArrowDown' ? 1 : -1
      setSel((s) => (s + d + rows.length) % rows.length)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      rows[sel]?.run()
    }
  }

  return (
    <div ref={panelRef} className="panel">
      <div className="search">
        <Logo size={26} />
        <input
          ref={inputRef}
          className="search-input"
          value={query}
          placeholder="Busca apps, calcula, busca en la web o pregunta a Claude"
          spellCheck={false}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>

      {showingAnswer && answer ? (
        <div className="answer">
          <div className="answer-head">
            <Sparkles size={14} className="accent" />
            <span>Claude</span>
            {answer.status === 'streaming' && <LoaderCircle size={13} className="spin accent" />}
            <span className="spacer" />
            <button className="btn sm" disabled={answer.status !== 'done'} onClick={copyAnswer}>
              {copied ? <Check size={13} /> : <Copy size={13} />} {copied ? 'Copiado' : 'Copiar'}
            </button>
          </div>
          <div className="answer-body">
            {answer.status === 'error' ? (
              <div className="error">
                <TriangleAlert size={15} /> {answer.error}
              </div>
            ) : answer.text ? (
              <Markdown remarkPlugins={[remarkGfm]}>{answer.text}</Markdown>
            ) : (
              <span className="muted">Pensando…</span>
            )}
          </div>
        </div>
      ) : rows.length > 0 ? (
        <ul className="rows" role="listbox">
          {rows.map((r, i) => (
            <li key={r.key}>
              <button className={i === sel ? 'row active' : 'row'} onMouseEnter={() => setSel(i)} onClick={r.run}>
                {r.appId ? <AppIcon id={r.appId} /> : <r.icon size={18} className="row-icon" />}
                <span className="row-text">
                  <span className="row-title">{r.title}</span>
                  <span className="row-sub">{r.subtitle}</span>
                </span>
                {i === sel && <kbd>Enter</kbd>}
              </button>
            </li>
          ))}
        </ul>
      ) : null}

      <footer className="hints">
        {showingAnswer ? (
          <span>
            <kbd>Enter</kbd> copiar respuesta
          </span>
        ) : (
          <span>
            <kbd>↑</kbd> <kbd>↓</kbd> elegir
          </span>
        )}
        <span>
          <kbd>Ctrl+Enter</kbd> preguntar a Claude
        </span>
        <span>
          <kbd>Esc</kbd> cerrar
        </span>
      </footer>
    </div>
  )
}
