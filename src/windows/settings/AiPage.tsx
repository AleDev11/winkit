import { useCallback, useEffect, useId, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { ProgressRing } from '../../components/ProgressRing'
import { ai as aiApi, localAi, settings as settingsApi, ui } from '../../lib/api'
import type {
  Ai,
  AiTestResult,
  AiTool,
  KeyTestResult,
  LocalAi,
  LocalAiStatus,
  LocalModel,
  ModelInfo,
  ModelRef,
  ProviderId,
  ProviderInfo
} from '../../shared/types'
import { Card, Group, Select } from './controls'
import type { PageProps } from './SettingsApp'

/** Proveedores en la nube, con su nota de coste. */
const CLOUD: { id: ProviderId; glyph: string; cost: string; placeholder: string }[] = [
  { id: 'anthropic', glyph: G.sparkle, cost: 'De pago por uso en tu cuenta de Anthropic.', placeholder: 'sk-ant-…' },
  { id: 'openai', glyph: G.globe, cost: 'De pago por uso en tu cuenta de OpenAI.', placeholder: 'sk-…' },
  { id: 'deepseek', glyph: G.globe, cost: 'De pago por uso; de los más baratos.', placeholder: 'sk-…' },
  { id: 'gemini', glyph: G.globe, cost: 'De pago por uso; Google tiene un nivel gratuito con límites.', placeholder: 'AIza…' },
  {
    id: 'openrouter',
    glyph: G.globe,
    cost: 'Cientos de modelos con una sola key. Se paga por uso; algunos son gratis.',
    placeholder: 'sk-or-…'
  }
]

const TOOLS: { id: AiTool; glyph: string; title: string; description: string }[] = [
  { id: 'writing', glyph: G.edit, title: 'Escribir', description: 'La paleta y los atajos de corregir y traducir.' },
  { id: 'understand', glyph: G.book, title: 'Entender', description: 'La traducción para leer lo que te han escrito.' },
  { id: 'launcher', glyph: G.search, title: 'Buscador', description: 'Las preguntas que haces con Ctrl+Enter en el buscador.' },
  { id: 'assistant', glyph: G.message, title: 'Asistente', description: 'El asistente de escritorio (llegará más adelante).' }
]

const CONTEXTS = [2048, 4096, 8192, 16384, 32768]
const UNLOAD = [1, 2, 5, 10, 15, 30, 60, 0]

const gb = (mb: number): string => (mb / 1000).toLocaleString('es-ES', { maximumFractionDigits: 1, minimumFractionDigits: 1 })

/** La lista de proveedores, con un `reload` para después de guardar una key. */
function useProviders(): [ProviderInfo[] | null, () => void] {
  const [list, setList] = useState<ProviderInfo[] | null>(null)
  const reload = useCallback(() => {
    void aiApi
      .providers()
      .then(setList)
      .catch(() => {})
  }, [])
  useEffect(reload, [reload])
  return [list, reload]
}

/** Modelos de un proveedor (en vivo). `null` mientras carga. */
function useModels(provider: ProviderId | null, ready: boolean, version: number): { models: ModelInfo[] | null; error: string | null } {
  const [state, setState] = useState<{ key: string; models: ModelInfo[] | null; error: string | null }>({ key: '', models: null, error: null })
  const key = `${provider}:${ready}:${version}`
  useEffect(() => {
    if (!provider || !ready) return
    let alive = true
    aiApi
      .models(provider)
      .then((models) => alive && setState({ key, models, error: null }))
      .catch((err: unknown) => alive && setState({ key, models: [], error: String(err) }))
    return () => {
      alive = false
    }
  }, [key, provider, ready])
  if (state.key !== key) return { models: null, error: null }
  return { models: state.models, error: state.error }
}

function useLocalStatus(): LocalAiStatus | null {
  const [status, setStatus] = useState<LocalAiStatus | null>(null)
  useEffect(() => {
    let alive = true
    void localAi.status().then((s) => alive && setStatus(s))
    const off = localAi.onStatus(setStatus)
    return () => {
      alive = false
      off()
    }
  }, [])
  return status
}

export function AiPage({ s, save, setState }: PageProps): React.JSX.Element {
  // Siempre sobre los ajustes más recientes (varios cambios seguidos no se pisan).
  const latest = useRef(s.ai)
  latest.current = s.ai
  const setAi = useCallback((patch: Partial<Ai>) => save({ ai: { ...latest.current, ...patch } }), [save])
  const [providers, reloadProviders] = useProviders()
  // Sube al guardar o quitar una key: las listas de modelos se vuelven a pedir.
  const [version, setVersion] = useState(0)

  /** Tras cambios hechos por el backend (instalar un modelo local…), recarga todo. */
  const refresh = useCallback(() => {
    reloadProviders()
    setVersion((v) => v + 1)
    void settingsApi
      .get()
      .then((st) => setState(st))
      .catch(() => {})
  }, [reloadProviders, setState])

  // La marca de «falta configurar» del menú sigue al modelo de «Escribir».
  const writing = s.ai.tools.writing?.provider ? s.ai.tools.writing : s.ai.default
  useEffect(() => {
    if (!providers) return
    const ok = providers.find((p) => p.id === writing.provider)?.ready ?? false
    setState((st) => (st && st.aiReady !== ok ? { ...st, aiReady: ok } : st))
  }, [providers, writing.provider, setState])

  const ready = (p: ProviderId): boolean => providers?.find((x) => x.id === p)?.ready ?? false
  const name = (p: ProviderId): string => providers?.find((x) => x.id === p)?.name ?? p

  return (
    <>
      <p className="page-intro">
        Elige quién hace el trabajo: un servicio en la nube con tu propia API key (de pago por uso) o la IA local, gratis y sin que tus
        textos salgan del PC.
      </p>

      <Group title="Proveedores">
        {CLOUD.map((c) => {
          const info = providers?.find((p) => p.id === c.id)
          return info ? <ProviderCard key={c.id} info={info} glyph={c.glyph} cost={c.cost} placeholder={c.placeholder} onChange={refresh} /> : null
        })}
        <CustomCard ai={s.ai} info={providers?.find((p) => p.id === 'custom')} setAi={setAi} onChange={refresh} />
      </Group>

      <Group title="Modelo predeterminado">
        <DefaultModelCard ai={s.ai} providers={providers} version={version} setAi={setAi} ready={ready} name={name} />
      </Group>

      <Group title="Por herramienta">
        {TOOLS.map((t) => (
          <ToolCard key={t.id} tool={t} ai={s.ai} providers={providers} version={version} setAi={setAi} ready={ready} name={name} />
        ))}
      </Group>

      <LocalSection local={s.ai.local} setAi={setAi} onChange={refresh} />
    </>
  )
}

// ── Proveedores ────────────────────────────────────────────────────────

function KeyStatus({ info }: { info: ProviderInfo }): React.JSX.Element {
  if (info.keyFromEnv)
    return (
      <span className="status good">
        <Icon glyph={G.checkCircle} size={12} /> Se usa la variable de entorno
      </span>
    )
  if (info.hasKey)
    return (
      <span className="status good">
        <Icon glyph={G.checkCircle} size={12} /> Lista · key guardada y cifrada en este equipo
      </span>
    )
  return <span className="muted">Sin configurar</span>
}

function KeyResultBar({ result }: { result: KeyTestResult | null }): React.JSX.Element | null {
  if (!result) return null
  return (
    <div className={result.ok ? 'infobar success fade-in' : 'infobar error fade-in'} role="status">
      <Icon glyph={result.ok ? G.checkCircle : G.error} />
      <div className="infobar-body">{result.message}</div>
    </div>
  )
}

/** Campo de key con «mostrar», Guardar (la prueba contra el proveedor antes) y Quitar. */
function KeyEditor(props: {
  provider: ProviderId
  hasKey: boolean
  placeholder: string
  optional?: boolean
  onChange: () => void
}): React.JSX.Element {
  const [value, setValue] = useState('')
  const [reveal, setReveal] = useState(false)
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<KeyTestResult | null>(null)

  const submit = async (): Promise<void> => {
    if (!value.trim() || busy) return
    setBusy(true)
    setResult(null)
    try {
      const r = await aiApi.saveKey(props.provider, value.trim())
      setResult(r)
      if (r.ok) {
        setValue('')
        setReveal(false)
        props.onChange()
      }
    } catch (err) {
      setResult({ ok: false, message: String(err) })
    } finally {
      setBusy(false)
    }
  }

  const remove = async (): Promise<void> => {
    await aiApi.removeKey(props.provider).catch(() => {})
    setResult(null)
    props.onChange()
  }

  return (
    <div className="key">
      <div className="key-row">
        <div className="password">
          <input
            className="input"
            type={reveal ? 'text' : 'password'}
            placeholder={props.hasKey ? 'Pega otra para reemplazarla' : props.placeholder}
            value={value}
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void submit()}
            aria-label={props.optional ? 'API key (opcional)' : 'API key'}
          />
          {value && (
            <button
              className="btn subtle sm icon-only password-reveal"
              onClick={() => setReveal((r) => !r)}
              title={reveal ? 'Ocultar' : 'Mostrar'}
              aria-label={reveal ? 'Ocultar la key' : 'Mostrar la key'}
            >
              <Icon glyph={reveal ? '' : ''} size={14} />
            </button>
          )}
        </div>
        <button className="btn accent" disabled={!value.trim() || busy} onClick={() => void submit()}>
          {busy ? <ProgressRing size={14} /> : null}
          Guardar
        </button>
        {props.hasKey && (
          <button className="btn" disabled={busy} onClick={() => void remove()}>
            Quitar
          </button>
        )}
      </div>
      <KeyResultBar result={result} />
    </div>
  )
}

function ProviderCard(props: {
  info: ProviderInfo
  glyph: string
  cost: string
  placeholder: string
  onChange: () => void
}): React.JSX.Element {
  const { info } = props
  const [open, setOpen] = useState(false)
  const url = info.keyUrl
  return (
    <Card
      glyph={props.glyph}
      title={info.name}
      description={
        <>
          <KeyStatus info={info} />
          <span className="card-note">{props.cost}</span>
        </>
      }
      below={
        open && (
          <div className="key fade-in">
            {info.keyFromEnv ? (
              <p className="card-note">
                La key viene de una variable de entorno. Para cambiarla, cambia la variable y reinicia Winkit; mientras exista, la
                guardada aquí no se usa.
              </p>
            ) : (
              <KeyEditor provider={info.id} hasKey={info.hasKey} placeholder={props.placeholder} onChange={props.onChange} />
            )}
            {url && !info.keyFromEnv && (
              <p className="card-note">
                Créala en{' '}
                <a
                  href={url}
                  onClick={(e) => {
                    e.preventDefault()
                    void ui.openUrl(url)
                  }}
                >
                  {new URL(url).host}
                </a>
                . Se comprueba antes de guardarla y se cifra con tu cuenta de Windows.
              </p>
            )}
          </div>
        )
      }
    >
      {/* Con la key en una variable de entorno no hay nada que cambiar aquí. */}
      {!info.keyFromEnv && (
        <button className="btn" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
          {open ? 'Cerrar' : info.hasKey ? 'Cambiar' : 'Añadir key'}
        </button>
      )}
    </Card>
  )
}

/** Servidor propio compatible con OpenAI: nombre, URL y key opcional. */
function CustomCard(props: {
  ai: Ai
  info: ProviderInfo | undefined
  setAi: (patch: Partial<Ai>) => void
  onChange: () => void
}): React.JSX.Element {
  const { ai, info } = props
  const [open, setOpen] = useState(false)
  const [name, setName] = useState(ai.custom.name)
  const [baseUrl, setBaseUrl] = useState(ai.custom.baseUrl)
  useEffect(() => setName(ai.custom.name), [ai.custom.name])
  useEffect(() => setBaseUrl(ai.custom.baseUrl), [ai.custom.baseUrl])
  const commit = (): void => {
    if (name.trim() === ai.custom.name && baseUrl.trim() === ai.custom.baseUrl) return
    props.setAi({ custom: { name: name.trim(), baseUrl: baseUrl.trim() } })
    window.setTimeout(props.onChange, 300)
  }
  const nameId = useId()
  const urlId = useId()
  const configured = !!ai.custom.baseUrl
  return (
    <Card
      glyph={G.app}
      title={ai.custom.name || 'Servidor propio'}
      description={
        <>
          {configured ? (
            <span className="status good">
              <Icon glyph={G.checkCircle} size={12} /> Listo · {ai.custom.baseUrl}
            </span>
          ) : (
            <span className="muted">Sin configurar</span>
          )}
          <span className="card-note">Cualquier API compatible con OpenAI: LM Studio, Ollama, vLLM… en tu red o en tu PC.</span>
        </>
      }
      below={
        open && (
          <div className="key fade-in">
            <div className="key-row">
              <label className="card-note" htmlFor={nameId} style={{ minWidth: 72 }}>
                Nombre
              </label>
              <input
                id={nameId}
                className="input"
                placeholder="Mi servidor"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onBlur={commit}
                style={{ maxWidth: 420, flex: 1 }}
              />
            </div>
            <div className="key-row">
              <label className="card-note" htmlFor={urlId} style={{ minWidth: 72 }}>
                URL
              </label>
              <input
                id={urlId}
                className="input"
                placeholder="http://192.168.1.20:8000/v1"
                value={baseUrl}
                spellCheck={false}
                onChange={(e) => setBaseUrl(e.target.value)}
                onBlur={commit}
                onKeyDown={(e) => e.key === 'Enter' && commit()}
                style={{ maxWidth: 420, flex: 1 }}
              />
            </div>
            <p className="card-note">La raíz de la API, la que termina en /v1. La key es opcional: guárdala solo si tu servidor la pide (al guardarla se prueba la conexión).</p>
            {configured && <KeyEditor provider="custom" hasKey={info?.hasKey ?? false} placeholder="Key (opcional)" optional onChange={props.onChange} />}
          </div>
        )
      }
    >
      <button className="btn" onClick={() => (open ? (commit(), setOpen(false)) : setOpen(true))} aria-expanded={open}>
        {open ? 'Cerrar' : configured ? 'Cambiar' : 'Configurar'}
      </button>
    </Card>
  )
}

// ── Modelos ────────────────────────────────────────────────────────────

interface ModelPickerProps {
  value: ModelRef
  providers: ProviderInfo[] | null
  version: number
  ready: (p: ProviderId) => boolean
  name: (p: ProviderId) => string
  onChange: (m: ModelRef) => void
}

/** Proveedor (solo los listos) + modelo de ese proveedor. */
function ModelPicker({ value, providers, version, ready, name, onChange }: ModelPickerProps): React.JSX.Element {
  const isReady = ready(value.provider)
  const { models, error } = useModels(value.provider, isReady, version)
  const pending = useRef<ProviderId | null>(null)

  // Al cambiar de proveedor, el modelo recomendado (cuando llega la lista).
  useEffect(() => {
    if (!models || pending.current !== value.provider) return
    pending.current = null
    const pick = models.find((m) => m.recommended) ?? models[0]
    if (pick && pick.id !== value.model) onChange({ provider: value.provider, model: pick.id })
  }, [models, value.provider, value.model, onChange])

  const providerOptions = (providers ?? [])
    .filter((p) => p.ready || p.id === value.provider)
    .map((p) => ({ value: p.id, label: p.ready ? p.name : `${p.name} (sin configurar)` }))

  // La nota (precio, «Recomendado»…) va en la segunda línea de la lista, no pegada al nombre.
  const modelOptions: { value: string; label: string; hint?: string | null }[] = (models ?? []).map((m) => ({
    value: m.id,
    label: m.name,
    hint: m.hint
  }))
  if (value.model && !modelOptions.some((o) => o.value === value.model)) {
    modelOptions.unshift({ value: value.model, label: models ? `${value.model} (no está en la lista)` : value.model })
  }
  if (!value.model && value.provider === 'local') modelOptions.unshift({ value: '', label: 'El de la IA local' })

  return (
    <div className="key">
      <div className="key-row">
        <Select<ProviderId>
          label="Proveedor"
          value={value.provider}
          options={providerOptions.length ? providerOptions : [{ value: value.provider, label: name(value.provider) }]}
          onChange={(provider) => {
            pending.current = provider
            onChange({ provider, model: provider === value.provider ? value.model : '' })
          }}
        />
        {isReady && (
          <Select<string>
            label="Modelo"
            value={value.model}
            disabled={!models}
            options={modelOptions.length ? modelOptions : [{ value: '', label: 'Cargando modelos…' }]}
            onChange={(model) => onChange({ provider: value.provider, model })}
          />
        )}
        {isReady && !models && <ProgressRing size={16} />}
      </div>
      {!isReady && (
        <p className="card-note">
          <span className="status caution">
            <Icon glyph={G.warning} size={12} /> {name(value.provider)} no está configurado: añade su key arriba o elige otro.
          </span>
        </p>
      )}
      {error && (
        <div className="infobar error fade-in" role="status">
          <Icon glyph={G.error} />
          <div className="infobar-body">No se han podido cargar los modelos: {error}</div>
        </div>
      )}
    </div>
  )
}

function TestButton({ model, disabled }: { model: ModelRef; disabled: boolean }): React.JSX.Element {
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<AiTestResult | null>(null)
  useEffect(() => setResult(null), [model.provider, model.model])
  const run = async (): Promise<void> => {
    setBusy(true)
    setResult(null)
    try {
      setResult(await aiApi.test(model))
    } catch (err) {
      setResult({ ok: false, message: String(err), ms: 0 })
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="key">
      <div className="key-row">
        <button className="btn" disabled={disabled || busy} onClick={() => void run()}>
          {busy ? <ProgressRing size={14} /> : null}
          Probar
        </button>
        <span className="card-note">Pide una frase corta al modelo y mide lo que tarda.</span>
      </div>
      {result && (
        <div className={result.ok ? 'infobar success fade-in' : 'infobar error fade-in'} role="status">
          <Icon glyph={result.ok ? G.checkCircle : G.error} />
          <div className="infobar-body">
            {result.ok ? (
              <>
                <span className="body-strong">{(result.ms / 1000).toLocaleString('es-ES', { maximumFractionDigits: 1 })} s</span> · «
                {result.message}»
              </>
            ) : (
              result.message
            )}
          </div>
        </div>
      )}
    </div>
  )
}

function DefaultModelCard(props: {
  ai: Ai
  providers: ProviderInfo[] | null
  version: number
  setAi: (patch: Partial<Ai>) => void
  ready: (p: ProviderId) => boolean
  name: (p: ProviderId) => string
}): React.JSX.Element {
  const { ai, setAi } = props
  const onChange = useCallback((m: ModelRef) => setAi({ default: m }), [setAi])
  const isReady = props.ready(ai.default.provider)
  return (
    <Card
      glyph={G.chip}
      title="Modelo predeterminado"
      description="El que usan todas las herramientas que no tienen uno propio."
      className={props.providers && !isReady ? 'attention' : undefined}
      below={
        <div className="key">
          <ModelPicker value={ai.default} providers={props.providers} version={props.version} ready={props.ready} name={props.name} onChange={onChange} />
          <TestButton model={ai.default} disabled={!isReady || (!ai.default.model && ai.default.provider !== 'local')} />
        </div>
      }
    />
  )
}

function ToolCard(props: {
  tool: (typeof TOOLS)[number]
  ai: Ai
  providers: ProviderInfo[] | null
  version: number
  setAi: (patch: Partial<Ai>) => void
  ready: (p: ProviderId) => boolean
  name: (p: ProviderId) => string
}): React.JSX.Element {
  const { tool, ai, setAi } = props
  const own = ai.tools[tool.id]
  const latestTools = useRef(ai.tools)
  latestTools.current = ai.tools
  const onChange = useCallback(
    (m: ModelRef) => setAi({ tools: { ...latestTools.current, [tool.id]: m } }),
    [setAi, tool.id]
  )
  const pickDefault = (v: boolean): void => {
    const next = { ...latestTools.current }
    if (v) delete next[tool.id]
    else next[tool.id] = { ...ai.default }
    setAi({ tools: next })
  }
  return (
    <Card
      glyph={tool.glyph}
      title={tool.title}
      description={tool.description}
      below={
        own && (
          <ModelPicker value={own} providers={props.providers} version={props.version} ready={props.ready} name={props.name} onChange={onChange} />
        )
      }
    >
      <Select<'default' | 'own'>
        label={`Modelo de ${tool.title}`}
        value={own ? 'own' : 'default'}
        options={[
          { value: 'default', label: 'Usar el predeterminado' },
          { value: 'own', label: 'Elegir uno' }
        ]}
        onChange={(v) => pickDefault(v === 'default')}
      />
    </Card>
  )
}

// ── IA local ───────────────────────────────────────────────────────────

function LocalSection(props: { local: LocalAi; setAi: (patch: Partial<Ai>) => void; onChange: () => void }): React.JSX.Element {
  const { local } = props
  const latest = useRef(local)
  latest.current = local
  const set = (patch: Partial<LocalAi>): void => props.setAi({ local: { ...latest.current, ...patch } })
  const status = useLocalStatus()
  const [catalog, setCatalog] = useState<LocalModel[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [installing, setInstalling] = useState<string | null>(null)
  const enabledId = useId()

  const loadCatalog = useCallback(() => {
    void localAi
      .catalog()
      .then(setCatalog)
      .catch(() => {})
  }, [])
  useEffect(loadCatalog, [loadCatalog])

  const install = async (id: string): Promise<void> => {
    setError(null)
    setInstalling(id)
    try {
      await localAi.install(id)
    } catch (err) {
      setError(String(err))
    } finally {
      setInstalling(null)
      loadCatalog()
      props.onChange()
    }
  }

  const remove = async (id: string): Promise<void> => {
    setError(null)
    try {
      await localAi.remove(id)
    } catch (err) {
      setError(String(err))
    } finally {
      loadCatalog()
      props.onChange()
    }
  }

  const busy = status?.state === 'downloading' || status?.state === 'verifying'
  const anyInstalled = catalog?.some((m) => m.installed) ?? false

  let statusLine: React.ReactNode
  if (!status || status.state === 'off') {
    statusLine = anyInstalled ? 'Instalada, sin cargar.' : 'Elige un modelo abajo para descargarlo.'
  } else if (status.state === 'starting') {
    statusLine = (
      <span className="status">
        <ProgressRing size={12} /> {status.message ?? 'Cargando el modelo…'}
      </span>
    )
  } else if (status.state === 'ready') {
    statusLine = (
      <span className="status good">
        <Icon glyph={G.checkCircle} size={12} /> En memoria · {status.model} · {status.backend}
      </span>
    )
  } else if (status.state === 'idle') {
    statusLine = `Lista · ${status.model} · se carga al usarla (${status.backend ?? 'CPU'})`
  } else if (status.state === 'error') {
    statusLine = (
      <span className="status bad">
        <Icon glyph={G.warning} size={12} /> {status.message}
      </span>
    )
  } else {
    statusLine = (
      <span className="status">
        <ProgressRing size={12} /> {status.message}
      </span>
    )
  }

  return (
    <>
      <Group title="IA local">
        <Card
          glyph={G.hardDrive}
          title="IA local"
          htmlFor={enabledId}
          className="clickable"
          description={
            <>
              Un modelo que corre en tu PC: gratis, sin conexión y sin que tus textos salgan del equipo. Activarla no descarga nada: el
              motor (~40 MB) y el modelo (de 2,8 a 7 GB) se bajan solo cuando eliges uno abajo.
            </>
          }
        >
          <span className="toggle-state">{local.enabled ? 'Activado' : 'Desactivado'}</span>
          <input id={enabledId} type="checkbox" className="switch" checked={local.enabled} onChange={(e) => set({ enabled: e.target.checked })} />
        </Card>
        {local.enabled && (
          <Card
            glyph={G.chip}
            title="Estado"
            description={statusLine}
            below={
              error && (
                <div className="infobar error fade-in" role="status">
                  <Icon glyph={G.error} />
                  <div className="infobar-body">{error}</div>
                </div>
              )
            }
          >
            <button className="btn" disabled={status?.state !== 'ready'} onClick={() => void localAi.unload()} title="Libera la VRAM y la RAM; se vuelve a cargar al usarla">
              Liberar memoria ahora
            </button>
            <button className="btn" onClick={() => void localAi.openFolder()}>
              Abrir carpeta
            </button>
          </Card>
        )}
      </Group>

      {local.enabled && (
        <>
          <Group title="Modelos">
            {(catalog ?? []).map((m) => {
              const downloadingThis = busy && (status?.model ? status.model === m.name : installing === m.id)
              const active = m.installed && local.model === m.id
              return (
                <Card
                  key={m.id}
                  glyph={m.installed ? G.checkCircle : G.download}
                  title={
                    <>
                      {m.name}
                      {m.recommended && <span className="muted"> · Recomendado</span>}
                      {active && <span className="muted"> · En uso</span>}
                    </>
                  }
                  description={
                    <>
                      {m.description}
                      <span className="card-note">
                        {gb(m.sizeMb)} GB de descarga · ~{m.memoryGb.toLocaleString('es-ES')} GB de memoria · Gratis
                      </span>
                    </>
                  }
                  below={
                    downloadingThis && (
                      <div className="key fade-in">
                        <span className="ai-progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round((status?.progress ?? 0) * 100)}>
                          <span style={{ width: `${(status?.progress ?? 0) * 100}%` }} />
                        </span>
                        <span className="card-note">{status?.message}</span>
                      </div>
                    )
                  }
                >
                  {downloadingThis ? (
                    <button className="btn" onClick={() => void localAi.cancel()}>
                      Cancelar
                    </button>
                  ) : m.installed ? (
                    <>
                      {!active && (
                        <button className="btn" onClick={() => set({ model: m.id })}>
                          Usar
                        </button>
                      )}
                      <button className="btn" disabled={busy} onClick={() => void remove(m.id)}>
                        Quitar
                      </button>
                    </>
                  ) : (
                    // Un solo botón de acento por vista (como en Windows): el del recomendado.
                    <button
                      className={m.recommended ? 'btn accent' : 'btn'}
                      disabled={installing !== null}
                      onClick={() => void install(m.id)}
                    >
                      <Icon glyph={G.download} />
                      Instalar
                    </button>
                  )}
                </Card>
              )
            })}
          </Group>

          <Group title="Rendimiento">
            <Card
              glyph={G.speed}
              title="Usar la GPU"
              description="Mucho más rápido (Vulkan: NVIDIA, AMD o Intel). Desactivado, usa solo el procesador."
              htmlFor={`${enabledId}-gpu`}
              className="clickable"
            >
              <span className="toggle-state">{local.gpu ? 'Activado' : 'Desactivado'}</span>
              <input id={`${enabledId}-gpu`} type="checkbox" className="switch" checked={local.gpu} onChange={(e) => set({ gpu: e.target.checked })} />
            </Card>
            <Card glyph={G.ruler} title="Contexto" description="Cuánto texto cabe de una vez. Más contexto usa más memoria.">
              <Select<string>
                label="Contexto"
                value={String(local.context)}
                options={[...new Set([...CONTEXTS, local.context])]
                  .sort((a, b) => a - b)
                  .map((c) => ({ value: String(c), label: `${c.toLocaleString('es-ES')} tokens` }))}
                onChange={(v) => set({ context: Number(v) })}
              />
            </Card>
            <Card
              glyph={G.history}
              title="Liberar la memoria tras"
              description="Sin usarse ese tiempo, el modelo sale de la VRAM y la RAM (para jugar o grabar clips sin estorbar). Se vuelve a cargar en unos segundos al usarlo."
            >
              <Select<string>
                label="Liberar la memoria tras"
                value={String(local.unloadAfterMinutes)}
                options={[...new Set([...UNLOAD, local.unloadAfterMinutes])].map((m) => ({
                  value: String(m),
                  label: m === 0 ? 'Nunca' : `${m} min`
                }))}
                onChange={(v) => set({ unloadAfterMinutes: Number(v) })}
              />
            </Card>
          </Group>
        </>
      )}
    </>
  )
}
