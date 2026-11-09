import { useEffect, useRef } from 'react'
import { picker, type Capture } from '../../lib/api'

/** Celdas por lado de la lupa (impar: el píxel elegido queda en el centro). */
const CELLS = 11
/** Tamaño de cada píxel ampliado, en px CSS. */
const ZOOM = 12
const LOUPE = CELLS * ZOOM
/** Separación entre el puntero y la lupa. */
const GAP = 24

const hex = (r: number, g: number, b: number): string =>
  `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`.toUpperCase()

/**
 * Cuentagotas a pantalla completa sobre una captura congelada del monitor.
 * Nada de React por cada movimiento: refs y requestAnimationFrame.
 */
export function Picker(): React.JSX.Element {
  const screenRef = useRef<HTMLCanvasElement>(null)
  const loupeRef = useRef<HTMLDivElement>(null)
  const zoomRef = useRef<HTMLCanvasElement>(null)
  const reticleRef = useRef<HTMLDivElement>(null)
  const swatchRef = useRef<HTMLSpanElement>(null)
  const hexRef = useRef<HTMLSpanElement>(null)
  const rgbRef = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    const screen = screenRef.current!
    const zoom = zoomRef.current!
    const loupe = loupeRef.current!
    const reticle = reticleRef.current!
    const screenCtx = screen.getContext('2d', { alpha: false })!
    const zoomCtx = zoom.getContext('2d')!

    let cap: Capture | null = null
    let session: number | null = null
    // Píxel (físico) bajo la lupa; null hasta que se mueva el ratón o se pulse una flecha.
    let px: number | null = null
    let py = 0
    let raf = 0
    let finished = false

    const dpr = (): number => window.devicePixelRatio || 1

    const colorAt = (x: number, y: number): { r: number; g: number; b: number } => {
      const i = (y * cap!.width + x) * 4
      const p = cap!.pixels
      return { r: p[i], g: p[i + 1], b: p[i + 2] }
    }

    const draw = (): void => {
      raf = 0
      if (!cap || px === null) {
        loupe.style.visibility = 'hidden'
        reticle.style.visibility = 'hidden'
        return
      }
      const ratio = dpr()
      const half = (CELLS - 1) / 2

      // Lupa: los píxeles alrededor, ampliados sin suavizar.
      const size = zoom.width
      const cell = size / CELLS
      zoomCtx.imageSmoothingEnabled = false
      zoomCtx.fillStyle = '#000'
      zoomCtx.fillRect(0, 0, size, size)
      zoomCtx.drawImage(screen, px - half, py - half, CELLS, CELLS, 0, 0, size, size)

      // Rejilla.
      zoomCtx.beginPath()
      for (let i = 1; i < CELLS; i++) {
        const p = Math.round(i * cell) + 0.5
        zoomCtx.moveTo(p, 0)
        zoomCtx.lineTo(p, size)
        zoomCtx.moveTo(0, p)
        zoomCtx.lineTo(size, p)
      }
      zoomCtx.strokeStyle = 'rgba(128, 128, 128, 0.28)'
      zoomCtx.lineWidth = 1
      zoomCtx.stroke()

      // Píxel central: borde blanco con contorno negro para que se vea sobre cualquier color.
      const c0 = Math.round(half * cell)
      const c1 = Math.round((half + 1) * cell)
      zoomCtx.lineWidth = Math.max(1, Math.round(ratio))
      zoomCtx.strokeStyle = '#000'
      zoomCtx.strokeRect(c0 - zoomCtx.lineWidth, c0 - zoomCtx.lineWidth, c1 - c0 + 2 * zoomCtx.lineWidth, c1 - c0 + 2 * zoomCtx.lineWidth)
      zoomCtx.strokeStyle = '#fff'
      zoomCtx.strokeRect(c0, c0, c1 - c0, c1 - c0)

      const { r, g, b } = colorAt(px, py)
      const h = hex(r, g, b)
      swatchRef.current!.style.background = h
      hexRef.current!.textContent = h
      rgbRef.current!.textContent = `${r}, ${g}, ${b}`

      // Posición en px CSS; la lupa cambia de lado cerca de los bordes.
      const cx = px / ratio
      const cy = py / ratio
      const w = loupe.offsetWidth
      const hgt = loupe.offsetHeight
      let x = cx + GAP
      let y = cy + GAP
      if (x + w > window.innerWidth - 4) x = cx - GAP - w
      if (y + hgt > window.innerHeight - 4) y = cy - GAP - hgt
      loupe.style.transform = `translate3d(${Math.round(x)}px, ${Math.round(y)}px, 0)`
      loupe.style.visibility = 'visible'

      reticle.style.width = reticle.style.height = `${1 / ratio}px`
      reticle.style.transform = `translate3d(${cx}px, ${cy}px, 0)`
      reticle.style.visibility = 'visible'
    }

    const schedule = (): void => {
      if (!raf) raf = requestAnimationFrame(draw)
    }

    const clampTo = (x: number, y: number): void => {
      if (!cap) return
      px = Math.min(cap.width - 1, Math.max(0, x))
      py = Math.min(cap.height - 1, Math.max(0, y))
      schedule()
    }

    const finish = (pick: boolean): void => {
      if (finished) return
      if (pick && (!cap || px === null)) return
      finished = true
      const color = pick ? colorAt(px!, py) : null
      void picker.done(color).catch(() => {})
    }

    const offInit = picker.onInit(({ session: s }) => {
      if (s === session) return
      session = s
      cap = null
      finished = false
      schedule()
      void picker
        .capture()
        .then((c) => {
          if (session !== s) return
          cap = c
          const ratio = dpr()
          screen.width = c.width
          screen.height = c.height
          screen.style.width = `${c.width / ratio}px`
          screen.style.height = `${c.height / ratio}px`
          const data = new Uint8ClampedArray(c.pixels.buffer as ArrayBuffer, c.pixels.byteOffset, c.width * c.height * 4)
          screenCtx.putImageData(new ImageData(data, c.width, c.height), 0, 0)
          // La lupa, nítida a la densidad de la pantalla.
          zoom.width = zoom.height = Math.round(LOUPE * ratio)
          if (px !== null) clampTo(px, py)
          schedule()
          window.focus()
        })
        .catch(() => finish(false))
    })

    const onMove = (e: MouseEvent): void => {
      const ratio = dpr()
      clampTo(Math.floor(e.clientX * ratio), Math.floor(e.clientY * ratio))
    }
    const onDown = (e: MouseEvent): void => {
      if (e.button === 0) {
        e.preventDefault()
        onMove(e)
        finish(true)
      }
    }
    const onContextMenu = (e: MouseEvent): void => {
      e.preventDefault()
      finish(false)
    }
    const onKey = (e: KeyboardEvent): void => {
      const step = e.shiftKey ? 10 : 1
      const nudge = (dx: number, dy: number): void => {
        e.preventDefault()
        if (!cap) return
        if (px === null) clampTo(Math.floor(cap.width / 2), Math.floor(cap.height / 2))
        else clampTo(px + dx, py + dy)
      }
      switch (e.key) {
        case 'ArrowLeft':
          return nudge(-step, 0)
        case 'ArrowRight':
          return nudge(step, 0)
        case 'ArrowUp':
          return nudge(0, -step)
        case 'ArrowDown':
          return nudge(0, step)
        case 'Enter':
        case ' ':
          e.preventDefault()
          return finish(true)
        case 'Escape':
          e.preventDefault()
          return finish(false)
      }
    }

    window.addEventListener('mousemove', onMove)
    window.addEventListener('pointerover', onMove)
    window.addEventListener('mousedown', onDown)
    window.addEventListener('contextmenu', onContextMenu)
    window.addEventListener('keydown', onKey)
    return () => {
      offInit()
      cancelAnimationFrame(raf)
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('pointerover', onMove)
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('contextmenu', onContextMenu)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  return (
    <>
      <canvas ref={screenRef} className="screen" />
      <div ref={reticleRef} className="reticle" />
      <div ref={loupeRef} className="loupe" aria-hidden="true">
        <canvas ref={zoomRef} className="loupe-zoom" style={{ width: LOUPE, height: LOUPE }} />
        <div className="loupe-label">
          <span ref={swatchRef} className="loupe-swatch" />
          <span className="loupe-values">
            <span ref={hexRef} className="loupe-hex" />
            <span ref={rgbRef} className="loupe-rgb" />
          </span>
        </div>
      </div>
    </>
  )
}
