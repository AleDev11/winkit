import { resolve } from 'path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const root = resolve(import.meta.dirname, 'src')

// Una página por ventana. Tauri las carga por ruta: `windows/<nombre>/index.html`.
const WINDOWS = ['palette', 'launcher', 'settings', 'picker']

export default defineConfig({
  root,
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: resolve(import.meta.dirname, 'dist'),
    emptyOutDir: true,
    target: 'chrome120',
    rollupOptions: {
      input: Object.fromEntries(WINDOWS.map((w) => [w, resolve(root, 'windows', w, 'index.html')]))
    }
  }
})
