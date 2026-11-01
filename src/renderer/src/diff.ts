export type Segment = { type: 'same' | 'add' | 'del'; text: string }

/** Palabras, espacios y signos sueltos: la unidad mínima que se marca como cambiada. */
function tokenize(text: string): string[] {
  return text.match(/\s+|[\p{L}\p{N}_'’-]+|[^\s\p{L}\p{N}]/gu) ?? []
}

const MAX_CELLS = 4_000_000

/** Diff por palabras (LCS). Devuelve null si el texto es demasiado largo para compararlo. */
export function wordDiff(before: string, after: string): Segment[] | null {
  const a = tokenize(before)
  const b = tokenize(after)
  const n = a.length
  const m = b.length
  if (n * m > MAX_CELLS) return null

  // lcs[i][j] = longitud de la subsecuencia común de a[i..] y b[j..], en una tabla plana.
  const w = m + 1
  const lcs = new Uint32Array((n + 1) * w)
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i * w + j] = a[i] === b[j] ? lcs[(i + 1) * w + j + 1] + 1 : Math.max(lcs[(i + 1) * w + j], lcs[i * w + j + 1])
    }
  }

  const out: Segment[] = []
  const push = (type: Segment['type'], text: string): void => {
    const last = out[out.length - 1]
    if (last?.type === type) last.text += text
    else out.push({ type, text })
  }
  let i = 0
  let j = 0
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      push('same', a[i])
      i++
      j++
    } else if (lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
      push('del', a[i++])
    } else {
      push('add', b[j++])
    }
  }
  while (i < n) push('del', a[i++])
  while (j < m) push('add', b[j++])
  return absorbSpaces(out)
}

/**
 * Un espacio "igual" entre dos cambios hace que una frase reescrita se vea como
 * confeti de palabras sueltas. Se funde en el cambio para leerlo de un tirón.
 */
function absorbSpaces(segs: Segment[]): Segment[] {
  const out: Segment[] = []
  for (let k = 0; k < segs.length; k++) {
    const s = segs[k]
    const prev = segs[k - 1]
    const next = segs[k + 1]
    if (s.type === 'same' && /^\s+$/.test(s.text) && prev && next && prev.type !== 'same' && next.type !== 'same') {
      out.push({ type: 'del', text: s.text }, { type: 'add', text: s.text })
    } else {
      out.push({ ...s })
    }
  }
  // Agrupa: todo lo borrado seguido y todo lo añadido seguido.
  const grouped: Segment[] = []
  let k = 0
  while (k < out.length) {
    if (out[k].type === 'same') {
      grouped.push(out[k++])
      continue
    }
    let del = ''
    let add = ''
    while (k < out.length && out[k].type !== 'same') {
      if (out[k].type === 'del') del += out[k].text
      else add += out[k].text
      k++
    }
    if (del) grouped.push({ type: 'del', text: del })
    if (add) grouped.push({ type: 'add', text: add })
  }
  return grouped
}

export function hasChanges(segs: Segment[]): boolean {
  return segs.some((s) => s.type !== 'same')
}
