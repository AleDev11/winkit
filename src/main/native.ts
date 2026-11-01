import koffi from 'koffi'

/**
 * Lo mínimo de user32 que Electron no expone: simular teclas en la app que
 * tiene el foco y saber/devolver cuál es esa ventana.
 */
const user32 = koffi.load('user32.dll')

const keybd_event = user32.func('void __stdcall keybd_event(uint8_t bVk, uint8_t bScan, uint32_t dwFlags, uintptr_t dwExtraInfo)')
const GetAsyncKeyState = user32.func('int16_t __stdcall GetAsyncKeyState(int vKey)')
const GetForegroundWindowRaw = user32.func('uintptr_t __stdcall GetForegroundWindow()')
const SetForegroundWindowRaw = user32.func('bool __stdcall SetForegroundWindow(uintptr_t hWnd)')

const KEYEVENTF_KEYUP = 0x0002

export const VK = {
  SHIFT: 0x10,
  CONTROL: 0x11,
  MENU: 0x12, // Alt
  LWIN: 0x5b,
  RWIN: 0x5c,
  A: 0x41,
  C: 0x43,
  V: 0x56
} as const

const MODIFIERS = [VK.SHIFT, VK.CONTROL, VK.MENU, VK.LWIN, VK.RWIN]

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms))

function isDown(vk: number): boolean {
  return (GetAsyncKeyState(vk) & 0x8000) !== 0
}

/**
 * El atajo global salta con las teclas aún pulsadas: si mandamos Ctrl+C mientras
 * el usuario sigue con Alt abajo, la app recibe Ctrl+Alt+C. Esperamos a que las
 * suelte y, si tarda demasiado, las soltamos nosotros.
 */
export async function waitForModifiersReleased(timeoutMs = 800): Promise<void> {
  const until = Date.now() + timeoutMs
  while (Date.now() < until) {
    if (!MODIFIERS.some(isDown)) return
    await sleep(15)
  }
  for (const vk of MODIFIERS) if (isDown(vk)) keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)
}

/** Pulsa Ctrl + `key` en la ventana que tenga el foco. */
export async function sendCtrl(key: number): Promise<void> {
  keybd_event(VK.CONTROL, 0, 0, 0)
  keybd_event(key, 0, 0, 0)
  await sleep(10)
  keybd_event(key, 0, KEYEVENTF_KEYUP, 0)
  keybd_event(VK.CONTROL, 0, KEYEVENTF_KEYUP, 0)
}

export function getForegroundWindow(): number {
  return Number(GetForegroundWindowRaw())
}

export function setForegroundWindow(hwnd: number): boolean {
  return SetForegroundWindowRaw(hwnd)
}

const crypt32 = koffi.load('crypt32.dll')
const kernel32 = koffi.load('kernel32.dll')
const DATA_BLOB = koffi.struct('DATA_BLOB', { cbData: 'uint32_t', pbData: 'void *' })
const CryptUnprotectData = crypt32.func(
  'bool __stdcall CryptUnprotectData(DATA_BLOB *pDataIn, void *desc, void *entropy, void *reserved, void *prompt, uint32_t flags, _Out_ DATA_BLOB *pDataOut)'
)
const LocalFree = kernel32.func('void * __stdcall LocalFree(void *hMem)')

/** Descifra datos protegidos con DPAPI para el usuario actual. */
export function dpapiUnprotect(data: Buffer): Buffer | null {
  const out: { cbData?: number; pbData?: unknown } = {}
  if (!CryptUnprotectData({ cbData: data.length, pbData: data }, null, null, null, null, 0, out)) return null
  const bytes = Buffer.from(koffi.decode(out.pbData, koffi.array('uint8_t', out.cbData ?? 0)) as number[])
  LocalFree(out.pbData)
  return bytes
}

/** Handle nativo de una BrowserWindow, para compararlo con GetForegroundWindow. */
export function hwndOf(handle: Buffer): number {
  return handle.length >= 8 ? Number(handle.readBigUInt64LE(0)) : handle.readUInt32LE(0)
}
