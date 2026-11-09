import { describe, expect, test } from 'vitest'
import { calculate, formatNumber } from './calc'

describe('calculate', () => {
  test.each([
    ['2+2', 4],
    ['15*3+7', 52],
    ['(1+2)*3', 9],
    ['2^10', 1024],
    ['-3 + 5', 2],
    ['10 / 4', 2.5],
    ['0.1 + 0.2', 0.3],
    ['3,5 * 2', 7],
    ['1.234,5 + 1', 1235.5],
    ['1,234.5 + 1', 1235.5],
    ['3 x 4', 12],
    ['6 ÷ 3', 2],
    ['20% de 340', 68],
    ['20% of 340', 68],
    ['340 + 15%', 391],
    ['200 - 10%', 180],
    ['50 * 10%', 5],
    ['sqrt(16)', 4],
    ['√(9) + 1', 4],
    ['2 * pi', 6.28318530718],
    ['8+2=', 10]
  ])('%s = %d', (input, expected) => {
    expect(calculate(input)).toBeCloseTo(expected, 9)
  })

  test.each(['2026', 'hola', 'discord', '2+', '(1+2', '1/0', 'spotify 2', ''])('%s no es una cuenta', (input) => {
    expect(calculate(input)).toBeNull()
  })
})

describe('formatNumber', () => {
  test('formato español', () => {
    expect(formatNumber(0.5)).toBe('0,5')
    expect(formatNumber(1234567.25)).toBe('1.234.567,25')
    expect(formatNumber(1234.5, false)).toBe('1234,5')
  })
})
