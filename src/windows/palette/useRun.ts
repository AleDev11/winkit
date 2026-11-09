import { useCallback, useEffect, useRef, useState } from 'react'
import { palette } from '../../lib/api'
import type { RunRequest } from '../../shared/types'

export interface RunState {
  status: 'idle' | 'streaming' | 'done' | 'error'
  text: string
  model: string | null
  from: string | null
  to: string | null
  error: { message: string; openSettings: boolean } | null
}

const IDLE: RunState = { status: 'idle', text: '', model: null, from: null, to: null, error: null }

/**
 * Una ejecución en curso de la paleta. Cada `start` invalida la anterior: los
 * eventos que aún lleguen de ella se descartan. Al desmontar se cancela.
 */
export function useRun() {
  const [state, setState] = useState<RunState>(IDLE)
  const current = useRef(0)
  const streaming = useRef(false)

  const start = useCallback((request: RunRequest) => {
    const id = ++current.current
    streaming.current = true
    setState({ ...IDLE, status: 'streaming' })
    palette
      .run(request, (ev) => {
        if (id !== current.current) return
        switch (ev.type) {
          case 'start':
            setState((s) => ({ ...s, model: ev.model, from: ev.from, to: ev.to }))
            break
          case 'delta':
            setState((s) => ({ ...s, text: ev.text }))
            break
          case 'done':
            streaming.current = false
            setState((s) => ({ ...s, status: 'done', text: ev.text }))
            break
          case 'error':
            streaming.current = false
            setState((s) => ({ ...s, status: 'error', error: { message: ev.message, openSettings: ev.openSettings } }))
            break
        }
      })
      .catch((err: unknown) => {
        if (id !== current.current) return
        streaming.current = false
        setState((s) => ({ ...s, status: 'error', error: { message: errorMessage(err), openSettings: false } }))
      })
  }, [])

  const cancel = useCallback(() => {
    current.current++
    if (streaming.current) void palette.cancel().catch(() => {})
    streaming.current = false
    setState(IDLE)
  }, [])

  useEffect(
    () => () => {
      current.current++
      if (streaming.current) void palette.cancel().catch(() => {})
    },
    []
  )

  return { state, start, cancel }
}

export function errorMessage(err: unknown): string {
  if (typeof err === 'string') return err
  if (err instanceof Error) return err.message
  return 'Algo ha fallado. Inténtalo de nuevo.'
}
