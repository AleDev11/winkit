import { useCallback, useEffect, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { ProgressRing } from '../../components/ProgressRing'
import { ui, voice as api } from '../../lib/api'
import type { KeyTestResult, Voice, VoiceEngineId, VoiceEngineInfo, VoiceInfo, VoiceStatus } from '../../shared/types'
import { Card, Group, RadioList, Select } from './controls'
import type { PageProps } from './SettingsApp'

const SAMPLE = 'Hola, soy la voz de Winkit. Puedo leerte las traducciones y las respuestas en voz alta.'

const LANGUAGE_NAMES: Record<string, string> = {
  es: 'Español',
  en: 'Inglés',
  ca: 'Catalán',
  fr: 'Francés',
  it: 'Italiano',
  de: 'Alemán',
  pt: 'Portugués',
  eu: 'Euskera',
  gl: 'Gallego',
  nl: 'Neerlandés',
  ja: 'Japonés',
  zh: 'Chino',
  ko: 'Coreano',
  ru: 'Ruso',
  pl: 'Polaco'
}

/** "es-ES" → "Español (es-ES)"; "mul" → "Varios idiomas". */
function languageLabel(tag: string): string {
  if (tag === 'mul') return 'Varios idiomas'
  const name = LANGUAGE_NAMES[tag.split(/[-_]/)[0].toLowerCase()]
  return name ? `${name} (${tag})` : tag
}

function voiceLabel(v: VoiceInfo): string {
  return [v.name, languageLabel(v.language), v.gender].filter(Boolean).join(' · ')
}

/** Estado de la voz (descarga, lectura) que manda el backend. */
function useVoiceStatus(): VoiceStatus | null {
  const [status, setStatus] = useState<VoiceStatus | null>(null)
  useEffect(() => {
    let alive = true
    void api.status().then((s) => alive && setStatus(s))
    const off = api.onStatus(setStatus)
    return () => {
      alive = false
      off()
    }
  }, [])
  return status
}

function useEngines(): [VoiceEngineInfo[] | null, () => void] {
  const [engines, setEngines] = useState<VoiceEngineInfo[] | null>(null)
  const reload = useCallback(() => {
    void api
      .engines()
      .then(setEngines)
      .catch(() => {})
  }, [])
  useEffect(() => {
    reload()
    // La key de OpenAI se pone en la página IA: al volver a la ventana se refresca.
    window.addEventListener('focus', reload)
    return () => window.removeEventListener('focus', reload)
  }, [reload])
  return [engines, reload]
}

/** Valor de un slider que se guarda al dejar de moverlo. */
function useSlider(value: number, commit: (v: number) => void): [number, (v: number) => void] {
  const [local, setLocal] = useState(value)
  const timer = useRef<number | undefined>(undefined)
  const commitRef = useRef(commit)
  commitRef.current = commit
  useEffect(() => setLocal(value), [value])
  useEffect(() => () => window.clearTimeout(timer.current), [])
  const change = (v: number): void => {
    setLocal(v)
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => commitRef.current(v), 350)
  }
  return [local, change]
}

export function VoicePage({ s, save }: PageProps): React.JSX.Element {
  const v = s.voice
  // Siempre sobre los ajustes más recientes (los sliders guardan con retraso).
  const latest = useRef(v)
  latest.current = v
  const set = (patch: Partial<Voice>): void => save({ voice: { ...latest.current, ...patch } })
  const [engines, reloadEngines] = useEngines()
  const status = useVoiceStatus()
  const engine = engines?.find((e) => e.id === v.engine)

  // Al terminar una descarga o borrar, se vuelve a pedir la lista (instalada sí/no).
  const lastState = useRef<string | null>(null)
  useEffect(() => {
    if (lastState.current === 'downloading' && status?.state !== 'downloading') reloadEngines()
    lastState.current = status?.state ?? null
  }, [status?.state, reloadEngines])

  const [speed, setSpeed] = useSlider(v.speed, (speed) => set({ speed }))
  const [volume, setVolume] = useSlider(v.volume, (volume) => set({ volume }))

  const engineOptions = (engines ?? []).map((e) => {
    let note = ''
    if (e.needsKey && !e.hasKey) note = e.id === 'openai' ? ' Falta la key de OpenAI (página IA).' : ' Falta la API key.'
    else if (e.needsDownload && !e.installed) note = ' Sin descargar.'
    return {
      value: e.id,
      label: e.name,
      hint: (
        <>
          {e.hint}
          {note && <span className="voice-note">{note}</span>}
        </>
      )
    }
  })

  return (
    <>
      <Group title="Motor">
        <Card
          glyph={G.volume}
          title="Motor de voz"
          description="Con qué voz lee Winkit en voz alta las traducciones de «Entender», los resultados de la paleta y las respuestas del buscador."
          below={
            engines ? (
              <RadioList<VoiceEngineId>
                name="voice-engine"
                value={v.engine}
                options={engineOptions}
                onChange={(id) => set({ engine: id, voice: '' })}
              />
            ) : (
              <ProgressRing size={16} />
            )
          }
        />
        {v.engine === 'kokoro' && engine && <LocalVoiceCard info={engine} status={status} onChanged={reloadEngines} />}
        {v.engine === 'elevenlabs' && engine && <ElevenLabsKeyCard info={engine} onChanged={reloadEngines} />}
        {v.engine === 'openai' && engine && (
          <Card
            glyph={G.key}
            title="API key de OpenAI"
            description={
              engine.hasKey ? (
                <span className="status good">
                  <Icon glyph={G.checkCircle} size={12} /> Se usa la key de OpenAI de la página IA
                </span>
              ) : (
                <span className="status caution">
                  <Icon glyph={G.warning} size={12} /> Ponla en la página IA (OpenAI). Mientras tanto se usan las voces de Windows.
                </span>
              )
            }
          />
        )}
        {engine && !engine.ready && v.engine !== 'windows' && (
          <p className="card-note voice-fallback">Hasta que esté listo, Winkit lee con las voces de Windows.</p>
        )}
      </Group>

      <Group title="Voz">
        <VoiceSelect engine={v.engine} ready={engine?.ready ?? false} value={v.voice} onChange={(voice) => set({ voice })} />
        <Card glyph={G.speed} title="Velocidad" description="1× es la velocidad normal de la voz.">
          <input
            type="range"
            className="slider"
            min={0.5}
            max={2}
            step={0.05}
            value={speed}
            style={{ '--fill': `${((speed - 0.5) / 1.5) * 100}%` } as React.CSSProperties}
            onChange={(e) => setSpeed(Number(e.target.value))}
            aria-label="Velocidad de la voz"
          />
          <span className="slider-value">{speed.toFixed(2).replace('.', ',')}×</span>
        </Card>
        <Card glyph={G.volume} title="Volumen" description="Respecto al volumen de Windows.">
          <input
            type="range"
            className="slider"
            min={0}
            max={1}
            step={0.05}
            value={volume}
            style={{ '--fill': `${volume * 100}%` } as React.CSSProperties}
            onChange={(e) => setVolume(Number(e.target.value))}
            aria-label="Volumen de la voz"
          />
          <span className="slider-value">{Math.round(volume * 100)} %</span>
        </Card>
      </Group>

      <Group title="Probar">
        <TryCard status={status} downloading={status?.state === 'downloading'} />
      </Group>
    </>
  )
}

function VoiceSelect(props: { engine: VoiceEngineId; ready: boolean; value: string; onChange: (v: string) => void }): React.JSX.Element {
  const { engine, ready } = props
  const [list, setList] = useState<VoiceInfo[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let alive = true
    setList(null)
    setError(null)
    if (!ready) return
    api
      .voices(engine)
      .then((l) => alive && setList(l))
      .catch((err: unknown) => alive && setError(String(err)))
    return () => {
      alive = false
    }
  }, [engine, ready])

  const options = [
    { value: '', label: engine === 'windows' ? 'La predeterminada de Windows' : 'La recomendada' },
    ...(list ?? []).map((voice) => ({ value: voice.id, label: voiceLabel(voice) }))
  ]
  if (props.value && list && !list.some((x) => x.id === props.value)) options.push({ value: props.value, label: 'Voz que ya no está' })

  const description =
    error ??
    (!ready
      ? 'Cuando el motor esté listo podrás elegir la voz.'
      : engine === 'windows'
        ? list && !list.some((x) => x.language.toLowerCase().startsWith('es'))
          ? 'No tienes ninguna voz de Windows en español: añádela en Configuración de Windows → Hora e idioma → Voz, o usa la voz local.'
          : 'Si el texto está en otro idioma, se lee con una voz de ese idioma si la tienes. Más voces: Configuración de Windows → Hora e idioma → Voz.'
        : engine === 'kokoro'
          ? 'Todas hablan los 31 idiomas de la voz local; el idioma se detecta en cada texto.'
          : 'Hablan varios idiomas; el del texto se detecta solo.')

  return (
    <Card glyph={G.microphone} title="Voz" description={description}>
      {ready && !list && !error ? (
        <ProgressRing size={16} />
      ) : (
        <Select<string> label="Voz" value={props.value} options={options} disabled={!ready || !list} onChange={props.onChange} />
      )}
    </Card>
  )
}

function LocalVoiceCard(props: { info: VoiceEngineInfo; status: VoiceStatus | null; onChanged: () => void }): React.JSX.Element {
  const { info, status } = props
  const [busy, setBusy] = useState<'install' | 'remove' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const downloading = status?.state === 'downloading'
  const mb = info.downloadMb ?? 0

  const install = async (): Promise<void> => {
    setError(null)
    setBusy('install')
    try {
      await api.install('kokoro')
    } catch (err) {
      const msg = String(err)
      if (!msg.includes('cancelada')) setError(msg)
    } finally {
      setBusy(null)
      props.onChanged()
    }
  }
  const remove = async (): Promise<void> => {
    setError(null)
    setBusy('remove')
    try {
      await api.remove('kokoro')
    } catch (err) {
      setError(String(err))
    } finally {
      setBusy(null)
      props.onChanged()
    }
  }

  let description: React.ReactNode
  if (downloading) {
    description = (
      <span className="status">
        <ProgressRing size={12} /> {status?.message ?? 'Descargando…'}
      </span>
    )
  } else if (info.installed) {
    description = (
      <span className="status good">
        <Icon glyph={G.checkCircle} size={12} /> Descargada en este equipo. Funciona sin conexión.
      </span>
    )
  } else {
    description = `Se descarga una vez, unos ${mb} MB, de las versiones oficiales de sherpa-onnx (motor) y Supertonic 3 (voces), y se comprueba antes de usarla. Si se corta, sigue donde se quedó.`
  }

  return (
    <Card
      glyph={G.download}
      title="Voz local"
      description={description}
      below={
        (downloading || error) && (
          <div className="voice-download">
            {downloading && (
              <span
                className="voice-bar"
                role="progressbar"
                aria-label="Descarga de la voz local"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={status?.progress != null ? Math.round(status.progress * 100) : undefined}
              >
                <span style={{ width: `${(status?.progress ?? 0) * 100}%` }} />
              </span>
            )}
            {error && (
              <div className="infobar error fade-in" role="status">
                <Icon glyph={G.error} />
                <div className="infobar-body">{error}</div>
              </div>
            )}
          </div>
        )
      }
    >
      {downloading ? (
        <button className="btn" onClick={() => void api.cancel()}>
          Cancelar
        </button>
      ) : info.installed ? (
        <button className="btn" disabled={busy !== null} onClick={() => void remove()}>
          {busy === 'remove' ? <ProgressRing size={14} /> : <Icon glyph={G.delete} />}
          Quitar
        </button>
      ) : (
        <button className="btn accent" disabled={busy !== null} onClick={() => void install()}>
          <Icon glyph={G.download} /> Descargar ({mb} MB)
        </button>
      )}
    </Card>
  )
}

const ELEVENLABS_KEYS = 'https://elevenlabs.io/app/settings/api-keys'

function ElevenLabsKeyCard(props: { info: VoiceEngineInfo; onChanged: () => void }): React.JSX.Element {
  const { info } = props
  const [value, setValue] = useState('')
  const [reveal, setReveal] = useState(false)
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<KeyTestResult | null>(null)

  const submit = async (): Promise<void> => {
    if (!value.trim() || busy) return
    setBusy(true)
    setResult(null)
    try {
      const r = await api.saveKey('elevenlabs', value.trim())
      setResult(r)
      if (r.ok) {
        setValue('')
        setReveal(false)
        props.onChanged()
      }
    } catch (err) {
      setResult({ ok: false, message: String(err) })
    } finally {
      setBusy(false)
    }
  }

  const remove = async (): Promise<void> => {
    await api.removeKey('elevenlabs')
    setResult(null)
    props.onChanged()
  }

  const status = info.keyFromEnv ? (
    <span className="status good">
      <Icon glyph={G.checkCircle} size={12} /> Se usa la variable de entorno ELEVENLABS_API_KEY
    </span>
  ) : info.hasKey ? (
    <span className="status good">
      <Icon glyph={G.checkCircle} size={12} /> Guardada y cifrada en este equipo
    </span>
  ) : (
    <span className="status caution">
      <Icon glyph={G.warning} size={12} /> Hace falta para usar ElevenLabs
    </span>
  )

  return (
    <Card
      glyph={G.key}
      title="API key de ElevenLabs"
      description={status}
      className={!info.hasKey ? 'attention' : undefined}
      below={
        <div className="key">
          {info.keyFromEnv ? (
            <p className="card-note">Para cambiarla, cambia la variable de entorno y reinicia Winkit.</p>
          ) : (
            <>
              <div className="key-row">
                <div className="password">
                  <input
                    className="input"
                    type={reveal ? 'text' : 'password'}
                    placeholder={info.hasKey ? 'Pega otra para reemplazarla' : 'sk_…'}
                    value={value}
                    autoComplete="off"
                    spellCheck={false}
                    onChange={(e) => setValue(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && void submit()}
                    aria-label="API key de ElevenLabs"
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
                <button className="btn accent" disabled={!value.trim() || busy} onClick={() => void submit()}>
                  {busy ? <ProgressRing size={14} /> : null}
                  Guardar
                </button>
                {info.hasKey && (
                  <button className="btn" disabled={busy} onClick={() => void remove()}>
                    Quitar
                  </button>
                )}
              </div>
              <p className="card-note">
                Créala en{' '}
                <a
                  href={ELEVENLABS_KEYS}
                  onClick={(e) => {
                    e.preventDefault()
                    void ui.openUrl(ELEVENLABS_KEYS)
                  }}
                >
                  elevenlabs.io
                </a>{' '}
                → API keys (con permiso de texto a voz y de leer voces). Se comprueba antes de guardarla y se cifra con tu
                cuenta de Windows. Se cobra por caracteres leídos: aprox. 0,04–0,08 $ por cada 1.000 según el modelo.
              </p>
            </>
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

function TryCard(props: { status: VoiceStatus | null; downloading: boolean }): React.JSX.Element {
  const { status } = props
  const [text, setText] = useState(SAMPLE)
  // pending: pedida y aún sin sonar (el «idle» de cortar la anterior no cuenta).
  const [phase, setPhase] = useState<'idle' | 'pending' | 'playing'>('idle')
  const [error, setError] = useState<string | null>(null)
  const active = phase !== 'idle'

  // Al terminar o fallar mientras suena, el botón vuelve a «Probar».
  useEffect(() => {
    if (phase !== 'playing') return
    if (status?.state === 'error') {
      setError(status.message)
      setPhase('idle')
    } else if (status?.state === 'idle') {
      setPhase('idle')
    }
  }, [status, phase])
  // Salir de la página corta la prueba.
  const activeRef = useRef(active)
  activeRef.current = active
  useEffect(
    () => () => {
      if (activeRef.current) void api.stop()
    },
    []
  )

  const run = async (): Promise<void> => {
    if (active) {
      setPhase('idle')
      void api.stop()
      return
    }
    setError(null)
    setPhase('pending')
    try {
      await api.speak(text)
      setPhase((p) => (p === 'pending' ? 'playing' : p))
    } catch (err) {
      setPhase('idle')
      setError(String(err))
    }
  }
  const loading = phase === 'pending' || (phase === 'playing' && status?.state === 'loading')

  return (
    <Card
      glyph={G.message}
      title="Frase de prueba"
      description="Escribe lo que quieras y pulsa Probar para oírlo con los ajustes de arriba."
      below={
        <div className="voice-try">
          <textarea className="input" rows={2} value={text} spellCheck={false} onChange={(e) => setText(e.target.value)} aria-label="Frase de prueba" />
          {error && (
            <div className="infobar error fade-in" role="status">
              <Icon glyph={G.error} />
              <div className="infobar-body">{error}</div>
            </div>
          )}
        </div>
      }
    >
      <button
        className={active ? 'btn' : 'btn accent'}
        disabled={!active && (props.downloading || !text.trim())}
        onClick={() => void run()}
        aria-pressed={active}
      >
        {loading ? <ProgressRing size={14} /> : <Icon glyph={active ? G.stop : G.play} />}
        {active ? 'Detener' : 'Probar'}
      </button>
    </Card>
  )
}
