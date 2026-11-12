import { useCallback, useEffect, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { Logo } from '../../components/Logo'
import { ProgressRing } from '../../components/ProgressRing'
import { settings as api, ui, updates } from '../../lib/api'
import {
  MODELS,
  SEARCH_ENGINES,
  type ColorFormat,
  type CustomAction,
  type KeyTestResult,
  type ModelId,
  type NoticePosition,
  type SearchEngine,
  type Shortcuts,
  type UpdateStatus
} from '../../shared/types'
import { acceleratorParts } from './accelerator'
import { Card, Group, RadioList, Select, ToggleCard } from './controls'
import type { PageProps } from './SettingsApp'
import { ShortcutRecorder } from './ShortcutRecorder'

const REPO = 'https://github.com/AleDev11/winkit'
const CONSOLE_KEYS = 'https://console.anthropic.com/settings/keys'

/**
 * Edición local con guardado diferido: los campos de texto no guardan en cada
 * tecla, sino al dejar de escribir un momento o al salir del campo.
 */
function useDraft<T>(value: T, commit: (v: T) => void, delay = 600) {
  const [draft, setDraft] = useState(value)
  const latest = useRef(value)
  const dirty = useRef(false)
  const timer = useRef<number | undefined>(undefined)
  const commitRef = useRef(commit)
  commitRef.current = commit

  useEffect(() => {
    if (!dirty.current) {
      latest.current = value
      setDraft(value)
    }
  }, [value])

  const flush = useCallback(() => {
    window.clearTimeout(timer.current)
    if (!dirty.current) return
    dirty.current = false
    commitRef.current(latest.current)
  }, [])

  const update = useCallback(
    (next: T) => {
      latest.current = next
      dirty.current = true
      setDraft(next)
      window.clearTimeout(timer.current)
      timer.current = window.setTimeout(flush, delay)
    },
    [delay, flush]
  )

  useEffect(() => flush, [flush])

  return [draft, update, flush] as const
}

// ── General ─────────────────────────────────────────────────────────────

const NOTICE_POSITIONS: { value: NoticePosition; label: string }[] = [
  { value: 'bottomRight', label: 'Abajo a la derecha' },
  { value: 'bottomLeft', label: 'Abajo a la izquierda' },
  { value: 'topRight', label: 'Arriba a la derecha' },
  { value: 'topLeft', label: 'Arriba a la izquierda' },
  { value: 'cursor', label: 'Junto al ratón' }
]

export function GeneralPage({ s, state, save, setState }: PageProps): React.JSX.Element {
  return (
    <>
      <Group title="Claude">
        <ApiKeyCard
          hasKey={state.hasKey}
          keyFromEnv={state.keyFromEnv}
          onSaved={() => setState((st) => (st ? { ...st, hasKey: true } : st))}
        />
        <Card
          glyph={G.chip}
          title="Modelo"
          description="El que usan todas las acciones de escritura y las preguntas del buscador."
          below={
            <RadioList<ModelId>
              name="model"
              value={s.model}
              onChange={(model) => save({ model })}
              options={MODELS.map((m) => ({ value: m.id, label: m.label, hint: m.hint }))}
            />
          }
        />
      </Group>

      <Group title="Al escribir">
        <ToggleCard
          glyph={G.swap}
          title="Reemplazar directamente con los atajos"
          description="Corregir y Traducir pegan el resultado en cuanto Claude termina, sin enseñártelo antes."
          checked={s.directAutoReplace}
          onChange={(directAutoReplace) => save({ directAutoReplace })}
        />
        <ToggleCard
          glyph={G.selectAll}
          title="Si no hay nada seleccionado, usar todo el campo"
          description="Winkit pulsa Ctrl+A por ti antes de copiar. Desactívalo si alguna app selecciona de más."
          checked={s.selectAllFallback}
          onChange={(selectAllFallback) => save({ selectAllFallback })}
        />
      </Group>

      <Group title="Sistema">
        <ToggleCard
          glyph={G.power}
          title="Iniciar con Windows"
          description="Winkit espera en la bandeja del sistema, junto al reloj."
          checked={s.launchAtLogin}
          onChange={(launchAtLogin) => save({ launchAtLogin })}
        />
        <Card
          glyph={G.notification}
          title="Dónde salen los avisos"
          description="Clip guardado, color copiado, «No hay texto seleccionado»… La paleta y el buscador no cambian."
        >
          <Select<NoticePosition>
            label="Dónde salen los avisos"
            value={s.noticePosition}
            options={NOTICE_POSITIONS}
            onChange={(noticePosition) => save({ noticePosition })}
          />
        </Card>
      </Group>
    </>
  )
}

function ApiKeyCard(props: { hasKey: boolean; keyFromEnv: boolean; onSaved: () => void }): React.JSX.Element {
  const { hasKey, keyFromEnv } = props
  const [value, setValue] = useState('')
  const [reveal, setReveal] = useState(false)
  const [busy, setBusy] = useState<'save' | 'test' | null>(null)
  const [result, setResult] = useState<KeyTestResult | null>(null)

  const submit = async (): Promise<void> => {
    if (!value.trim() || busy) return
    setBusy('save')
    setResult(null)
    try {
      const r = await api.saveKey(value.trim())
      setResult(r)
      if (r.ok) {
        setValue('')
        setReveal(false)
        props.onSaved()
      }
    } catch (err) {
      setResult({ ok: false, message: String(err) })
    } finally {
      setBusy(null)
    }
  }

  const test = async (): Promise<void> => {
    if (busy) return
    setBusy('test')
    setResult(null)
    try {
      setResult(await api.testKey())
    } catch (err) {
      setResult({ ok: false, message: String(err) })
    } finally {
      setBusy(null)
    }
  }

  const status = keyFromEnv ? (
    <span className="status good">
      <Icon glyph={G.checkCircle} size={12} /> Se usa la variable de entorno ANTHROPIC_API_KEY
    </span>
  ) : hasKey ? (
    <span className="status good">
      <Icon glyph={G.checkCircle} size={12} /> Guardada y cifrada en este equipo
    </span>
  ) : (
    <span className="status caution">
      <Icon glyph={G.warning} size={12} /> Hace falta para usar Winkit
    </span>
  )

  return (
    <Card
      glyph={G.key}
      title="API key de Anthropic"
      description={status}
      className={!hasKey && !keyFromEnv ? 'attention' : undefined}
      below={
        <div className="key">
          {keyFromEnv ? (
            <p className="card-note">
              Para cambiarla, cambia la variable de entorno y reinicia Winkit. Mientras exista, la key guardada aquí no se usa.
            </p>
          ) : (
            <>
              <div className="key-row">
                <div className="password">
                  <input
                    className="input"
                    type={reveal ? 'text' : 'password'}
                    placeholder={hasKey ? 'Pega otra para reemplazarla' : 'sk-ant-…'}
                    value={value}
                    autoComplete="off"
                    spellCheck={false}
                    onChange={(e) => setValue(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && void submit()}
                    aria-label="API key"
                  />
                  {value && (
                    <button
                      className="btn subtle sm icon-only password-reveal"
                      onClick={() => setReveal((r) => !r)}
                      title={reveal ? 'Ocultar' : 'Mostrar'}
                      aria-label={reveal ? 'Ocultar la key' : 'Mostrar la key'}
                    >
                      <Icon glyph={reveal ? '' : ''} size={14} />
                    </button>
                  )}
                </div>
                <button className="btn accent" disabled={!value.trim() || busy !== null} onClick={() => void submit()}>
                  {busy === 'save' ? <ProgressRing size={14} /> : null}
                  Guardar
                </button>
                <button className="btn" disabled={(!hasKey && !keyFromEnv) || busy !== null} onClick={() => void test()}>
                  {busy === 'test' ? <ProgressRing size={14} /> : null}
                  Probar
                </button>
              </div>
              {!hasKey && (
                <p className="card-note">
                  Créala en{' '}
                  <a
                    href={CONSOLE_KEYS}
                    onClick={(e) => {
                      e.preventDefault()
                      void ui.openUrl(CONSOLE_KEYS)
                    }}
                  >
                    console.anthropic.com
                  </a>{' '}
                  → API keys. Se comprueba antes de guardarla y se cifra con tu cuenta de Windows.
                </p>
              )}
            </>
          )}
          {keyFromEnv && (
            <div className="key-row">
              <button className="btn" disabled={busy !== null} onClick={() => void test()}>
                {busy === 'test' ? <ProgressRing size={14} /> : null}
                Probar
              </button>
            </div>
          )}
          {result && (
            <div className={result.ok ? 'infobar success fade-in' : 'infobar error fade-in'} role="status">
              <Icon glyph={result.ok ? G.checkCircle : G.error} />
              <div className="infobar-body">{result.message}</div>
            </div>
          )}
        </div>
      }
    />
  )
}

// ── Atajos ──────────────────────────────────────────────────────────────

const SHORTCUT_ROWS: { key: keyof Shortcuts; label: string; hint: string; glyph: string }[] = [
  { key: 'palette', label: 'Paleta de escritura', hint: 'Todas las acciones sobre el texto seleccionado.', glyph: G.edit },
  { key: 'correct', label: 'Corregir', hint: 'Corrige la selección sin pasar por la paleta.', glyph: G.spelling },
  { key: 'translate', label: 'Traducir', hint: 'Traduce la selección. El idioma lo decide la detección.', glyph: G.translate },
  { key: 'understand', label: 'Entender', hint: 'Abre lo seleccionado con su traducción al lado, sin tocarlo.', glyph: G.book },
  { key: 'launcher', label: 'Buscador', hint: 'Apps, cuentas, unidades, comandos, la web y Claude.', glyph: G.search },
  { key: 'colorPicker', label: 'Selector de color', hint: 'Copia el color de cualquier punto de la pantalla.', glyph: G.eyedropper },
  {
    key: 'saveClip',
    label: 'Guardar clip',
    hint: 'Guarda los últimos minutos de pantalla (actívalo en Clips).',
    glyph: G.record
  }
]

export function ShortcutsPage({ s, state, save, setShortcutStatus }: PageProps): React.JSX.Element {
  const onRecordingChange = useCallback(
    (recording: boolean) => {
      if (recording) void api.suspendShortcuts().catch(() => {})
      else
        void api
          .resumeShortcuts()
          .then(setShortcutStatus)
          .catch(() => {})
    },
    [setShortcutStatus]
  )

  return (
    <>
      <p className="page-intro">
        Pulsa un atajo y escribe la combinación nueva: Ctrl, Alt o Win con una letra, un número o una tecla F. Esc cancela.
      </p>
      <Group>
        {SHORTCUT_ROWS.map((row) => {
          const value = s.shortcuts[row.key]
          const failed = !!value && state.shortcuts.find((x) => x.key === row.key)?.ok === false
          const twin = value
            ? SHORTCUT_ROWS.find((o) => o.key !== row.key && s.shortcuts[o.key].toLowerCase() === value.toLowerCase())
            : undefined
          const description = failed ? (
            <span className="status bad">
              <Icon glyph={G.warning} size={12} /> Otra app ya usa este atajo
            </span>
          ) : twin ? (
            <span className="status caution">
              <Icon glyph={G.warning} size={12} /> También lo usa «{twin.label}»
            </span>
          ) : (
            row.hint
          )
          return (
            <Card key={row.key} glyph={row.glyph} title={row.label} description={description} className={failed ? 'invalid' : undefined}>
              <ShortcutRecorder
                value={value}
                label={row.label}
                invalid={failed || !!twin}
                onChange={(accel) => save({ shortcuts: { ...s.shortcuts, [row.key]: accel } })}
                onRecordingChange={onRecordingChange}
              />
            </Card>
          )
        })}
      </Group>
    </>
  )
}

// ── Idiomas ─────────────────────────────────────────────────────────────

export function LanguagesPage({ s, save }: PageProps): React.JSX.Element {
  const [draft, setDraft] = useState('')
  const protectedLangs = [s.myLanguage, s.translateTarget]
  const options = s.languages.map((l) => ({ value: l, label: l }))
  const withCurrent = (v: string) => (s.languages.includes(v) ? options : [{ value: v, label: v }, ...options])

  const add = (): void => {
    const name = draft.trim().replace(/\s+/g, ' ')
    if (!name) return
    const nice = name.charAt(0).toLocaleUpperCase('es') + name.slice(1)
    if (!s.languages.some((l) => l.toLocaleLowerCase('es') === nice.toLocaleLowerCase('es'))) {
      save({ languages: [...s.languages, nice] })
    }
    setDraft('')
  }

  return (
    <>
      <Group>
        <Card glyph={G.globe} title="Mi idioma" description="Al que se traduce lo que te escriben, y lo que ya está en tu idioma de destino.">
          <Select label="Mi idioma" value={s.myLanguage} options={withCurrent(s.myLanguage)} onChange={(myLanguage) => save({ myLanguage })} />
        </Card>
        <Card glyph={G.translate} title="Traducir lo que escribo a" description="El destino de «Traducir». Si el texto ya está en ese idioma, se traduce al tuyo.">
          <Select
            label="Traducir lo que escribo a"
            value={s.translateTarget}
            options={withCurrent(s.translateTarget)}
            onChange={(translateTarget) => save({ translateTarget })}
          />
        </Card>
      </Group>

      <Group title="Idiomas de destino">
        <Card
          glyph={G.language}
          title="Idiomas disponibles"
          description="Los que aparecen al traducir y en «Entender». Escríbelos en español: «Japonés», «Gallego», «Inglés británico»…"
          below={
            <div className="langs">
              <ul className="lang-chips">
                {s.languages.map((l) => {
                  const locked = protectedLangs.includes(l)
                  return (
                    <li key={l} className="lang-chip">
                      <span>{l}</span>
                      {locked ? (
                        <span className="lang-lock" title="Está en uso arriba">
                          <Icon glyph={G.check} size={10} />
                        </span>
                      ) : (
                        <button
                          className="lang-remove"
                          title={`Quitar ${l}`}
                          aria-label={`Quitar ${l}`}
                          onClick={() => save({ languages: s.languages.filter((x) => x !== l) })}
                        >
                          <Icon glyph={G.cancel} size={9} />
                        </button>
                      )}
                    </li>
                  )
                })}
              </ul>
              <div className="lang-add">
                <input
                  className="input"
                  placeholder="Añadir idioma"
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && add()}
                  aria-label="Nuevo idioma"
                />
                <button className="btn" disabled={!draft.trim()} onClick={add}>
                  <Icon glyph={G.add} /> Añadir
                </button>
              </div>
            </div>
          }
        />
      </Group>
    </>
  )
}

// ── Acciones ────────────────────────────────────────────────────────────

export function ActionsPage({ s, save }: PageProps): React.JSX.Element {
  const commit = useCallback((customActions: CustomAction[]) => save({ customActions }), [save])
  const [actions, setActions, flush] = useDraft(s.customActions, commit)
  const [focusId, setFocusId] = useState<string | null>(null)

  const update = (id: string, patch: Partial<CustomAction>): void =>
    setActions(actions.map((a) => (a.id === id ? { ...a, ...patch } : a)))

  const addAction = (): void => {
    flush()
    const id = `custom:${crypto.randomUUID()}`
    const next = [...actions, { id, name: '', prompt: '' }]
    setFocusId(id)
    save({ customActions: next })
  }

  const remove = (id: string): void => {
    flush()
    save({ customActions: actions.filter((a) => a.id !== id) })
  }

  return (
    <>
      <p className="page-intro">
        Aparecen en la paleta después de las integradas. Escribe la instrucción como se la darías a una persona: Claude la aplica
        al texto seleccionado.
      </p>
      {actions.length === 0 ? (
        <div className="empty card">
          <Icon glyph={G.lightning} size={28} className="empty-icon" />
          <span className="body-strong">Aún no tienes acciones propias</span>
          <span className="muted caption">Por ejemplo: «Email a un cliente», «Lista de tareas», «Tono de LinkedIn».</span>
          <button className="btn accent" onClick={addAction}>
            <Icon glyph={G.add} /> Nueva acción
          </button>
        </div>
      ) : (
        <>
          <Group>
            {actions.map((a) => (
              <div className="card action" key={a.id}>
                <div className="action-head">
                  <Icon glyph={G.lightning} size={20} className="card-icon" />
                  <input
                    className="input action-name"
                    placeholder="Nombre (p. ej. Email a un cliente)"
                    value={a.name}
                    autoFocus={a.id === focusId}
                    onChange={(e) => update(a.id, { name: e.target.value })}
                    onBlur={flush}
                    aria-label="Nombre de la acción"
                  />
                  <button className="btn subtle icon-only" onClick={() => remove(a.id)} title="Eliminar" aria-label="Eliminar la acción">
                    <Icon glyph={G.delete} />
                  </button>
                </div>
                <textarea
                  className="input action-prompt"
                  rows={3}
                  placeholder="Instrucción (p. ej. Conviértelo en un email profesional y cordial para un cliente, con saludo y despedida.)"
                  value={a.prompt}
                  onChange={(e) => update(a.id, { prompt: e.target.value })}
                  onBlur={flush}
                  aria-label="Instrucción"
                />
              </div>
            ))}
          </Group>
          <button className="btn add-action" onClick={addAction}>
            <Icon glyph={G.add} /> Nueva acción
          </button>
        </>
      )}
    </>
  )
}

// ── Buscador ────────────────────────────────────────────────────────────

export function SearchPage({ s, save }: PageProps): React.JSX.Element {
  return (
    <>
      <Group>
        <Card
          glyph={G.search}
          title="Buscar en la web con"
          description="Lo que no sea una app, una cuenta o un comando se puede buscar aquí. Se abre en tu navegador predeterminado."
          below={
            <RadioList<SearchEngine>
              name="engine"
              value={s.searchEngine}
              onChange={(searchEngine) => save({ searchEngine })}
              options={(Object.keys(SEARCH_ENGINES) as SearchEngine[]).map((k) => ({ value: k, label: SEARCH_ENGINES[k].label }))}
            />
          }
        />
      </Group>
      <Group title="Qué entiende el buscador">
        <Card glyph={G.app} title="Aplicaciones" description="Escribe parte del nombre o sus iniciales: «vsc» encuentra Visual Studio Code." />
        <Card glyph={G.calculator} title="Calculadora" description="«(12 + 4) × 3», «340 + 15%», «20% de 340», «raíz(2)». Enter copia el resultado." />
        <Card glyph={G.ruler} title="Unidades" description="«10 km a mi», «5 kg en lb», «100 f a c», «1,5 h en min», «2 gb a mb»." />
        <Card glyph={G.sparkle} title="Claude" description="Ctrl+Enter pregunta lo que hayas escrito. La respuesta aparece en el mismo buscador." />
      </Group>
    </>
  )
}

// ── Color ───────────────────────────────────────────────────────────────

const SAMPLE = { r: 224, g: 120, b: 70 }

function toHsl({ r, g, b }: typeof SAMPLE): [number, number, number] {
  const rn = r / 255
  const gn = g / 255
  const bn = b / 255
  const max = Math.max(rn, gn, bn)
  const min = Math.min(rn, gn, bn)
  const l = (max + min) / 2
  let h = 0
  let s = 0
  if (max !== min) {
    const d = max - min
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
    if (max === rn) h = (gn - bn) / d + (gn < bn ? 6 : 0)
    else if (max === gn) h = (bn - rn) / d + 2
    else h = (rn - gn) / d + 4
    h *= 60
  }
  return [Math.round(h), Math.round(s * 100), Math.round(l * 100)]
}

export function formatColor(c: typeof SAMPLE, format: ColorFormat): string {
  if (format === 'rgb') return `rgb(${c.r}, ${c.g}, ${c.b})`
  if (format === 'hsl') {
    const [h, s, l] = toHsl(c)
    return `hsl(${h}, ${s}%, ${l}%)`
  }
  // Igual que lo que copia el selector (src-tauri/src/picker.rs): en mayúsculas.
  return `#${[c.r, c.g, c.b].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`
}

const FORMATS: { value: ColorFormat; label: string }[] = [
  { value: 'hex', label: 'Hexadecimal' },
  { value: 'rgb', label: 'RGB' },
  { value: 'hsl', label: 'HSL' }
]

export function ColorPage({ s, save }: PageProps): React.JSX.Element {
  const swatch = `rgb(${SAMPLE.r}, ${SAMPLE.g}, ${SAMPLE.b})`
  const keys = acceleratorParts(s.shortcuts.colorPicker)
  return (
    <>
      <Group>
        <Card
          glyph={G.eyedropper}
          title="Selector de color"
          description={
            keys.length ? (
              <>
                Púlsalo con{' '}
                {keys.map((k, i) => (
                  <kbd key={i} className="inline-kbd">
                    {k}
                  </kbd>
                ))}{' '}
                y haz clic en cualquier punto de la pantalla. Las flechas mueven un píxel; Esc cancela.
              </>
            ) : (
              'No tiene atajo. Puedes ponerle uno en Atajos o abrirlo desde el buscador con «color».'
            )
          }
        />
        <Card
          glyph={G.color}
          title="Formato al copiar"
          description="Cómo se copia el color elegido al portapapeles."
          below={
            <RadioList<ColorFormat>
              name="color-format"
              value={s.colorFormat}
              onChange={(colorFormat) => save({ colorFormat })}
              options={FORMATS.map((f) => ({
                value: f.value,
                label: f.label,
                hint: (
                  <span className="color-example">
                    <span className="color-dot" style={{ background: swatch }} />
                    <code>{formatColor(SAMPLE, f.value)}</code>
                  </span>
                )
              }))}
            />
          }
        />
      </Group>
    </>
  )
}

// ── Acerca de ───────────────────────────────────────────────────────────

function updateText(s: UpdateStatus): string {
  switch (s.state) {
    case 'checking':
      return 'Buscando actualizaciones…'
    case 'downloading':
      return `Descargando la versión ${s.version}${s.progress != null ? ` · ${Math.round(s.progress * 100)} %` : '…'}`
    case 'ready':
      return `La versión ${s.version} está lista. Se instalará al reiniciar Winkit.`
    case 'upToDate':
      return 'Tienes la última versión.'
    case 'error':
      return s.message ?? 'No se ha podido comprobar.'
    default:
      return 'Winkit se actualiza solo desde GitHub.'
  }
}

function UpdateCard(): React.JSX.Element {
  const [status, setStatus] = useState<UpdateStatus | null>(null)
  useEffect(() => {
    void updates.status().then(setStatus)
    return updates.onStatus(setStatus)
  }, [])
  const busy = status?.state === 'checking' || status?.state === 'downloading'
  return (
    <Card glyph={G.refresh} title="Actualizaciones" description={status ? updateText(status) : ' '}>
      {status?.state === 'ready' ? (
        <button className="btn accent" onClick={() => void updates.restart()}>
          Reiniciar y actualizar
        </button>
      ) : (
        <button className="btn" disabled={busy} onClick={() => void updates.check().then(setStatus)}>
          {busy ? <ProgressRing size={16} /> : 'Buscar ahora'}
        </button>
      )}
    </Card>
  )
}

export function AboutPage({ state }: PageProps): React.JSX.Element {
  const open = (url: string) => (e: React.MouseEvent) => {
    e.preventDefault()
    void ui.openUrl(url)
  }
  return (
    <>
      <div className="card about">
        <Logo size={64} />
        <div className="about-text">
          <span className="subtitle">Winkit</span>
          <span className="muted">Versión {state.version}</span>
          <span className="about-desc">Herramientas para Windows con Claude: escribe mejor, entiende otros idiomas y encuentra lo que buscas.</span>
        </div>
      </div>
      <Group>
        <UpdateCard />
        <Card glyph={G.openExternal} title="Código fuente" description="github.com/AleDev11/winkit">
          <a className="btn" href={REPO} onClick={open(REPO)}>
            Abrir en GitHub
          </a>
        </Card>
        <Card glyph={G.message} title="Informar de un problema" description="Cuéntanos qué ha pasado y cómo reproducirlo.">
          <a className="btn" href={`${REPO}/issues`} onClick={open(`${REPO}/issues`)}>
            Abrir una incidencia
          </a>
        </Card>
        <Card glyph={G.info} title="Licencia" description="MIT · © Alejandro Font. Winkit usa la API de Claude, de Anthropic." />
      </Group>
    </>
  )
}
