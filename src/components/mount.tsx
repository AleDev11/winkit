import { createRoot } from 'react-dom/client'
import { ui } from '../lib/api'
import { installMock } from '../dev/mock'

// En un navegador normal (npm run vite), datos de ejemplo en vez de Tauri. Ver dev/mock.ts.
if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) installMock()

/**
 * Aplica el entorno de Windows (material de fondo y color de acento) a la
 * ventana. Hasta que llega, se asume acrílico: la ventana aún no es visible.
 */
async function applyEnv(): Promise<void> {
  const root = document.documentElement
  try {
    const env = await ui.env()
    root.dataset.backdrop = env.backdrop
    if (/^#[0-9a-f]{6}$/i.test(env.accent)) root.style.setProperty('--accent', env.accent)
  } catch {
    root.dataset.backdrop = 'none'
  }
}

/** Monta la ventana. Quita el menú contextual del navegador fuera de los campos de texto. */
export function mount(app: React.ReactNode): void {
  void applyEnv()
  document.addEventListener('contextmenu', (e) => {
    const t = e.target as HTMLElement | null
    if (!t?.closest('input, textarea, .selectable')) e.preventDefault()
  })
  createRoot(document.getElementById('root')!).render(app)
}
