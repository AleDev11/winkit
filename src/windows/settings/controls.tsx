import { useId } from 'react'
import { Icon } from '../../components/Icon'

/** Tarjeta de ajuste de Windows 11: icono, título y descripción a la izquierda; el control a la derecha. */
export function Card(props: {
  glyph?: string
  title: React.ReactNode
  description?: React.ReactNode
  children?: React.ReactNode
  /** Contenido debajo de la cabecera (listas, campos largos…). */
  below?: React.ReactNode
  className?: string
  htmlFor?: string
}): React.JSX.Element {
  const head = (
    <>
      {props.glyph && <Icon glyph={props.glyph} size={20} className="card-icon" />}
      <span className="card-text">
        <span className="card-title">{props.title}</span>
        {props.description && <span className="card-desc">{props.description}</span>}
      </span>
      {props.children && <span className="card-control">{props.children}</span>}
    </>
  )
  return (
    <div className={['card', props.className ?? ''].join(' ')}>
      {props.htmlFor ? (
        <label className="card-head" htmlFor={props.htmlFor}>
          {head}
        </label>
      ) : (
        <div className="card-head">{head}</div>
      )}
      {props.below && <div className="card-below">{props.below}</div>}
    </div>
  )
}

export function ToggleCard(props: {
  glyph?: string
  title: string
  description?: string
  checked: boolean
  onChange: (v: boolean) => void
}): React.JSX.Element {
  const id = useId()
  return (
    <Card glyph={props.glyph} title={props.title} description={props.description} htmlFor={id} className="clickable">
      <span className="toggle-state">{props.checked ? 'Activado' : 'Desactivado'}</span>
      <input id={id} type="checkbox" className="switch" checked={props.checked} onChange={(e) => props.onChange(e.target.checked)} />
    </Card>
  )
}

export function Select<T extends string>(props: {
  value: T
  options: { value: T; label: string }[]
  onChange: (v: T) => void
  label: string
}): React.JSX.Element {
  return (
    <span className="select-wrap">
      <select className="select" value={props.value} aria-label={props.label} onChange={(e) => props.onChange(e.target.value as T)}>
        {props.options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </span>
  )
}

/** Lista de opciones excluyentes dentro de una tarjeta. */
export function RadioList<T extends string>(props: {
  name: string
  value: T
  options: { value: T; label: React.ReactNode; hint?: React.ReactNode }[]
  onChange: (v: T) => void
}): React.JSX.Element {
  return (
    <div className="radio-list" role="radiogroup">
      {props.options.map((o) => (
        <label key={o.value} className={o.value === props.value ? 'radio-row checked' : 'radio-row'}>
          <input type="radio" className="radio" name={props.name} checked={o.value === props.value} onChange={() => props.onChange(o.value)} />
          <span className="radio-text">
            <span>{o.label}</span>
            {o.hint && <span className="radio-hint">{o.hint}</span>}
          </span>
        </label>
      ))}
    </div>
  )
}

export function Group({ title, children }: { title?: string; children: React.ReactNode }): React.JSX.Element {
  return (
    <section className="group">
      {title && <h2 className="group-title">{title}</h2>}
      <div className="group-cards">{children}</div>
    </section>
  )
}
