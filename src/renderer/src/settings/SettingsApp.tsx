import {
  MODELS,
  SEARCH_ENGINES,
  type CustomAction,
  type KeyTestResult,
  type SearchEngine,
  type Settings,
  type Shortcuts,
  type ShortcutStatus
} from '@shared/types'
import { Check, KeyRound, Plus, Trash2, TriangleAlert, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Logo } from '../Logo'
import { prettyAccelerator, ShortcutInput } from './ShortcutInput'

const api = window.winkit.settings

const SHORTCUT_ROWS: { key: keyof Shortcuts; label: string; hint: string }[] = [
  { key: 'launcher', label: 'Buscador', hint: 'Apps, calculadora, búsqueda web y preguntas a Claude.' },
  { key: 'menu', label: 'Menú de escritura', hint: 'Elige qué hacer con el texto seleccionado.' },
  { key: 'correct', label: 'Corregir directamente', hint: 'Sin menú: corrige la selección.' },
  { key: 'translate', label: 'Traducir lo que escribes', hint: 'Sin menú: traduce la selección y la reemplaza.' },
  { key: 'reader', label: 'Lector', hint: 'Abre lo seleccionado con su traducción al lado, sin tocarlo.' }
]

export function SettingsApp(): React.JSX.Element | null {
  const [s, setS] = useState<Settings | null>(null)
  const [hasKey, setHasKey] = useState(false)
  const [status, setStatus] = useState<ShortcutStatus[]>([])
  const [version, setVersion] = useState('')

  useEffect(() => {
    api.get().then((r) => {
      setS(r.settings)
      setHasKey(r.hasKey)
      setStatus(r.shortcuts)
      setVersion(r.version)
    })
  }, [])

  const save = useCallback(async (patch: Partial<Settings>) => {
    setS((prev) => (prev ? { ...prev, ...patch } : prev))
    const r = await api.save(patch)
    setS(r.settings)
    setStatus(r.shortcuts)
  }, [])

  if (!s) return null

  return (
    <div className="page">
      <header className="top">
        <Logo size={40} />
        <div>
          <h1>Winkit</h1>
          <p>Herramientas para Windows con Claude: escribe mejor, entiende otros idiomas y encuentra lo que buscas.</p>
        </div>
      </header>

      <ApiKeySection hasKey={hasKey} onSaved={() => setHasKey(true)} />

      <section>
        <h2>Cómo se usa</h2>
        <ul className="howto">
          <li>
            <kbd>{prettyAccelerator(s.shortcuts.launcher)}</kbd> abre el buscador: escribe el nombre de una app, una cuenta, algo
            que buscar en la web o una pregunta para Claude.
          </li>
          <li>
            Para escribir: selecciona tu texto y pulsa <kbd>{prettyAccelerator(s.shortcuts.correct)}</kbd> para corregirlo,{' '}
            <kbd>{prettyAccelerator(s.shortcuts.translate)}</kbd> para traducirlo o <kbd>{prettyAccelerator(s.shortcuts.menu)}</kbd>{' '}
            para ver todas las opciones.
          </li>
          <li>
            Para leer: selecciona lo que te han escrito y pulsa <kbd>{prettyAccelerator(s.shortcuts.reader)}</kbd>. Se abre con la
            traducción al lado.
          </li>
        </ul>
      </section>

      <section>
        <h2>Modelo</h2>
        <div className="models">
          {MODELS.map((m) => (
            <label key={m.id} className={s.model === m.id ? 'model active' : 'model'}>
              <input type="radio" name="model" checked={s.model === m.id} onChange={() => save({ model: m.id })} />
              <span className="model-name">{m.label}</span>
              <span className="model-hint">{m.hint}</span>
            </label>
          ))}
        </div>
      </section>

      <section>
        <h2>Atajos de teclado</h2>
        <div className="rows">
          {SHORTCUT_ROWS.map((row) => {
            const failed = status.find((x) => x.key === row.key)?.ok === false
            return (
              <div className="row" key={row.key}>
                <div className="row-text">
                  <span className="row-label">{row.label}</span>
                  <span className="row-hint">
                    {failed ? (
                      <span className="bad">
                        <TriangleAlert size={12} /> Otra aplicación ya usa esta combinación. Elige otra.
                      </span>
                    ) : (
                      row.hint
                    )}
                  </span>
                </div>
                <ShortcutInput
                  value={s.shortcuts[row.key]}
                  onChange={(accel) => save({ shortcuts: { ...s.shortcuts, [row.key]: accel } })}
                />
              </div>
            )
          })}
          <Toggle
            label="Reemplazar sin preguntar con los atajos directos"
            hint="Si lo desactivas, verás el resultado y lo confirmarás con Enter."
            checked={s.directAutoReplace}
            onChange={(v) => save({ directAutoReplace: v })}
          />
        </div>
      </section>

      <section>
        <h2>Traducción</h2>
        <div className="rows">
          <div className="row">
            <div className="row-text">
              <span className="row-label">Idioma principal</span>
              <span className="row-hint">El de «Traducir directamente» y el primero del menú.</span>
            </div>
            <select value={s.translateTarget} onChange={(e) => save({ translateTarget: e.target.value })}>
              {s.languages.map((l) => (
                <option key={l}>{l}</option>
              ))}
            </select>
          </div>
          <div className="row">
            <div className="row-text">
              <span className="row-label">Si ya está en ese idioma, traducir a</span>
              <span className="row-hint">Así el mismo atajo sirve para ida y vuelta.</span>
            </div>
            <select value={s.translateFallback} onChange={(e) => save({ translateFallback: e.target.value })}>
              {s.languages
                .filter((l) => l !== s.translateTarget)
                .map((l) => (
                  <option key={l}>{l}</option>
                ))}
            </select>
          </div>
          <div className="row">
            <div className="row-text">
              <span className="row-label">El lector traduce al</span>
              <span className="row-hint">El idioma en el que quieres leer lo que te escriben.</span>
            </div>
            <select value={s.readerLanguage} onChange={(e) => save({ readerLanguage: e.target.value })}>
              {s.languages.map((l) => (
                <option key={l}>{l}</option>
              ))}
            </select>
          </div>
          <LanguageList
            languages={s.languages}
            protectedLangs={[s.translateTarget, s.translateFallback, s.readerLanguage]}
            onChange={(languages) => save({ languages })}
          />
        </div>
      </section>

      <section>
        <h2>Buscador</h2>
        <div className="rows">
          <div className="row">
            <div className="row-text">
              <span className="row-label">Buscar en la web con</span>
              <span className="row-hint">Se abre en tu navegador predeterminado.</span>
            </div>
            <select value={s.searchEngine} onChange={(e) => save({ searchEngine: e.target.value as SearchEngine })}>
              {(Object.keys(SEARCH_ENGINES) as SearchEngine[]).map((k) => (
                <option key={k} value={k}>
                  {SEARCH_ENGINES[k].label}
                </option>
              ))}
            </select>
          </div>
        </div>
      </section>

      <section>
        <h2>Acciones personalizadas</h2>
        <p className="section-hint">Aparecen en el menú. Escribe la instrucción como se la darías a una persona.</p>
        <CustomActions actions={s.customActions} onChange={(customActions) => save({ customActions })} />
      </section>

      <section>
        <h2>General</h2>
        <div className="rows">
          <Toggle
            label="Si no hay nada seleccionado, usar todo el campo"
            hint="Winkit pulsa Ctrl+A por ti. Desactívalo si alguna app selecciona de más."
            checked={s.selectAllFallback}
            onChange={(v) => save({ selectAllFallback: v })}
          />
          <Toggle
            label="Arrancar con Windows"
            hint="Winkit se queda en la bandeja del sistema, junto al reloj."
            checked={s.launchAtLogin}
            onChange={(v) => save({ launchAtLogin: v })}
          />
        </div>
      </section>

      <footer className="bottom">Winkit {version}</footer>
    </div>
  )
}

function ApiKeySection({ hasKey, onSaved }: { hasKey: boolean; onSaved: () => void }): React.JSX.Element {
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<KeyTestResult | null>(null)

  const submit = async (): Promise<void> => {
    if (!value.trim()) return
    setBusy(true)
    const r = await api.saveKey(value)
    setBusy(false)
    setResult(r)
    if (r.ok) {
      setValue('')
      onSaved()
    }
  }

  const test = async (): Promise<void> => {
    setBusy(true)
    setResult(await api.testKey())
    setBusy(false)
  }

  return (
    <section className={hasKey ? '' : 'attention'}>
      <h2>API key de Anthropic</h2>
      <p className="section-hint">
        {hasKey ? (
          <span className="good">
            <Check size={13} /> Guardada y cifrada en este ordenador.
          </span>
        ) : (
          'La necesitas para que Winkit funcione. Créala en console.anthropic.com → API keys.'
        )}
      </p>
      <div className="key-row">
        <div className="input-icon">
          <KeyRound size={14} />
          <input
            type="password"
            placeholder={hasKey ? 'Pega otra para reemplazarla' : 'sk-ant-…'}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && submit()}
            spellCheck={false}
          />
        </div>
        <button className="btn primary" disabled={busy || !value.trim()} onClick={submit}>
          Guardar
        </button>
        {hasKey && (
          <button className="btn" disabled={busy} onClick={test}>
            Probar
          </button>
        )}
      </div>
      {result && <p className={result.ok ? 'result-msg good' : 'result-msg bad'}>{result.message}</p>}
    </section>
  )
}

function Toggle(props: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }): React.JSX.Element {
  return (
    <label className="row toggle-row">
      <div className="row-text">
        <span className="row-label">{props.label}</span>
        {props.hint && <span className="row-hint">{props.hint}</span>}
      </div>
      <input type="checkbox" className="switch" checked={props.checked} onChange={(e) => props.onChange(e.target.checked)} />
    </label>
  )
}

function LanguageList(props: { languages: string[]; protectedLangs: string[]; onChange: (l: string[]) => void }): React.JSX.Element {
  const [draft, setDraft] = useState('')
  const add = (): void => {
    const name = draft.trim()
    if (!name) return
    const capitalized = name.charAt(0).toLocaleUpperCase('es') + name.slice(1)
    if (!props.languages.some((l) => l.toLocaleLowerCase('es') === capitalized.toLocaleLowerCase('es'))) {
      props.onChange([...props.languages, capitalized])
    }
    setDraft('')
  }
  return (
    <div className="row column">
      <div className="row-text">
        <span className="row-label">Idiomas del menú</span>
        <span className="row-hint">Escribe el nombre en español: «Japonés», «Gallego», «Inglés británico»…</span>
      </div>
      <div className="chips">
        {props.languages.map((l) => (
          <span className="lang-chip" key={l}>
            {l}
            {!props.protectedLangs.includes(l) && (
              <button title={`Quitar ${l}`} onClick={() => props.onChange(props.languages.filter((x) => x !== l))}>
                <X size={12} />
              </button>
            )}
          </span>
        ))}
        <input
          className="chip-input"
          placeholder="Añadir idioma"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && add()}
          onBlur={add}
        />
      </div>
    </div>
  )
}

function CustomActions(props: { actions: CustomAction[]; onChange: (a: CustomAction[]) => void }): React.JSX.Element {
  // Se edita en local y se guarda al salir del campo, no en cada tecla.
  const [local, setLocal] = useState(props.actions)
  useEffect(() => setLocal(props.actions), [props.actions])

  const update = (id: string, patch: Partial<CustomAction>): void =>
    setLocal((list) => list.map((a) => (a.id === id ? { ...a, ...patch } : a)))
  const commit = (): void => props.onChange(local)

  return (
    <div className="actions">
      {local.map((a) => (
        <div className="action" key={a.id}>
          <div className="action-head">
            <input
              className="action-name"
              placeholder="Nombre (p. ej. Email a un cliente)"
              value={a.name}
              onChange={(e) => update(a.id, { name: e.target.value })}
              onBlur={commit}
            />
            <button className="icon-btn danger" title="Eliminar" onClick={() => props.onChange(local.filter((x) => x.id !== a.id))}>
              <Trash2 size={14} />
            </button>
          </div>
          <textarea
            rows={2}
            placeholder="Instrucción (p. ej. Conviértelo en un email profesional y cordial para un cliente, con saludo y despedida.)"
            value={a.prompt}
            onChange={(e) => update(a.id, { prompt: e.target.value })}
            onBlur={commit}
          />
        </div>
      ))}
      <button
        className="btn"
        onClick={() => props.onChange([...local, { id: `custom:${crypto.randomUUID()}`, name: '', prompt: '' }])}
      >
        <Plus size={14} /> Nueva acción
      </button>
    </div>
  )
}
