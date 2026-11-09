import { useLayoutEffect, useRef } from 'react'

/**
 * Las ventanas emergentes miden exactamente lo que su contenido: el elemento
 * raíz se observa y cada cambio de tamaño se le pasa a Rust (px CSS).
 * `session` vuelve a informar del tamaño en cada apertura aunque no haya cambiado.
 */
export function useAutoResize<T extends HTMLElement>(
  resize: (width: number, height: number) => Promise<void>,
  session: unknown
) {
  const ref = useRef<T>(null)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    let last = ''
    const report = (): void => {
      const w = el.offsetWidth
      const h = el.offsetHeight
      const key = `${w}x${h}`
      if (!w || !h || key === last) return
      last = key
      void resize(w, h).catch(() => {})
    }
    report()
    const ro = new ResizeObserver(report)
    ro.observe(el)
    return () => ro.disconnect()
  }, [resize, session])

  return ref
}
