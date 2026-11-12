import { useCallback } from 'react'
import { palette } from '../lib/api'

/** Lo que no arrastra: controles con los que se interactúa dentro de la cabecera. */
const INTERACTIVE = 'button, input, textarea, select, a, label, [role="combobox"], [contenteditable], .no-drag'

/**
 * Cabecera que arrastra la ventana de la paleta, como la barra de título de
 * una ventana normal: `<header onMouseDown={drag}>`. Los botones, campos y
 * desplegables de dentro siguen funcionando.
 */
export function useDragHandle(): (e: React.MouseEvent) => void {
  return useCallback((e: React.MouseEvent) => {
    if (e.button !== 0 || e.detail > 1) return
    if ((e.target as HTMLElement).closest(INTERACTIVE)) return
    e.preventDefault()
    void palette.drag()
  }, [])
}
