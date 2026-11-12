import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { fold } from '../shared/search'
import { G, Icon } from './Icon'

/**
 * Desplegable con el aspecto de los de Windows 11 (ComboBox de Fluent), en
 * lugar del <select> nativo. El botón tiene un ancho fijo y recorta el texto;
 * la lista sale en una capa propia (no la recortan las tarjetas), se abre
 * hacia arriba si abajo no cabe, tiene scroll y, con muchas opciones, un
 * buscador. Teclado: ↑ ↓ Inicio Fin RePág AvPág Enter Esc Tab, y escribir
 * para saltar a una opción (o filtrar, si hay buscador).
 */

export interface DropdownOption<T extends string> {
  value: T
  label: string
  /** Segunda línea, más pequeña, solo en la lista ("Recomendado · 0,30 $…"). */
  hint?: string | null
  disabled?: boolean
}

export interface DropdownProps<T extends string> {
  value: T
  options: DropdownOption<T>[]
  onChange: (v: T) => void
  /** Nombre accesible (no se ve). */
  label: string
  disabled?: boolean
  /** Buscador en la lista. Por defecto, si hay más de 10 opciones. */
  searchable?: boolean
  /** Texto cuando el valor no está entre las opciones. */
  placeholder?: string
  /** Ancho del botón: "sm" 140 px, "md" 220 px (por defecto), "lg" 300 px. */
  size?: 'sm' | 'md' | 'lg'
  className?: string
}

const WIDTHS = { sm: 140, md: 220, lg: 300 }
const GAP = 4 // separación entre el botón y la lista
const MARGIN = 8 // margen con el borde de la ventana

interface Place {
  left: number
  width: number
  top?: number
  bottom?: number
  maxHeight: number
  up: boolean
}

export function Dropdown<T extends string>(props: DropdownProps<T>): React.JSX.Element {
  const { value, options, onChange, label, disabled } = props
  const id = useId()
  const listId = `${id}-list`
  const triggerRef = useRef<HTMLButtonElement>(null)
  const popupRef = useRef<HTMLDivElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const typeahead = useRef({ text: '', at: 0 })

  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(-1)
  const [place, setPlace] = useState<Place | null>(null)

  const searchable = props.searchable ?? options.length > 10
  const current = options.find((o) => o.value === value)

  const visible = useMemo(() => {
    const q = fold(query.trim())
    if (!q) return options
    return options.filter((o) => fold(`${o.label} ${o.hint ?? ''} ${o.value}`).includes(q))
  }, [options, query])

  const close = useCallback((refocus = true) => {
    setOpen(false)
    setQuery('')
    if (refocus) triggerRef.current?.focus()
  }, [])

  const choose = (o: DropdownOption<T> | undefined): void => {
    if (!o || o.disabled) return
    if (o.value !== value) onChange(o.value)
    close()
  }

  const openList = (): void => {
    if (disabled) return
    setOpen(true)
    setQuery('')
    setActive(Math.max(0, options.findIndex((o) => o.value === value)))
  }

  // Colocación: debajo del botón, o encima si abajo no cabe; nunca fuera de la ventana.
  const measure = useCallback(() => {
    const t = triggerRef.current
    if (!t) return
    const r = t.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight
    const below = vh - r.bottom - GAP - MARGIN
    const above = r.top - GAP - MARGIN
    const wanted = Math.min(360, (searchable ? 48 : 8) + options.length * 36)
    const up = below < Math.min(wanted, 200) && above > below
    // Tan ancha como pida el texto más largo (como la de Windows), entre el
    // ancho del botón y 440 px; lo que aún no quepa se recorta con «…».
    const longest = Math.max(0, ...options.map((o) => Math.max(o.label.length, (o.hint?.length ?? 0) * 0.86)))
    const width = Math.min(Math.max(r.width, Math.ceil(longest * 7.4 + 48)), 440, vw - MARGIN * 2)
    const left = Math.min(Math.max(MARGIN, r.left), vw - width - MARGIN)
    setPlace(
      up
        ? { left, width, bottom: vh - r.top + GAP, maxHeight: Math.min(360, above), up }
        : { left, width, top: r.bottom + GAP, maxHeight: Math.min(360, below), up }
    )
  }, [options.length, searchable])

  useLayoutEffect(() => {
    if (open) measure()
  }, [open, measure])

  // Mientras está abierta: cerrar al hacer clic fuera, al perder la ventana el
  // foco o al desplazar la página; recolocar si cambia el tamaño.
  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent): void => {
      const target = e.target as Node
      if (popupRef.current?.contains(target) || triggerRef.current?.contains(target)) return
      close(false)
    }
    const onScroll = (e: Event): void => {
      if (popupRef.current?.contains(e.target as Node)) return
      close(false)
    }
    const onBlur = (): void => close(false)
    window.addEventListener('mousedown', onDown, true)
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', measure)
    window.addEventListener('blur', onBlur)
    return () => {
      window.removeEventListener('mousedown', onDown, true)
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', measure)
      window.removeEventListener('blur', onBlur)
    }
  }, [open, close, measure])

  // Foco: al buscador si lo hay; si no, a la lista.
  useEffect(() => {
    if (!open || !place) return
    if (searchable) searchRef.current?.focus()
    else listRef.current?.focus()
  }, [open, place, searchable])

  // La opción activa, siempre a la vista.
  useEffect(() => {
    if (!open || active < 0) return
    listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [open, active])

  // Al filtrar, la primera que encaje pasa a ser la activa.
  useEffect(() => {
    if (open && query) setActive(visible.findIndex((o) => !o.disabled))
  }, [query, open, visible])

  const move = (from: number, step: number): number => {
    if (!visible.length) return -1
    let i = from
    for (let n = 0; n < visible.length; n++) {
      i = Math.min(Math.max(i + step, 0), visible.length - 1)
      if (!visible[i].disabled) return i
      if (i === 0 || i === visible.length - 1) break
    }
    return from
  }

  const onListKey = (e: React.KeyboardEvent): void => {
    const page = 8
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setActive((a) => move(a, 1))
        break
      case 'ArrowUp':
        e.preventDefault()
        setActive((a) => move(a, -1))
        break
      case 'PageDown':
        e.preventDefault()
        setActive((a) => move(Math.min(a + page - 1, visible.length - 1), 1))
        break
      case 'PageUp':
        e.preventDefault()
        setActive((a) => move(Math.max(a - page + 1, 0), -1))
        break
      case 'Home':
        if (searchable && e.target === searchRef.current) return
        e.preventDefault()
        setActive(move(-1, 1))
        break
      case 'End':
        if (searchable && e.target === searchRef.current) return
        e.preventDefault()
        setActive(move(visible.length, -1))
        break
      case 'Enter':
        e.preventDefault()
        choose(visible[active])
        break
      case 'Escape':
        e.preventDefault()
        e.stopPropagation()
        close()
        break
      case 'Tab':
        close()
        break
      default:
        // Sin buscador: escribir salta a la primera opción que empiece así.
        if (!searchable && e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
          const now = Date.now()
          const t = typeahead.current
          t.text = now - t.at > 700 ? e.key : t.text + e.key
          t.at = now
          const q = fold(t.text)
          const i = visible.findIndex((o) => !o.disabled && fold(o.label).startsWith(q))
          if (i >= 0) setActive(i)
        }
    }
  }

  const onTriggerKey = (e: React.KeyboardEvent): void => {
    if (disabled) return
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key) || (e.altKey && e.key === 'ArrowDown')) {
      e.preventDefault()
      openList()
    }
  }

  const width = WIDTHS[props.size ?? 'md']
  const shown = current?.label ?? props.placeholder ?? ''

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className={`dropdown${open ? ' open' : ''}${props.className ? ` ${props.className}` : ''}`}
        style={{ width }}
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        disabled={disabled}
        title={shown.length > 28 ? shown : undefined}
        onClick={() => (open ? close() : openList())}
        onKeyDown={onTriggerKey}
      >
        <span className={current ? 'dropdown-value' : 'dropdown-value placeholder'}>{shown}</span>
        <Icon glyph={G.chevronDown} className="dropdown-chevron" />
      </button>
      {open &&
        place &&
        createPortal(
          <div
            ref={popupRef}
            className={`dropdown-popup${place.up ? ' up' : ''}`}
            style={{ left: place.left, width: place.width, top: place.top, bottom: place.bottom, maxHeight: place.maxHeight }}
            onKeyDown={onListKey}
          >
            {searchable && (
              <div className="dropdown-search">
                <Icon glyph={G.search} />
                <input
                  ref={searchRef}
                  className="dropdown-search-input"
                  value={query}
                  placeholder="Buscar…"
                  aria-label={`Buscar en ${label}`}
                  aria-controls={listId}
                  aria-activedescendant={active >= 0 ? `${id}-o${active}` : undefined}
                  onChange={(e) => setQuery(e.target.value)}
                  spellCheck={false}
                />
                <span className="dropdown-count">{visible.length}</span>
              </div>
            )}
            <div
              ref={listRef}
              id={listId}
              className="dropdown-list"
              role="listbox"
              aria-label={label}
              tabIndex={searchable ? -1 : 0}
              aria-activedescendant={!searchable && active >= 0 ? `${id}-o${active}` : undefined}
            >
              {visible.length === 0 && <div className="dropdown-empty">Sin resultados</div>}
              {visible.map((o, i) => (
                <div
                  key={o.value}
                  id={`${id}-o${i}`}
                  data-index={i}
                  role="option"
                  aria-selected={o.value === value}
                  aria-disabled={o.disabled || undefined}
                  className={[
                    'dropdown-option',
                    o.value === value ? 'selected' : '',
                    i === active ? 'active' : '',
                    o.disabled ? 'disabled' : ''
                  ].join(' ')}
                  onMouseMove={() => i !== active && !o.disabled && setActive(i)}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => choose(o)}
                >
                  <span className="dropdown-option-text">
                    <span className="dropdown-option-label">{o.label}</span>
                    {o.hint && <span className="dropdown-option-hint">{o.hint}</span>}
                  </span>
                </div>
              ))}
            </div>
          </div>,
          document.body
        )}
    </>
  )
}
