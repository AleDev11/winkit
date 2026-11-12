import { useCallback, useEffect, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { Logo } from '../../components/Logo'
import { ProgressRing } from '../../components/ProgressRing'
import { settings as api } from '../../lib/api'
import type { Settings, SettingsState, ShortcutStatus } from '../../shared/types'
import { AboutPage, ActionsPage, ColorPage, GeneralPage, LanguagesPage, SearchPage, ShortcutsPage } from './pages'
import { AiPage } from './AiPage'
import { ClipsPage } from './ClipsPage'
import { VoicePage } from './VoicePage'

export type PageId = 'general' | 'ai' | 'voice' | 'shortcuts' | 'languages' | 'actions' | 'search' | 'color' | 'clips' | 'about'

const PAGES: { id: PageId; label: string; glyph: string }[] = [
  { id: 'general', label: 'General', glyph: G.settings },
  { id: 'ai', label: 'IA', glyph: G.sparkle },
  { id: 'voice', label: 'Voz', glyph: G.volume },
  { id: 'shortcuts', label: 'Atajos', glyph: G.keyboard },
  { id: 'languages', label: 'Idiomas', glyph: G.language },
  { id: 'actions', label: 'Acciones', glyph: G.lightning },
  { id: 'search', label: 'Buscador', glyph: G.search },
  { id: 'color', label: 'Color', glyph: G.color },
  { id: 'clips', label: 'Clips', glyph: G.record },
  { id: 'about', label: 'Acerca de', glyph: G.info }
]

type SaveState = 'idle' | 'saving' | 'saved' | 'error'

export interface PageProps {
  s: Settings
  state: SettingsState
  save: (patch: Partial<Settings>) => void
  setState: React.Dispatch<React.SetStateAction<SettingsState | null>>
  setShortcutStatus: (status: ShortcutStatus[]) => void
}

export function SettingsApp(): React.JSX.Element {
  const [state, setState] = useState<SettingsState | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [page, setPage] = useState<PageId>('general')
  const [saveState, setSaveState] = useState<SaveState>('idle')
  const saveSeq = useRef(0)
  const savedTimer = useRef<number | undefined>(undefined)
  const contentRef = useRef<HTMLElement>(null)

  useEffect(() => {
    api
      .get()
      .then((st) => {
        setState(st)
        // Sin IA lista para escribir, lo primero es configurarla.
        if (!st.aiReady) setPage('ai')
      })
      .catch((err: unknown) => setLoadError(String(err)))
  }, [])

  useEffect(() => {
    contentRef.current?.scrollTo({ top: 0 })
  }, [page])

  const save = useCallback((patch: Partial<Settings>) => {
    const seq = ++saveSeq.current
    setState((st) => (st ? { ...st, settings: { ...st.settings, ...patch } } : st))
    setSaveState('saving')
    window.clearTimeout(savedTimer.current)
    api
      .save(patch)
      .then((r) => {
        if (seq !== saveSeq.current) return
        setState((st) => (st ? { ...st, settings: r.settings, shortcuts: r.shortcuts } : st))
        setSaveState('saved')
        savedTimer.current = window.setTimeout(() => setSaveState('idle'), 1800)
      })
      .catch(() => {
        if (seq === saveSeq.current) setSaveState('error')
      })
  }, [])

  const setShortcutStatus = useCallback((shortcuts: ShortcutStatus[]) => {
    setState((st) => (st ? { ...st, shortcuts } : st))
  }, [])

  const current = PAGES.find((p) => p.id === page)!

  return (
    <div className="settings">
      <nav className="nav" aria-label="Secciones">
        <div className="nav-app">
          <Logo size={36} />
          <span className="nav-app-text">
            <span className="body-strong">Winkit</span>
            <span className="caption muted">Ajustes</span>
          </span>
        </div>
        <ul className="nav-list">
          {PAGES.map((p) => (
            <li key={p.id}>
              <button
                className={p.id === page ? 'nav-item selected' : 'nav-item'}
                aria-current={p.id === page ? 'page' : undefined}
                onClick={() => setPage(p.id)}
              >
                <Icon glyph={p.glyph} className="nav-icon" />
                <span>{p.label}</span>
                {p.id === 'ai' && state && !state.aiReady && <span className="nav-badge" title="Falta configurar la IA" />}
              </button>
            </li>
          ))}
        </ul>
      </nav>

      <main ref={contentRef} className="content">
        <header className="page-head">
          <h1 className="title">{current.label}</h1>
          <span className={`save-state ${saveState}`} role="status" aria-live="polite">
            {saveState === 'saving' && (
              <>
                <ProgressRing size={14} /> Guardando…
              </>
            )}
            {saveState === 'saved' && (
              <>
                <Icon glyph={G.check} size={12} /> Guardado
              </>
            )}
            {saveState === 'error' && (
              <>
                <Icon glyph={G.error} size={12} /> No se ha podido guardar
              </>
            )}
          </span>
        </header>

        {loadError ? (
          <div className="infobar error">
            <Icon glyph={G.error} />
            <div className="infobar-body">
              <div className="body-strong">No se han podido cargar los ajustes</div>
              <div className="muted">{loadError}</div>
            </div>
          </div>
        ) : !state ? (
          <div className="page-loading">
            <ProgressRing size={32} />
          </div>
        ) : (
          <div className="page fade-in" key={page}>
            {(() => {
              const props: PageProps = { s: state.settings, state, save, setState, setShortcutStatus }
              switch (page) {
                case 'general':
                  return <GeneralPage {...props} />
                case 'ai':
                  return <AiPage {...props} />
                case 'shortcuts':
                  return <ShortcutsPage {...props} />
                case 'languages':
                  return <LanguagesPage {...props} />
                case 'actions':
                  return <ActionsPage {...props} />
                case 'search':
                  return <SearchPage {...props} />
                case 'color':
                  return <ColorPage {...props} />
                case 'voice':
                  return <VoicePage {...props} />
                case 'clips':
                  return <ClipsPage {...props} />
                case 'about':
                  return <AboutPage {...props} />
              }
            })()}
          </div>
        )}
      </main>
    </div>
  )
}
