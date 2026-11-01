import type { PopupInit, RunEvent, RunRequest } from '@shared/types'
import {
  ArrowLeft,
  Briefcase,
  Check,
  ChevronRight,
  Copy,
  CornerDownLeft,
  Languages,
  LoaderCircle,
  Scissors,
  Settings,
  Smile,
  Sparkles,
  TriangleAlert,
  WandSparkles,
  type LucideIcon
} from 'lucide-react'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { hasChanges, wordDiff } from '../diff'
import { Logo } from '../Logo'

const api = window.winkit.popup

type Screen = 'menu' | 'languages' | 'run' | 'notice'
type Status = 'streaming' | 'done' | 'error'

interface Item {
  key: string
  label: string
  hint?: string
  icon: LucideIcon
  submenu?: boolean
  select: () => void
}

const BUILTIN_LABELS: Record<string, string> = {
  correct: 'Corregir',
  clearer: 'Mejorar redacción',
  formal: 'Más formal',
  casual: 'Más cercano',
  shorter: 'Más corto'
}

const lower = (lang: string): string => lang.toLocaleLowerCase('es')

function title(req: RunRequest, init: PopupInit | null): string {
  if (req.action === 'translate') return `Traducir al ${lower(req.language ?? '')}`
  return BUILTIN_LABELS[req.action] ?? init?.customActions.find((a) => a.id === req.action)?.name ?? 'Resultado'
}

function progress(req: RunRequest): string {
  if (req.action === 'correct') return 'Corrigiendo…'
  if (req.action === 'translate') return `Traduciendo al ${lower(req.language ?? '')}…`
  return 'Reescribiendo…'
}

export function Popup(): React.JSX.Element {
  const [init, setInit] = useState<PopupInit | null>(null)
  const [screen, setScreen] = useState<Screen>('menu')
  const [request, setRequest] = useState<RunRequest | null>(null)
  const [fromMenu, setFromMenu] = useState(false)
  const [output, setOutput] = useState('')
  const [status, setStatus] = useState<Status>('streaming')
  const [error, setError] = useState<{ message: string; openSettings?: boolean } | null>(null)
  const [showDiff, setShowDiff] = useState(true)
  const [compact, setCompact] = useState(false)
  const [sel, setSel] = useState(0)
  const cardRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const offInit = api.onInit((data) => {
      setInit(data)
      setSel(0)
      setOutput('')
      setError(null)
      const v = data.view
      if (v.kind === 'run') {
        setScreen('run')
        setRequest(v.request)
        setFromMenu(false)
        setStatus('streaming')
        setShowDiff(v.request.action !== 'translate')
        setCompact(v.autoReplace)
      } else {
        setScreen(v.kind)
        setCompact(false)
      }
    })
    const offRun = api.onRunEvent((ev: RunEvent) => {
      if (ev.type === 'delta') setOutput(ev.text)
      else if (ev.type === 'done') {
        setOutput(ev.text)
        setStatus('done')
      } else {
        setError({ message: ev.message, openSettings: ev.openSettings })
        setStatus('error')
      }
    })
    // Con reemplazo automático la viñeta es una píldora sin foco. Si main decide
    // enseñar el resultado (otra ventana activa, error), le da el foco: se expande.
    const onFocus = (): void => setCompact(false)
    window.addEventListener('focus', onFocus)
    return () => {
      offInit()
      offRun()
      window.removeEventListener('focus', onFocus)
    }
  }, [])

  // La ventana se ajusta a la tarjeta.
  useLayoutEffect(() => {
    const card = cardRef.current
    if (!card) return
    const report = (): void => {
      const r = card.getBoundingClientRect()
      api.size(r.width, r.height)
    }
    report()
    const ro = new ResizeObserver(report)
    ro.observe(card)
    return () => ro.disconnect()
  }, [init, screen, compact])

  const start = (req: RunRequest): void => {
    setRequest(req)
    setFromMenu(true)
    setScreen('run')
    setOutput('')
    setError(null)
    setStatus('streaming')
    setShowDiff(req.action !== 'translate')
    api.run(req)
  }

  const text = init?.view.kind === 'menu' || init?.view.kind === 'run' ? init.view.text : ''

  const menuItems = useMemo<Item[]>(() => {
    if (!init) return []
    const target = init.translateTarget
    const items: Item[] = [
      { key: 'correct', label: 'Corregir', hint: 'Ortografía y gramática', icon: Check, select: () => start({ action: 'correct' }) },
      {
        key: 'translate',
        label: `Traducir al ${lower(target)}`,
        icon: Languages,
        select: () => start({ action: 'translate', language: target })
      },
      {
        key: 'translate-other',
        label: 'Traducir a…',
        icon: Languages,
        submenu: true,
        select: () => {
          setScreen('languages')
          setSel(0)
        }
      },
      { key: 'clearer', label: 'Mejorar redacción', icon: Sparkles, select: () => start({ action: 'clearer' }) },
      { key: 'formal', label: 'Más formal', icon: Briefcase, select: () => start({ action: 'formal' }) },
      { key: 'casual', label: 'Más cercano', icon: Smile, select: () => start({ action: 'casual' }) },
      { key: 'shorter', label: 'Más corto', icon: Scissors, select: () => start({ action: 'shorter' }) },
      ...init.customActions.map<Item>((a) => ({
        key: a.id,
        label: a.name || 'Sin nombre',
        icon: WandSparkles,
        select: () => start({ action: a.id })
      }))
    ]
    return items
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [init])

  const languageItems = useMemo<Item[]>(
    () =>
      (init?.languages ?? []).map((lang) => ({
        key: lang,
        label: lang,
        icon: Languages,
        select: () => start({ action: 'translate', language: lang })
      })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [init]
  )

  const items = screen === 'menu' ? menuItems : screen === 'languages' ? languageItems : []

  const diff = useMemo(
    () => (status === 'done' && request && request.action !== 'translate' ? wordDiff(text, output) : null),
    [status, request, text, output]
  )
  const unchanged = diff !== null && !hasChanges(diff)

  const back = (): void => {
    if (screen === 'languages') {
      setScreen('menu')
      setSel(2)
    } else if (screen === 'run' && fromMenu) {
      setScreen('menu')
      setSel(0)
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        e.preventDefault()
        return api.close()
      }
      if (e.key === 'Backspace' || (e.key === 'ArrowLeft' && screen === 'languages')) {
        e.preventDefault()
        return back()
      }
      if (items.length) {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault()
          const d = e.key === 'ArrowDown' ? 1 : -1
          setSel((s) => (s + d + items.length) % items.length)
        } else if (e.key === 'Enter' || (e.key === 'ArrowRight' && items[sel]?.submenu)) {
          e.preventDefault()
          items[sel]?.select()
        } else if (/^[1-9]$/.test(e.key) && !e.ctrlKey && !e.altKey) {
          items[Number(e.key) - 1]?.select()
        }
        return
      }
      if (screen === 'run' && status === 'done') {
        if (e.key === 'Enter') {
          e.preventDefault()
          api.replace(output)
        } else if (e.key.toLowerCase() === 'c' && e.ctrlKey && !window.getSelection()?.toString()) {
          e.preventDefault()
          api.copy(output)
        }
      } else if (screen === 'run' && status === 'error' && e.key === 'Enter' && request) {
        start(request)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  if (!init) return <div ref={cardRef} />

  if (screen === 'run' && compact && request) {
    return (
      <div ref={cardRef} className="card pill">
        {status === 'done' ? <Check size={15} className="ok" /> : <LoaderCircle size={15} className="spin accent" />}
        <span>{status === 'done' ? 'Listo' : progress(request)}</span>
        <kbd>Esc</kbd>
      </div>
    )
  }

  if (screen === 'notice' && init.view.kind === 'notice') {
    const v = init.view
    return (
      <div ref={cardRef} className="card notice">
        <TriangleAlert size={16} className="warn" />
        <span>{v.message}</span>
        {v.openSettings && (
          <button className="btn primary sm" onClick={api.openSettings}>
            Abrir ajustes
          </button>
        )}
      </div>
    )
  }

  if (screen === 'run' && request) {
    return (
      <div ref={cardRef} className="card">
        <header className="head">
          {fromMenu ? (
            <button className="icon-btn" onClick={back} title="Volver (Retroceso)">
              <ArrowLeft size={15} />
            </button>
          ) : (
            <Logo size={20} />
          )}
          <span className="head-title">{title(request, init)}</span>
          {status === 'streaming' && <LoaderCircle size={14} className="spin accent" />}
          <span className="chip">{init.modelLabel}</span>
        </header>

        {status === 'error' && error ? (
          <div className="error">
            <TriangleAlert size={16} />
            <span>{error.message}</span>
          </div>
        ) : (
          <div className="result" aria-live="polite">
            {unchanged && <div className="banner">No he encontrado nada que cambiar.</div>}
            {status === 'streaming' && !output ? (
              <span className="muted">Pensando…</span>
            ) : showDiff && diff && !unchanged ? (
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
                {status === 'streaming' && <span className="caret" />}
              </>
            )}
          </div>
        )}

        <footer className="foot">
          {status === 'error' ? (
            <>
              {error?.openSettings && (
                <button className="btn" onClick={api.openSettings}>
                  <Settings size={14} /> Ajustes
                </button>
              )}
              <button className="btn primary" onClick={() => start(request)}>
                Reintentar <kbd>Enter</kbd>
              </button>
            </>
          ) : (
            <>
              {diff && !unchanged && (
                <label className="toggle">
                  <input type="checkbox" checked={showDiff} onChange={(e) => setShowDiff(e.target.checked)} />
                  Ver cambios
                </label>
              )}
              <span className="spacer" />
              <button className="btn" disabled={status !== 'done'} onClick={() => api.copy(output)}>
                <Copy size={14} /> Copiar
              </button>
              <button className="btn primary" disabled={status !== 'done'} onClick={() => api.replace(output)}>
                Reemplazar <CornerDownLeft size={13} />
              </button>
            </>
          )}
        </footer>
      </div>
    )
  }

  return (
    <div ref={cardRef} className="card">
      <header className="head">
        {screen === 'languages' ? (
          <button className="icon-btn" onClick={back} title="Volver (Retroceso)">
            <ArrowLeft size={15} />
          </button>
        ) : (
          <Logo size={20} />
        )}
        <span className="head-title">{screen === 'languages' ? 'Traducir a…' : 'Winkit'}</span>
        <span className="chip">{init.modelLabel}</span>
        <button className="icon-btn" onClick={api.openSettings} title="Ajustes">
          <Settings size={14} />
        </button>
      </header>
      {text && <p className="preview">{text.trim()}</p>}
      <ul className="menu" role="menu">
        {items.map((it, i) => (
          <li key={it.key}>
            <button
              role="menuitem"
              className={i === sel ? 'item active' : 'item'}
              onMouseEnter={() => setSel(i)}
              onClick={it.select}
            >
              <it.icon size={15} className="item-icon" />
              <span className="item-label">{it.label}</span>
              {it.hint && <span className="item-hint">{it.hint}</span>}
              {it.submenu ? <ChevronRight size={14} className="item-key" /> : i < 9 && <span className="item-key">{i + 1}</span>}
            </button>
          </li>
        ))}
      </ul>
      <footer className="foot hints">
        <span>
          <kbd>↑</kbd> <kbd>↓</kbd> elegir
        </span>
        <span>
          <kbd>Enter</kbd> aplicar
        </span>
        <span>
          <kbd>Esc</kbd> cerrar
        </span>
      </footer>
    </div>
  )
}
