import { clipboard, ClipboardItem } from 'electron'
import { getForegroundWindow, sendCtrl, setForegroundWindow, VK, waitForModifiersReleased } from './native'

/**
 * Leer y reemplazar texto en cualquier app a través del portapapeles:
 * Ctrl+C para leer la selección y Ctrl+V para escribir el resultado. El
 * portapapeles del usuario se guarda antes y se devuelve después.
 */
type ClipboardSnapshot = ClipboardItem[]

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms))

/** Copia todos los formatos que exponga el portapapeles (texto, HTML, imágenes…). */
async function snapshot(): Promise<ClipboardSnapshot> {
  try {
    const items = await clipboard.read()
    const copies = await Promise.all(
      items.map(async (item) => {
        const entries = await Promise.all(
          item.types.map(async (type) => {
            try {
              return [type, await item.getType(type)] as const
            } catch {
              return null
            }
          })
        )
        const data = Object.fromEntries(entries.filter((e) => e !== null))
        return Object.keys(data).length ? new ClipboardItem(data) : null
      })
    )
    return copies.filter((c) => c !== null)
  } catch {
    return []
  }
}

async function restore(s: ClipboardSnapshot): Promise<void> {
  if (!s.length) return clipboard.clear()
  try {
    await clipboard.write(s)
  } catch {
    // Algún formato exótico que no se puede volver a escribir: mejor vacío que con nuestro texto.
    clipboard.clear()
  }
}

async function waitForText(timeoutMs: number): Promise<string> {
  const until = Date.now() + timeoutMs
  while (Date.now() < until) {
    const text = await clipboard.readText()
    if (text) return text
    await sleep(20)
  }
  return ''
}

export interface Capture {
  /** Ventana de la que salió el texto; ahí se pega el resultado. */
  hwnd: number
  text: string
}

export async function captureSelection(selectAllFallback: boolean): Promise<Capture> {
  const hwnd = getForegroundWindow()
  await waitForModifiersReleased()
  const saved = await snapshot()
  try {
    clipboard.clear()
    await sendCtrl(VK.C)
    let text = await waitForText(450)
    if (!text.trim() && selectAllFallback) {
      await sendCtrl(VK.A)
      await sleep(40)
      await sendCtrl(VK.C)
      text = await waitForText(450)
    }
    return { hwnd, text }
  } finally {
    await restore(saved)
  }
}

/** Devuelve el foco a la ventana original y pega `text` sobre la selección. */
export async function pasteInto(hwnd: number, text: string): Promise<void> {
  const saved = await snapshot()
  await clipboard.writeText(text)
  if (getForegroundWindow() !== hwnd) {
    setForegroundWindow(hwnd)
    await sleep(120)
  }
  await waitForModifiersReleased()
  await sendCtrl(VK.V)
  // La app lee el portapapeles de forma asíncrona: no lo devolvemos hasta que haya pegado.
  await sleep(350)
  await restore(saved)
}
