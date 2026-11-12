import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { useDragHandle } from '../../components/useDragHandle'
import { palette } from '../../lib/api'
import { fold } from '../../shared/search'
import type { PaletteInit, RunRequest } from '../../shared/types'
import { BUILTINS, MENU_ORDER, oneLine } from './actions'
import type { View } from './Palette'

export interface MenuState {
  query: string
  translateTo: string
}

interface Item {
  key: string
  glyph: string
  label: string
  /** Texto secundario a la derecha (p. ej. «Inglés → Español»). */
  detail?: string
  /** Número de la tecla rápida (solo con el campo vacío). */
  shortcut?: number
  select: () => void
}

const lower = (s: string): string => s.toLocaleLowerCase('es')

export function MenuView(props: {
  init: PaletteInit
  state: MenuState
  setState: (fn: (s: MenuState) => MenuState) => void
  go: (view: View) => void
}): React.JSX.Element {
  const drag = useDragHandle()
  const { init, state, setState, go } = props
  const [sel, setSel] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLUListElement>(null)

  const text = init.text
  const detected = init.language.detected
  const translateTo = state.translateTo || init.language.translateTo
  const query = state.query

  // Destinos por los que va pasando Tab: todos menos el idioma del propio texto.
  const targets = useMemo(() => {
    const list = init.languages.filter((l) => !detected || lower(l) !== lower(detected))
    return list.length ? list : init.languages
  }, [init.languages, detected])

  const cycleTarget = (dir: 1 | -1): void => {
    if (!targets.length) return
    const i = targets.findIndex((l) => lower(l) === lower(translateTo))
    const next = targets[(i + dir + targets.length) % targets.length]
    setState((s) => ({ ...s, translateTo: next }))
  }

  const run = (request: Omit<RunRequest, 'text'>): void => go({ kind: 'run', request: { ...request, text }, fromMenu: true })

  const items = useMemo<Item[]>(() => {
    const all: Item[] = MENU_ORDER.map((action) => {
      const info = BUILTINS[action]
      const item: Item = { key: action, glyph: info.glyph, label: info.label, select: () => run({ action }) }
      if (action === 'translate') {
        item.detail = detected ? `${detected} → ${translateTo}` : `→ ${translateTo}`
        item.select = () => run({ action, language: translateTo })
      } else if (action === 'understand') {
        item.select = () => go({ kind: 'understand', text, language: init.language.understandTo })
      }
      return item
    })
    for (const a of init.customActions) {
      all.push({ key: a.id, glyph: G.lightning, label: a.name || 'Sin nombre', select: () => run({ action: a.id }) })
    }
    all.forEach((it, i) => {
      if (i < 9) it.shortcut = i + 1
    })

    const q = fold(query.trim())
    if (!q) return all

    const keywords = (it: Item): string => fold(`${it.label} ${isBuiltinKey(it.key) ? BUILTINS[it.key].keywords : ''}`)
    const matches: Item[] = all.filter((it) => keywords(it).includes(q)).map((it) => ({ ...it, shortcut: undefined }))

    // Escribir un idioma («fran») ofrece traducir directamente a él.
    if (q.length >= 3) {
      for (const lang of init.languages) {
        if (fold(lang).startsWith(q) && lower(lang) !== lower(detected ?? '')) {
          matches.push({
            key: `translate:${lang}`,
            glyph: G.translate,
            label: `Traducir al ${lower(lang)}`,
            detail: detected ? `${detected} → ${lang}` : undefined,
            select: () => run({ action: 'translate', language: lang })
          })
        }
      }
    }

    const ask: Item = {
      key: 'instruction',
      glyph: G.message,
      label: `Pedir: «${query.trim()}»`,
      select: () => run({ action: 'instruction', instruction: query.trim() })
    }
    // Sin coincidencias, lo escrito es una instrucción; con ellas, queda como última opción.
    return matches.length ? [...matches, ask] : [ask]
  }, [init, query, translateTo, detected, text])

  useEffect(() => setSel(0), [query])

  useLayoutEffect(() => {
    const el = listRef.current?.children[sel] as HTMLElement | undefined
    el?.scrollIntoView({ block: 'nearest' })
  }, [sel])

  useEffect(() => {
    inputRef.current?.focus()
    // La ventana se reutiliza: al volver a enseñarse, el foco vuelve al campo.
    const onFocus = (): void => inputRef.current?.focus()
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [])

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === 'Escape') {
      e.preventDefault()
      void palette.close()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const d = e.key === 'ArrowDown' ? 1 : -1
      setSel((s) => (s + d + items.length) % items.length)
    } else if (e.key === 'Home' && !query) {
      e.preventDefault()
      setSel(0)
    } else if (e.key === 'End' && !query) {
      e.preventDefault()
      setSel(items.length - 1)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      items[sel]?.select()
    } else if (e.key === 'Tab') {
      e.preventDefault()
      cycleTarget(e.shiftKey ? -1 : 1)
    } else if (!query && /^[1-9]$/.test(e.key) && !e.ctrlKey && !e.altKey && !e.metaKey) {
      const it = items.find((x) => x.shortcut === Number(e.key))
      if (it) {
        e.preventDefault()
        it.select()
      }
    }
  }

  const preview = oneLine(text)

  return (
    // El foco se queda siempre en el campo: así las teclas funcionan aunque se haga clic en la lista.
    <div className="menu-view" onMouseDown={(e) => e.target !== inputRef.current && e.preventDefault()}>
      <header className="menu-head" onMouseDown={drag}>
        {preview && (
          <div className="menu-context">
            <p className="menu-preview" title={text.length > 400 ? undefined : text}>
              {preview}
            </p>
            {detected && (
              <span className="chip neutral" title="Idioma detectado">
                <Icon glyph={G.globe} />
                {detected}
              </span>
            )}
          </div>
        )}
        <div className="menu-search">
          <Icon glyph={G.search} className="menu-search-icon" />
          <input
            ref={inputRef}
            className="input menu-input"
            value={query}
            placeholder="Escribe una instrucción o filtra…"
            spellCheck={false}
            autoFocus
            onChange={(e) => setState((s) => ({ ...s, query: e.target.value }))}
            onKeyDown={onKeyDown}
            aria-controls="menu-list"
            aria-activedescendant={items[sel] ? `menu-item-${sel}` : undefined}
          />
        </div>
      </header>

      <ul id="menu-list" ref={listRef} className="menu-list" role="listbox" aria-label="Acciones">
        {items.map((it, i) => (
          <li
            key={it.key}
            id={`menu-item-${i}`}
            role="option"
            aria-selected={i === sel}
            className={i === sel ? 'menu-item selected' : 'menu-item'}
            onMouseMove={() => i !== sel && setSel(i)}
            onClick={it.select}
          >
            <Icon glyph={it.glyph} className="menu-item-icon" />
            <span className="menu-item-label">{it.label}</span>
            {it.detail && (
              <span className="menu-item-detail">
                {it.detail}
                {it.key === 'translate' && <kbd title="Tab cambia el idioma de destino">Tab</kbd>}
              </span>
            )}
            {it.shortcut !== undefined && !query && <span className="menu-item-key">{it.shortcut}</span>}
          </li>
        ))}
      </ul>

      <footer className="palette-foot hints">
        <span>
          <kbd>↑</kbd>
          <kbd>↓</kbd> elegir
        </span>
        <span>
          <kbd>Enter</kbd> aplicar
        </span>
        <span>
          <kbd>Tab</kbd> idioma
        </span>
        <span className="spacer" />
        <span>
          <kbd>Esc</kbd> cerrar
        </span>
      </footer>
    </div>
  )
}

function isBuiltinKey(key: string): key is keyof typeof BUILTINS {
  return key in BUILTINS
}
