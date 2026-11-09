import { useEffect, useState } from 'react'
import { G, Icon } from '../../components/Icon'
import { launcher } from '../../lib/api'

// Los iconos se piden una vez por app y se guardan en memoria mientras viva la ventana.
const cache = new Map<string, string | null>()
const pending = new Map<string, Promise<string | null>>()

function load(id: string): Promise<string | null> {
  let p = pending.get(id)
  if (!p) {
    p = launcher
      .icon(id)
      .catch(() => null)
      .then((url) => {
        cache.set(id, url)
        pending.delete(id)
        return url
      })
    pending.set(id, p)
  }
  return p
}

export function AppIcon({ id }: { id: string }): React.JSX.Element {
  const [src, setSrc] = useState<string | null | undefined>(() => cache.get(id))

  useEffect(() => {
    if (cache.has(id)) {
      setSrc(cache.get(id))
      return
    }
    let alive = true
    setSrc(undefined)
    void load(id).then((url) => alive && setSrc(url))
    return () => {
      alive = false
    }
  }, [id])

  if (src) return <img className="row-img" src={src} alt="" draggable={false} />
  return <Icon glyph={G.app} className={src === undefined ? 'row-glyph loading' : 'row-glyph'} />
}
