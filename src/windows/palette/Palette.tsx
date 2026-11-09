import { useCallback, useEffect, useRef, useState } from 'react'
import { useAutoResize } from '../../components/useAutoResize'
import { palette } from '../../lib/api'
import type { PaletteInit, RunRequest } from '../../shared/types'
import { MenuView, type MenuState } from './MenuView'
import { NoticeView } from './NoticeView'
import { RunView } from './RunView'
import { UnderstandView } from './UnderstandView'

export type View =
  | { kind: 'menu' }
  | { kind: 'run'; request: RunRequest; fromMenu: boolean }
  | { kind: 'understand'; text: string; language: string }
  | { kind: 'notice' }

function initialView(init: PaletteInit): View {
  switch (init.mode) {
    case 'run':
      return init.request ? { kind: 'run', request: init.request, fromMenu: false } : { kind: 'menu' }
    case 'understand':
      return { kind: 'understand', text: init.text, language: init.language.understandTo }
    case 'notice':
      return { kind: 'notice' }
    default:
      return { kind: 'menu' }
  }
}

export function Palette(): React.JSX.Element {
  const [init, setInit] = useState<PaletteInit | null>(null)
  const [view, setView] = useState<View>({ kind: 'menu' })
  // Cada cambio de vista monta la nueva desde cero (y desmontar la anterior cancela lo que tuviera en curso).
  const [seq, setSeq] = useState(0)
  // El menú recuerda lo escrito y el idioma elegido al volver de un resultado.
  const [menu, setMenu] = useState<MenuState>({ query: '', translateTo: '' })

  const session = useRef<number | null>(null)

  useEffect(
    () =>
      palette.onInit((data) => {
        // El mismo init puede llegar dos veces (evento y consulta al montar).
        if (session.current === data.session) return
        session.current = data.session
        setInit(data)
        setView(initialView(data))
        setMenu({ query: '', translateTo: data.language.translateTo })
        setSeq((s) => s + 1)
      }),
    []
  )

  const go = useCallback((next: View) => {
    setView(next)
    setSeq((s) => s + 1)
  }, [])

  const rootRef = useAutoResize<HTMLDivElement>(palette.resize, init?.session)

  const key = `${init?.session ?? 0}:${seq}`
  return (
    <div ref={rootRef} className={`popup palette palette-${view.kind}`} data-session={init?.session}>
      {init && view.kind === 'menu' && <MenuView key={key} init={init} state={menu} setState={setMenu} go={go} />}
      {init && view.kind === 'run' && (
        <RunView
          key={key}
          init={init}
          request={view.request}
          // Volver al menú tiene sentido siempre que haya texto sobre el que elegir otra acción.
          onBack={init.text.trim() ? () => go({ kind: 'menu' }) : null}
          autoReplace={!view.fromMenu && init.autoReplace}
        />
      )}
      {init && view.kind === 'understand' && <UnderstandView key={key} init={init} text={view.text} language={view.language} />}
      {init && view.kind === 'notice' && <NoticeView key={key} init={init} />}
    </div>
  )
}
