/**
 * Datos de ejemplo para ver y diseñar las ventanas en un navegador normal, sin
 * Tauri: `npm run vite` y abre p. ej. http://localhost:1420/windows/settings/index.html
 *
 * Solo actúa en desarrollo (`import.meta.env.DEV`) y si no hay Tauri. En la
 * versión publicada `install()` ni se llama y el bundler la elimina.
 *
 * Imita lo justo de los internos de Tauri 2: `invoke`, `transformCallback`
 * (canales y eventos) y el registro de escuchas de eventos. Los datos son
 * realistas a propósito (382 modelos de OpenRouter con nombres largos, varios
 * micrófonos…) para que se vean los casos difíciles de maquetación.
 *
 * Parámetros en la URL: `?mode=menu|run|understand|notice` (paleta),
 * `?light` (tema claro), `?solid` (sin acrílico, como Windows 10).
 */

type Args = Record<string, unknown> | undefined
type Callback = (payload: unknown) => void

export function installMock(): void {
  const callbacks = new Map<number, Callback>()
  let nextId = 1
  const listeners = new Map<string, number[]>()
  const params = new URLSearchParams(location.search)

  const emit = (event: string, payload: unknown): void => {
    for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id: 0, payload })
  }

  /** Envía los mensajes de un Channel de Tauri en orden, con pausas para ver el streaming. */
  const stream = (channel: unknown, messages: unknown[], delay = 60): void => {
    const id = (channel as { id: number }).id
    messages.forEach((message, index) => setTimeout(() => callbacks.get(id)?.({ message, index }), delay * (index + 1)))
  }

  // ── Datos ──────────────────────────────────────────────────────────────

  const settings = {
    shortcuts: {
      palette: 'Control+Alt+X',
      correct: 'Control+Alt+C',
      translate: 'Control+Alt+T',
      understand: 'Control+Alt+L',
      launcher: 'Alt+Space',
      colorPicker: 'Control+Alt+P',
      saveClip: 'Control+Alt+G'
    },
    directAutoReplace: true,
    selectAllFallback: true,
    languages: ['Inglés', 'Español', 'Catalán', 'Francés', 'Italiano', 'Alemán', 'Portugués'],
    myLanguage: 'Español',
    translateTarget: 'Inglés',
    customActions: [{ id: 'custom:1', name: 'Correo a un cliente', prompt: 'Reescríbelo como un correo profesional y cercano a un cliente.' }],
    launchAtLogin: true,
    searchEngine: 'google',
    colorFormat: 'hex',
    noticePosition: 'bottomRight',
    understandSize: { width: 880, height: 540 },
    clips: {
      enabled: true,
      minutes: 5,
      folder: '',
      fps: 60,
      quality: 'high',
      systemAudio: true,
      microphone: true,
      monitor: '',
      systemAudioDevice: '',
      microphoneDevice: 'yeti'
    },
    ai: {
      default: { provider: 'anthropic', model: 'claude-haiku-5-5' },
      tools: { launcher: { provider: 'openrouter', model: 'google/gemini-3.5-flash-lite' } },
      custom: { name: '', baseUrl: '' },
      local: { enabled: true, model: 'gemma-4-e2b', gpu: true, context: 8192, unloadAfterMinutes: 5 }
    },
    voice: { engine: 'kokoro', voice: '', speed: 1, volume: 1 }
  }

  const providers = [
    { id: 'anthropic', name: 'Claude (Anthropic)', needsKey: true, hasKey: true, keyFromEnv: false, keyUrl: 'https://console.anthropic.com/settings/keys', ready: true },
    { id: 'openai', name: 'OpenAI', needsKey: true, hasKey: false, keyFromEnv: false, keyUrl: 'https://platform.openai.com/api-keys', ready: false },
    { id: 'deepseek', name: 'DeepSeek', needsKey: true, hasKey: false, keyFromEnv: false, keyUrl: 'https://platform.deepseek.com/api_keys', ready: false },
    { id: 'gemini', name: 'Google Gemini', needsKey: true, hasKey: false, keyFromEnv: true, keyUrl: 'https://aistudio.google.com/apikey', ready: true },
    { id: 'openrouter', name: 'OpenRouter', needsKey: true, hasKey: true, keyFromEnv: false, keyUrl: 'https://openrouter.ai/keys', ready: true },
    { id: 'custom', name: 'Personalizado', needsKey: false, hasKey: false, keyFromEnv: false, keyUrl: null, ready: false },
    { id: 'local', name: 'IA local', needsKey: false, hasKey: false, keyFromEnv: false, keyUrl: null, ready: true }
  ]

  const vendors = ['anthropic', 'openai', 'google', 'meta-llama', 'mistralai', 'qwen', 'deepseek', 'x-ai', 'cohere', 'nvidia', 'microsoft', 'nousresearch']
  const openrouterModels = Array.from({ length: 382 }, (_, i) => {
    const v = vendors[i % vendors.length]
    const long = i % 7 === 0 ? '-instruct-preview-extended-context-1m-thinking' : i % 3 === 0 ? '-instruct' : ''
    return {
      id: `${v}/modelo-${i}${long}`,
      name: `${v[0].toUpperCase()}${v.slice(1)}: Modelo ${i}${long ? ' Instruct Preview (contexto extendido de 1M, con razonamiento)' : ''}`,
      hint: i === 5 ? 'Recomendado · 0,30 $ / 2,50 $ por millón de tokens' : `${((i % 9) / 10 + 0.1).toFixed(2).replace('.', ',')} $ / ${(i % 13) + 1} $ por millón de tokens`,
      recommended: i === 5
    }
  })
  openrouterModels.unshift({
    id: 'google/gemini-3.5-flash-lite',
    name: 'Google: Gemini 3.5 Flash Lite',
    hint: 'Recomendado · 0,30 $ / 2,50 $ por millón de tokens',
    recommended: true
  })
  const models: Record<string, unknown[]> = {
    anthropic: [
      { id: 'claude-haiku-5-5', name: 'Claude Haiku 5.5', hint: 'Rápido y barato. Ideal para corregir.', recommended: true },
      { id: 'claude-sonnet-5-5', name: 'Claude Sonnet 5.5', hint: 'Más fino con traducciones y textos largos.', recommended: false },
      { id: 'claude-opus-5-5', name: 'Claude Opus 5.5', hint: 'El más capaz. Más lento y caro.', recommended: false }
    ],
    openrouter: openrouterModels,
    gemini: [
      { id: 'gemini-3.5-flash-lite', name: 'Gemini 3.5 Flash Lite', hint: 'Recomendado', recommended: true },
      { id: 'gemini-3.5-pro', name: 'Gemini 3.5 Pro', hint: null, recommended: false }
    ],
    local: [{ id: 'gemma-4-e2b', name: 'Gemma 4 E2B', hint: '2,8 GB · instalado', recommended: true }]
  }

  const catalog = [
    { id: 'gemma-4-e2b', name: 'Gemma 4 E2B', description: 'Rápido y ligero. Corrige y traduce bien en español.', sizeMb: 2841, memoryGb: 1.8, installed: true, recommended: true },
    { id: 'gemma-4-e4b', name: 'Gemma 4 E4B', description: 'Más fino con textos largos. Pide algo más de memoria.', sizeMb: 4590, memoryGb: 3.2, installed: false, recommended: false },
    { id: 'qwen-3.5-4b', name: 'Qwen 3.5 4B', description: 'Bueno con instrucciones libres y código.', sizeMb: 3030, memoryGb: 2.6, installed: false, recommended: false },
    { id: 'gemma-4-12b', name: 'Gemma 4 12B', description: 'El más capaz. Necesita una GPU con 8 GB o más.', sizeMb: 6980, memoryGb: 7.5, installed: false, recommended: false }
  ]

  const engines = [
    { id: 'windows', name: 'Voces de Windows', hint: 'Gratis, sin conexión. Suenan más robóticas.', needsKey: false, hasKey: false, keyFromEnv: false, keyUrl: null, needsDownload: false, installed: true, downloadMb: null, ready: true },
    { id: 'kokoro', name: 'Voz local (Supertonic)', hint: 'Gratis, sin conexión, voces neuronales. Se descarga al activarla.', needsKey: false, hasKey: false, keyFromEnv: false, keyUrl: null, needsDownload: true, installed: true, downloadMb: 137, ready: true },
    { id: 'elevenlabs', name: 'ElevenLabs', hint: 'La mejor calidad. De pago: ~0,04–0,08 $ por 1.000 caracteres.', needsKey: true, hasKey: false, keyFromEnv: false, keyUrl: 'https://elevenlabs.io/app/settings/api-keys', needsDownload: false, installed: true, downloadMb: null, ready: false },
    { id: 'openai', name: 'OpenAI', hint: 'De pago. Usa la key de OpenAI de la página IA.', needsKey: true, hasKey: false, keyFromEnv: false, keyUrl: null, needsDownload: false, installed: true, downloadMb: null, ready: false }
  ]

  const base = { detail: null, color: null, actions: [], durationMs: 6000, placement: params.get('corner') ?? 'bottomRight', group: 'clip' }
  const notices: Record<string, unknown> = {
    progress: { ...base, kind: 'progress', title: 'Guardando clip…', detail: 'Últimos 5 min', durationMs: 0 },
    success: {
      ...base,
      kind: 'success',
      title: 'Clip guardado',
      detail: 'Últimos 5 min · 312 MB',
      actions: [
        { label: 'Ver clip', action: 'openFile', arg: 'C:/Users/fonta/Videos/Winkit/clip.mp4', primary: true },
        { label: 'Mostrar en la carpeta', action: 'reveal', arg: 'C:/Users/fonta/Videos/Winkit/clip.mp4', primary: false }
      ]
    },
    color: { ...base, kind: 'color', title: '#1E90FF copiado', detail: 'rgb(30, 144, 255)', color: '#1E90FF', durationMs: 3500, group: null },
    error: {
      ...base,
      kind: 'error',
      title: 'No se ha podido guardar el clip',
      detail: 'La grabación se está reiniciando; prueba en unos segundos.',
      durationMs: 9000,
      actions: [
        { label: 'Reintentar', action: 'retryClip', arg: null, primary: true },
        { label: 'Abrir ajustes', action: 'openSettings', arg: null, primary: false }
      ]
    },
    info: { ...base, kind: 'info', title: 'No hay texto seleccionado', detail: 'Selecciona un texto en cualquier app y vuelve a pulsar el atajo.', durationMs: 5000, group: null },
    warning: { ...base, kind: 'warning', title: 'Falta configurar la IA', detail: 'Añade una API key o activa la IA local.', durationMs: 8000, group: null, actions: [{ label: 'Abrir ajustes', action: 'openSettings', arg: null, primary: true }] }
  }

  const paletteInit = (mode: string): unknown => ({
    session: 1,
    mode,
    text: 'Hey, can we move the meeting to Thursday afternoon? Something came up and I won\u2019t make it on Wednesday.',
    language: { detected: 'Inglés', translateTo: 'Español', understandTo: 'Español' },
    request: mode === 'run' ? { action: 'correct', text: 'Ola, mañana no podre ir a la reunion por que tengo medico.', language: null } : null,
    autoReplace: false,
    canReplace: true,
    notice: mode === 'notice' ? notices[params.get('notice') ?? 'success'] : null,
    languages: settings.languages,
    myLanguage: 'Español',
    translateTarget: 'Inglés',
    customActions: settings.customActions,
    modelLabel: 'Claude Haiku 5.5'
  })

  const answer = (text: string): unknown[] => {
    const words = text.split(/(?<= )/)
    const out: unknown[] = [{ type: 'start', model: 'Claude Haiku 5.5', from: 'Inglés', to: 'Español' }]
    let acc = ''
    for (const w of words) out.push({ type: 'delta', text: (acc += w) })
    out.push({ type: 'done', text })
    return out
  }

  // ── Comandos ───────────────────────────────────────────────────────────

  const handlers: Record<string, (a: Args) => unknown> = {
    ui_env: () => ({ backdrop: params.has('solid') ? 'none' : 'acrylic', accent: '#4cc2ff' }),
    settings_get: () => ({
      settings,
      hasKey: true,
      keyFromEnv: false,
      aiReady: true,
      shortcuts: Object.keys(settings.shortcuts).map((key) => ({ key, ok: key !== 'saveClip' })),
      version: '0.3.0-dev'
    }),
    settings_save: (a) => {
      Object.assign(settings, (a as { patch: object }).patch)
      return { settings, shortcuts: [] }
    },
    ai_providers: () => providers,
    ai_models: (a) => models[(a as { provider: string }).provider] ?? [],
    ai_model_for: (a) => (settings.ai.tools as Record<string, unknown>)[(a as { tool: string }).tool] ?? settings.ai.default,
    ai_test: () => new Promise((r) => setTimeout(() => r({ ok: true, message: 'Hola, ¿en qué te ayudo?', ms: 812 }), 800)),
    ai_save_key: () => new Promise((r) => setTimeout(() => r({ ok: true, message: 'La API key funciona y está guardada.' }), 600)),
    local_ai_catalog: () => catalog,
    local_ai_status: () => ({ state: 'idle', progress: null, message: null, backend: 'GPU (Vulkan): NVIDIA GeForce RTX 3070', model: 'Gemma 4 E2B' }),
    voice_engines: () => engines,
    voice_voices: () => [
      { id: 'F1', name: 'Mujer 1', language: 'mul', gender: 'Mujer' },
      { id: 'F2', name: 'Mujer 2', language: 'mul', gender: 'Mujer' },
      { id: 'M1', name: 'Hombre 1', language: 'mul', gender: 'Hombre' },
      { id: 'M2', name: 'Hombre 2', language: 'mul', gender: 'Hombre' }
    ],
    voice_status: () => ({ state: 'idle', progress: null, message: null }),
    clips_status: () => ({ state: 'recording', message: 'Codificador: NVIDIA NVENC', bufferedSeconds: 272, folder: 'C:\\Users\\fonta\\Videos\\Winkit' }),
    clips_monitors: () => [
      { id: 'zowie', name: 'ZOWIE XL LCD', width: 1920, height: 1080, primary: true },
      { id: 'samsung', name: 'SAMSUNG', width: 3840, height: 2160, primary: false },
      { id: 'dell', name: 'DELL U2720Q (2)', width: 2560, height: 1440, primary: false }
    ],
    clips_audio_devices: () => ({
      inputs: [
        { id: 'creative', name: 'Microphone Array (Creative Live! Cam Sync 1080p V2 Audio)', isDefault: true },
        { id: 'yeti', name: 'Microphone (Yeti Stereo Microphone)', isDefault: false },
        { id: 'headset', name: 'Micrófono de los auriculares (HyperX Cloud Alpha Wireless)', isDefault: false },
        { id: 'line', name: 'Line In (Realtek(R) Audio)', isDefault: false }
      ],
      outputs: [
        { id: 'speakers', name: 'Altavoces (Realtek(R) Audio)', isDefault: true },
        { id: 'monitor', name: 'SAMSUNG (NVIDIA High Definition Audio)', isDefault: false }
      ]
    }),
    clips_mic_level: () => Math.random() * 0.6,
    update_status: () => ({ state: 'upToDate', version: null, progress: null, message: null }),
    palette_current: () => paletteInit(params.get('mode') ?? 'menu'),
    launcher_current: () => ({ session: 1, engineLabel: 'Google', modelLabel: 'Gemini 3.5 Flash Lite' }),
    list_apps: () => [
      { id: 'discord', name: 'Discord' },
      { id: 'spotify', name: 'Spotify' },
      { id: 'code', name: 'Visual Studio Code' }
    ],
    palette_run: (a) => {
      stream((a as { onEvent: unknown }).onEvent, answer('Hola, mañana no podré ir a la reunión porque tengo médico.'))
      return null
    },
    launcher_ask: (a) => {
      stream((a as { onEvent: unknown }).onEvent, answer('Un año bisiesto tiene **366 días**: el normal de 365 más el 29 de febrero.'))
      return null
    },
    detect_language: () => ({ detected: 'Inglés', translateTo: 'Español', understandTo: 'Español' }),
    'plugin:event|listen': (a) => {
      const { event, handler } = a as { event: string; handler: number }
      listeners.set(event, [...(listeners.get(event) ?? []), handler])
      return handler
    },
    'plugin:event|unlisten': () => null
  }

  const w = window as unknown as Record<string, unknown>
  w.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'mock' }, currentWebview: { windowLabel: 'mock', label: 'mock' } },
    transformCallback: (cb: Callback) => {
      const id = nextId++
      callbacks.set(id, cb)
      return id
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
    invoke: async (cmd: string, args: Args) => {
      const h = handlers[cmd]
      if (!h) {
        console.info('[mock] sin datos para', cmd, args)
        return null
      }
      return h(args)
    },
    convertFileSrc: (p: string) => p
  }
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) =>
      listeners.set(event, (listeners.get(event) ?? []).filter((x) => x !== id))
  }

  if (params.has('light')) {
    const style = document.createElement('style')
    style.textContent = ':root { color-scheme: light; }'
    document.head.append(style)
  }
  // Fondo de escritorio de ejemplo, para que el acrílico (translúcido) se lea como en Windows.
  document.documentElement.classList.add('mock')
  const bg = document.createElement('style')
  bg.textContent =
    'html.mock[data-backdrop="acrylic"] { background: linear-gradient(135deg, #1d3b6b, #0b1220 55%, #2a1440) fixed; }'
  document.head.append(bg)
  // Para probar los eventos a mano desde la consola: __mockEmit('local-ai:status', {...})
  w.__mockEmit = emit
}
