import { describe, expect, it } from 'vitest'
import { bufferBytes, clipBitrate, formatBytes, formatDuration } from './clips'

describe('clips', () => {
  it('usa la misma tasa de bits que el codificador', () => {
    expect(clipBitrate('low', 1920, 1080, 60)).toBe(6_000_000)
    expect(clipBitrate('medium', 1920, 1080, 60)).toBe(10_000_000)
    expect(clipBitrate('high', 1920, 1080, 60)).toBe(16_000_000)
    expect(clipBitrate('high', 1920, 1080, 30)).toBe(8_000_000)
    expect(clipBitrate('medium', 3840, 2160, 60)).toBe(40_000_000)
    expect(clipBitrate('low', 640, 360, 30)).toBe(1_500_000)
  })

  it('estima el búfer con los minutos más un segmento', () => {
    const c = { minutes: 5, fps: 60, quality: 'high' as const, systemAudio: true, microphone: false }
    // (16 Mbps + 192 kbps) × 315 s / 8
    expect(bufferBytes(c, 1920, 1080)).toBe((16_192_000 * 315) / 8)
    expect(formatBytes(bufferBytes(c, 1920, 1080))).toBe('638 MB')
    expect(formatBytes(bufferBytes({ ...c, minutes: 30 }, 1920, 1080))).toBe('3,7 GB')
  })

  it('formatea duraciones', () => {
    expect(formatDuration(0)).toBe('0:00')
    expect(formatDuration(272)).toBe('4:32')
    expect(formatDuration(3723)).toBe('1:02:03')
  })
})
