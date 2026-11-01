import { describe, expect, test } from 'bun:test'
import { hasChanges, wordDiff } from './diff'

const join = (segs: ReturnType<typeof wordDiff>, keep: 'add' | 'del'): string =>
  (segs ?? []).filter((s) => s.type === 'same' || s.type === keep).map((s) => s.text).join('')

describe('wordDiff', () => {
  test('reconstruye ambos textos', () => {
    const before = 'I has went to the store yesterday, and buyed milk.'
    const after = 'I went to the store yesterday and bought milk.'
    const d = wordDiff(before, after)
    expect(join(d, 'del')).toBe(before)
    expect(join(d, 'add')).toBe(after)
  })

  test('sin cambios', () => {
    const d = wordDiff('Hola, ¿qué tal?', 'Hola, ¿qué tal?')
    expect(d && hasChanges(d)).toBe(false)
  })

  test('marca solo la palabra cambiada', () => {
    const d = wordDiff('Esto es un ejenplo claro.', 'Esto es un ejemplo claro.')
    expect(d?.filter((s) => s.type !== 'same')).toEqual([
      { type: 'del', text: 'ejenplo' },
      { type: 'add', text: 'ejemplo' }
    ])
  })

  test('acentos y apóstrofos cuentan como parte de la palabra', () => {
    const d = wordDiff("don't panic, está", "don't panic, esta")
    expect(d?.filter((s) => s.type !== 'same')).toEqual([
      { type: 'del', text: 'está' },
      { type: 'add', text: 'esta' }
    ])
  })

  test('texto enorme: no compara', () => {
    const big = 'palabra '.repeat(3000)
    expect(wordDiff(big, big + 'más')).toBeNull()
  })
})
