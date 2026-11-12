import { useEffect } from 'react'
import { G, Icon } from '../../components/Icon'
import { palette, ui } from '../../lib/api'
import type { PaletteInit } from '../../shared/types'

/** Aviso breve. Rust lo oculta solo, salvo los que llevan a Ajustes. */
export function NoticeView({ init }: { init: PaletteInit }): React.JSX.Element {
  const notice = init.notice ?? { message: '', openSettings: false, color: null }

  useEffect(() => {
    const on = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') void palette.close()
      else if (e.key === 'Enter' && notice.openSettings) void ui.openSettings()
    }
    window.addEventListener('keydown', on)
    return () => window.removeEventListener('keydown', on)
  }, [notice.openSettings])

  // Un clic en cualquier parte lo cierra: los avisos salen sin foco, así que
  // Esc solo funciona si antes se ha hecho clic en ellos.
  return (
    <div
      className={notice.openSettings ? 'notice-view attention' : 'notice-view'}
      role="status"
      title="Clic para cerrar"
      onClick={() => void palette.close()}
    >
      {notice.color ? (
        <span className="notice-swatch" style={{ background: notice.color }} aria-hidden="true" />
      ) : (
        <span className="notice-icon">
          <Icon glyph={notice.openSettings ? G.warning : G.info} />
        </span>
      )}
      <span className="notice-message">{notice.message}</span>
      {notice.openSettings && (
        <button
          className="btn accent sm"
          onClick={(e) => {
            e.stopPropagation()
            void ui.openSettings()
          }}
        >
          Abrir ajustes
        </button>
      )}
      <button className="btn subtle sm icon-only notice-close" aria-label="Cerrar" onClick={() => void palette.close()}>
        <Icon glyph={G.close} />
      </button>
    </div>
  )
}
