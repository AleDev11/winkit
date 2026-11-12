import { useEffect, useId, useRef, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { ProgressRing } from '../../components/ProgressRing'
import { clips } from '../../lib/api'
import { bufferBytes, CLIP_QUALITIES, clipBitrate, formatBytes, formatDuration, formatMbps } from '../../shared/clips'
import type { AudioDevice, AudioDevices, ClipMonitor, ClipQuality, Clips, ClipsStatus } from '../../shared/types'
import { acceleratorParts } from './accelerator'
import { Card, Group, Select, ToggleCard } from './controls'
import type { PageProps } from './SettingsApp'

/** Resolución del monitor en píxeles reales (para estimar la tasa de bits). */
function screenPixels(): [number, number] {
  const dpr = window.devicePixelRatio || 1
  return [Math.round(window.screen.width * dpr), Math.round(window.screen.height * dpr)]
}

/** Monitores conectados; se vuelven a pedir al volver a la ventana (por si se enchufa uno). */
function useMonitors(): ClipMonitor[] | null {
  const [list, setList] = useState<ClipMonitor[] | null>(null)
  useEffect(() => {
    let alive = true
    const load = (): void =>
      void clips
        .monitors()
        .then((m) => alive && setList(m))
        .catch(() => {})
    load()
    window.addEventListener('focus', load)
    return () => {
      alive = false
      window.removeEventListener('focus', load)
    }
  }, [])
  return list
}

/** Micrófonos y salidas activos; se vuelven a pedir al volver a la ventana. */
function useAudioDevices(): AudioDevices | null {
  const [devices, setDevices] = useState<AudioDevices | null>(null)
  useEffect(() => {
    let alive = true
    const load = (): void =>
      void clips
        .audioDevices()
        .then((d) => alive && setDevices(d))
        .catch(() => {})
    load()
    window.addEventListener('focus', load)
    return () => {
      alive = false
      window.removeEventListener('focus', load)
    }
  }, [])
  return devices
}

/** Opciones de un selector de dispositivo: el predeterminado de Windows (con su
 *  nombre), cada dispositivo, y el guardado si ya no está conectado. */
function deviceOptions(list: AudioDevice[] | undefined, saved: string, missing: string): { value: string; label: string }[] {
  const def = list?.find((d) => d.isDefault)
  const options = [
    { value: '', label: def ? `Predeterminado de Windows (${def.name})` : 'Predeterminado de Windows' },
    ...(list ?? []).map((d) => ({ value: d.id, label: d.name }))
  ]
  if (saved && list && !list.some((d) => d.id === saved)) options.push({ value: saved, label: missing })
  return options
}

/** Medidor de nivel del micrófono: solo pregunta mientras se ve (el backend
 *  cierra el micro 2 s después de la última pregunta). */
function MicMeter({ device }: { device: string }): React.JSX.Element {
  const [level, setLevel] = useState(0)
  useEffect(() => {
    let alive = true
    let shown = 0
    const tick = (): void => {
      if (document.hidden) return
      void clips
        .micLevel(device)
        .then((peak) => {
          if (!alive) return
          // Escala en dB: de −60 dB (vacío) a 0 dB (lleno), con caída suave.
          const db = peak > 0 ? 20 * Math.log10(peak) : -60
          shown = Math.max(Math.min(Math.max((db + 60) / 60, 0), 1), shown * 0.7)
          setLevel(shown)
        })
        .catch(() => {})
    }
    const timer = window.setInterval(tick, 100)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [device])
  return (
    <span className="mic-meter" role="meter" aria-label="Nivel del micrófono" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(level * 100)}>
      <span style={{ width: `${level * 100}%` }} />
    </span>
  )
}

/** Estado del backend más un reloj local para que los segundos del búfer avancen entre avisos. */
function useClipsStatus(): [ClipsStatus | null, number] {
  const [status, setStatus] = useState<{ s: ClipsStatus; at: number } | null>(null)
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    let alive = true
    void clips.status().then((s) => alive && setStatus({ s, at: Date.now() }))
    const off = clips.onStatus((s) => setStatus({ s, at: Date.now() }))
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => {
      alive = false
      off()
      window.clearInterval(timer)
    }
  }, [])
  const extra = status && status.s.state === 'recording' ? Math.max(0, Math.floor((now - status.at) / 1000)) : 0
  return [status?.s ?? null, status ? status.s.bufferedSeconds + extra : 0]
}

type SaveResult = { ok: true; path: string } | { ok: false; message: string }

/** Cuánto se queda a la vista el aviso de «Guardar clip ahora». */
const RESULT_OK_MS = 6_000
const RESULT_ERROR_MS = 10_000

export function ClipsPage({ s, state, save }: PageProps): React.JSX.Element {
  const c = s.clips
  // Siempre sobre los ajustes más recientes (los minutos se guardan con retraso).
  const latest = useRef(c)
  latest.current = c
  const set = (patch: Partial<Clips>): void => save({ clips: { ...latest.current, ...patch } })
  const [status, buffered] = useClipsStatus()
  const monitors = useMonitors()
  const devices = useAudioDevices()
  // Resolución del monitor que se graba (para estimar la tasa de bits y el disco).
  const recorded = monitors?.find((m) => m.id === c.monitor) ?? monitors?.find((m) => m.primary)
  const [width, height] = recorded ? [recorded.width, recorded.height] : screenPixels()
  const monitorOptions = [
    { value: '', label: 'Principal' },
    ...(monitors ?? []).map((m) => ({
      value: m.id,
      label: `${m.name} · ${m.width}×${m.height}${m.primary ? ' (principal)' : ''}`
    }))
  ]
  if (c.monitor && monitors && !monitors.some((m) => m.id === c.monitor)) {
    monitorOptions.push({ value: c.monitor, label: 'Monitor no conectado (se graba el principal)' })
  }
  const keys = acceleratorParts(s.shortcuts.saveClip)
  const shortcutTaken = !!s.shortcuts.saveClip && state.shortcuts.find((x) => x.key === 'saveClip')?.ok === false
  const enabledId = useId()

  // Los minutos se guardan al soltar el control (cambiarlos no reinicia la grabación).
  const [minutes, setMinutes] = useState(c.minutes)
  const minutesTimer = useRef<number | undefined>(undefined)
  useEffect(() => setMinutes(c.minutes), [c.minutes])
  const changeMinutes = (m: number): void => {
    setMinutes(m)
    window.clearTimeout(minutesTimer.current)
    minutesTimer.current = window.setTimeout(() => set({ minutes: m }), 400)
  }

  const [saving, setSaving] = useState(false)
  const [result, setResult] = useState<SaveResult | null>(null)
  // El aviso del guardado se va solo: el de éxito a los 6 s, el de error a los 10 s.
  useEffect(() => {
    if (!result) return
    const timer = window.setTimeout(() => setResult(null), result.ok ? RESULT_OK_MS : RESULT_ERROR_MS)
    return () => window.clearTimeout(timer)
  }, [result])
  const saveNow = async (): Promise<void> => {
    setSaving(true)
    setResult(null)
    try {
      setResult({ ok: true, path: await clips.save() })
    } catch (err) {
      setResult({ ok: false, message: String(err) })
    } finally {
      setSaving(false)
    }
  }

  const recording = status?.state === 'recording'
  const shown = Math.min(buffered, minutes * 60)
  const encoder = status?.message?.replace(/^Codificador:\s*/, '')
  const estimate = formatBytes(bufferBytes({ ...c, minutes }, width, height))

  const shortcut = keys.length ? (
    <>
      {keys.map((k, i) => (
        <kbd key={i} className="inline-kbd">
          {k}
        </kbd>
      ))}
    </>
  ) : (
    'el atajo'
  )

  let statusLine: React.ReactNode
  if (!c.enabled || !status || status.state === 'off') {
    statusLine = 'No se está grabando.'
  } else if (status.state === 'starting') {
    statusLine = (
      <span className="status">
        <ProgressRing size={12} /> Iniciando la grabación…
      </span>
    )
  } else if (status.state === 'error') {
    statusLine = (
      <span className="status bad">
        <Icon glyph={G.warning} size={12} /> No se puede grabar
      </span>
    )
  } else {
    statusLine = (
      <span className="clips-live">
        <span className="rec-dot" aria-hidden="true" />
        Grabando · {formatDuration(shown)} en el búfer{encoder ? ` · ${encoder}` : ''}
      </span>
    )
  }

  return (
    <>
      <Group>
        <Card
          glyph={G.record}
          title="Clips"
          htmlFor={enabledId}
          className="clickable"
          description={
            <>
              Graba la pantalla sin parar y, cuando pulsas {shortcut}, guarda los últimos minutos en un vídeo. Mientras tanto solo
              hay un búfer temporal en tu disco que se va borrando; nada sale de tu PC.
            </>
          }
        >
          <span className="toggle-state">{c.enabled ? 'Activado' : 'Desactivado'}</span>
          <input
            id={enabledId}
            type="checkbox"
            className="switch"
            checked={c.enabled}
            onChange={(e) => set({ enabled: e.target.checked })}
          />
        </Card>
        {c.enabled && (
          <Card
            glyph={G.video}
            title="Estado"
            description={statusLine}
            below={
              (status?.state === 'error' || result || shortcutTaken) && (
                <div className="clips-messages">
                  {status?.state === 'error' && status.message && (
                    <div className="infobar error fade-in" role="status">
                      <Icon glyph={G.error} />
                      <div className="infobar-body">{status.message}</div>
                    </div>
                  )}
                  {shortcutTaken && (
                    <div className="infobar caution fade-in" role="status">
                      <Icon glyph={G.warning} />
                      <div className="infobar-body">
                        Otra app ya usa este atajo. Elige otro en Atajos o guarda
                        desde aquí.
                      </div>
                    </div>
                  )}
                  {result && (
                    <div className={result.ok ? 'infobar success fade-in' : 'infobar error fade-in'} role="status">
                      <Icon glyph={result.ok ? G.checkCircle : G.error} />
                      <div className="infobar-body">
                        {result.ok ? (
                          <>
                            Clip guardado: <span className="clips-path">{result.path}</span>
                          </>
                        ) : (
                          result.message
                        )}
                      </div>
                      {result.ok && (
                        <button className="btn sm" onClick={() => void clips.openFolder()}>
                          Abrir carpeta
                        </button>
                      )}
                    </div>
                  )}
                </div>
              )
            }
          >
            <button className="btn accent" disabled={!recording || saving} onClick={() => void saveNow()}>
              {saving ? <ProgressRing size={14} /> : <Icon glyph={G.save} />}
              Guardar clip ahora
            </button>
          </Card>
        )}
      </Group>

      <Group title="Grabación">
        <Card
          glyph={G.monitor}
          title="Monitor"
          description="Si no está conectado, se graba el principal. Con HDR se convierte a SDR para que se vea como en la pantalla."
        >
          <Select<string> label="Monitor" value={c.monitor} options={monitorOptions} onChange={(monitor) => set({ monitor })} />
        </Card>
        <Card
          glyph={G.history}
          title="Duración del clip"
          description={`Lo que se guarda al pulsar el atajo. El búfer ocupa como mucho unos ${estimate} en disco.`}
        >
          <input
            type="range"
            className="slider"
            min={1}
            max={30}
            step={1}
            value={minutes}
            style={{ '--fill': `${((minutes - 1) / 29) * 100}%` } as React.CSSProperties}
            onChange={(e) => changeMinutes(Number(e.target.value))}
            aria-label="Minutos que se guardan"
          />
          <span className="slider-value">{minutes} min</span>
        </Card>
        <Card glyph={G.speed} title="Fotogramas por segundo" description="60 fps se ve más fluido en juegos; 30 fps ocupa la mitad.">
          <Select<'30' | '60'>
            label="Fotogramas por segundo"
            value={c.fps === 30 ? '30' : '60'}
            options={[
              { value: '30', label: '30 fps' },
              { value: '60', label: '60 fps' }
            ]}
            onChange={(v) => set({ fps: v === '30' ? 30 : 60 })}
          />
        </Card>
        <Card
          glyph={G.equalizer}
          title="Calidad"
          description={`Unos ${formatMbps(clipBitrate(c.quality, width, height, c.fps))} a ${width}×${height}. Más calidad ocupa más disco.`}
        >
          <Select<ClipQuality> label="Calidad" value={c.quality} options={CLIP_QUALITIES} onChange={(quality) => set({ quality })} />
        </Card>
      </Group>

      <Group title="Audio">
        <ToggleCard
          glyph={G.volume}
          title="Audio del sistema"
          description="Lo que suena en el PC: juegos, vídeos, llamadas. Va en su propia pista."
          checked={c.systemAudio}
          onChange={(systemAudio) => set({ systemAudio })}
        />
        <Card title="Salida que se graba" description="Lo que suena por ese dispositivo. Si no está conectado, se usa el predeterminado." className="sub-card">
          <Select<string>
            label="Salida que se graba"
            value={c.systemAudioDevice}
            disabled={!c.systemAudio}
            options={deviceOptions(devices?.outputs, c.systemAudioDevice, 'Salida no conectada (se usa la predeterminada)')}
            onChange={(systemAudioDevice) => set({ systemAudioDevice })}
          />
        </Card>
        <ToggleCard
          glyph={G.microphone}
          title="Micrófono"
          description="Tu voz, en una pista aparte para poder ajustarla o quitarla al editar."
          checked={c.microphone}
          onChange={(microphone) => set({ microphone })}
        />
        <Card
          title="Micrófono que se graba"
          description="Habla y mira la barra para saber cuál es. Si no está conectado, se usa el predeterminado."
          className="sub-card"
        >
          {c.microphone && <MicMeter device={c.microphoneDevice} />}
          <Select<string>
            label="Micrófono que se graba"
            value={c.microphoneDevice}
            disabled={!c.microphone}
            options={deviceOptions(devices?.inputs, c.microphoneDevice, 'Micrófono no conectado (se usa el predeterminado)')}
            onChange={(microphoneDevice) => set({ microphoneDevice })}
          />
        </Card>
      </Group>

      <Group title="Destino">
        <Card glyph={G.folder} title="Carpeta de los clips" description={<span className="clips-path">{status?.folder ?? ' '}</span>}>
          <button
            className="btn"
            onClick={() =>
              void clips.chooseFolder().then((folder) => {
                if (folder) save({ clips: { ...c, folder } })
              })
            }
          >
            Cambiar…
          </button>
          <button className="btn" onClick={() => void clips.openFolder()}>
            Abrir carpeta
          </button>
        </Card>
      </Group>
    </>
  )
}
