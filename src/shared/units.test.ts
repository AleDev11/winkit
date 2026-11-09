import { describe, expect, it } from 'vitest'
import { convertUnits, findUnit, formatAmount, formatShort } from './units'

const conv = (q: string): number | undefined => convertUnits(q)?.result

describe('convertUnits', () => {
  it('convierte longitudes', () => {
    expect(conv('10 km a mi')).toBeCloseTo(6.21371, 4)
    expect(conv('1 mi en km')).toBeCloseTo(1.609344, 6)
    expect(conv('180 cm en ft')).toBeCloseTo(5.90551, 4)
    expect(conv('12 pulgadas a cm')).toBeCloseTo(30.48, 6)
  })

  it('convierte masas', () => {
    expect(conv('5 kg en lb')).toBeCloseTo(11.0231, 3)
    expect(conv('16 oz to lb')).toBeCloseTo(1, 6)
    expect(conv('2 toneladas a kilos')).toBe(2000)
  })

  it('convierte temperaturas', () => {
    expect(conv('100 f to c')).toBeCloseTo(37.7778, 3)
    expect(conv('0 c a f')).toBe(32)
    expect(conv('25 °C en K')).toBeCloseTo(298.15, 6)
    expect(conv('-40 grados celsius a fahrenheit')).toBe(-40)
  })

  it('acepta coma decimal y nombres en español', () => {
    expect(conv('1,5 h en min')).toBe(90)
    expect(conv('2,5 litros a ml')).toBe(2500)
    expect(conv('1.000,5 m a km')).toBeCloseTo(1.0005, 8)
    expect(conv('1 año en días')).toBeCloseTo(365.2425, 4)
  })

  it('convierte datos con prefijos decimales y binarios', () => {
    expect(conv('2 gb a mb')).toBe(2000)
    expect(conv('1 gib en mib')).toBe(1024)
    expect(conv('8 bits a bytes')).toBe(1)
    expect(conv('100 mbit en mb')).toBe(12.5)
  })

  it('convierte velocidades, volúmenes y superficies', () => {
    expect(conv('100 km/h a mph')).toBeCloseTo(62.1371, 3)
    expect(conv('10 m/s en km/h')).toBeCloseTo(36, 8)
    expect(conv('1 gal a l')).toBeCloseTo(3.785411784, 8)
    expect(conv('1 ha en m2')).toBe(10000)
    expect(conv('1 km² a ha')).toBe(100)
    expect(conv('100 metros cuadrados a pies cuadrados')).toBeCloseTo(1076.391, 2)
  })

  it('distingue «in» como unidad y como conector', () => {
    expect(conv('10 in to cm')).toBeCloseTo(25.4, 8)
    expect(conv('10 in in cm')).toBeCloseTo(25.4, 8)
    expect(conv('5 ft in m')).toBeCloseTo(1.524, 8)
  })

  it('devuelve las unidades encontradas', () => {
    const r = convertUnits('10km a mi')!
    expect(r.value).toBe(10)
    expect(r.from.id).toBe('km')
    expect(r.to.id).toBe('mi')
  })

  it('ignora lo que no es una conversión', () => {
    expect(convertUnits('10 km')).toBeNull()
    expect(convertUnits('km a mi')).toBeNull()
    expect(convertUnits('10 km a kg')).toBeNull()
    expect(convertUnits('3 + 4')).toBeNull()
    expect(convertUnits('2 cervezas a euros')).toBeNull()
    expect(convertUnits('visual studio code')).toBeNull()
  })
})

describe('findUnit', () => {
  it('ignora tildes, mayúsculas y superíndices', () => {
    expect(findUnit('Kilómetros')?.id).toBe('km')
    expect(findUnit('m²')?.id).toBe('m2')
    expect(findUnit('GB')?.id).toBe('GB')
  })
})

describe('formatAmount', () => {
  it('usa el formato español', () => {
    expect(formatAmount(12345.5)).toBe('12.345,5')
    expect(formatAmount(12345.5, false)).toBe('12345,5')
    expect(formatAmount(6.213711922)).toBe('6,213711922')
  })
})

describe('formatShort', () => {
  it('redondea para leer', () => {
    expect(formatShort(6.213711922)).toBe('6,2137')
    expect(formatShort(0.000621371)).toBe('0,0006214')
    expect(formatShort(90)).toBe('90')
  })
})
