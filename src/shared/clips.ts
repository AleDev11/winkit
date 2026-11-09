import type { ClipQuality } from './types'

/** Tasa de bits de referencia a 1080p60 (igual que `bitrate()` en src-tauri/src/replay/encoder.rs). */
const REFERENCE: Record<ClipQuality, number> = { low: 6_000_000, medium: 10_000_000, high: 16_000_000 }

export const CLIP_QUALITIES: { value: ClipQuality; label: string }[] = [
  { value: 'low', label: 'Baja' },
  { value: 'medium', label: 'Media' },
  { value: 'high', label: 'Alta' }
]

/** Segundos de cada segmento del búfer (SEGMENT_SECS en replay/mod.rs). */
const SEGMENT_SECS = 15

/** Bits por segundo del vídeo: la referencia escalada por píxeles × fps. */
export function clipBitrate(quality: ClipQuality, width: number, height: number, fps: number): number {
  const ref = REFERENCE[quality] ?? REFERENCE.high
  const scale = (width * height * fps) / (1920 * 1080 * 60)
  return Math.min(Math.max(ref * scale, 1_500_000), 100_000_000)
}

/** Lo que ocupa el búfer en disco como mucho (los minutos que se guardan más un segmento). */
export function bufferBytes(
  c: { minutes: number; fps: number; quality: ClipQuality; systemAudio: boolean; microphone: boolean },
  width: number,
  height: number
): number {
  const audio = (c.systemAudio ? 192_000 : 0) + (c.microphone ? 128_000 : 0)
  const bps = clipBitrate(c.quality, width, height, c.fps) + audio
  return (bps * (c.minutes * 60 + SEGMENT_SECS)) / 8
}

export function formatBytes(bytes: number): string {
  const num = (n: number) => n.toLocaleString('es-ES', { maximumFractionDigits: n < 10 ? 1 : 0 })
  if (bytes >= 1e9) return `${num(bytes / 1e9)} GB`
  return `${num(Math.max(bytes, 0) / 1e6)} MB`
}

export function formatMbps(bps: number): string {
  return `${Math.round(bps / 1e6).toLocaleString('es-ES')} Mbps`
}

/** 272 → «4:32»; 3723 → «1:02:03». */
export function formatDuration(secs: number): string {
  const s = Math.max(0, Math.floor(secs))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const pad = (n: number) => String(n).padStart(2, '0')
  return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`
}
