/**
 * Conversor de unidades del buscador: «10 km a mi», «5 kg en lb», «100 f to c»,
 * «1,5 h en min», «2 gb a mb». Nombres y abreviaturas en español e inglés,
 * con coma o punto decimal.
 */

export type UnitCategory = 'length' | 'mass' | 'temperature' | 'volume' | 'speed' | 'data' | 'time' | 'area'

export const CATEGORY_LABELS: Record<UnitCategory, string> = {
  length: 'Longitud',
  mass: 'Masa',
  temperature: 'Temperatura',
  volume: 'Volumen',
  speed: 'Velocidad',
  data: 'Datos',
  time: 'Tiempo',
  area: 'Superficie'
}

export interface Unit {
  id: string
  category: UnitCategory
  /** Lo que se enseña junto al número: «km», «°C», «GB»… */
  symbol: string
  /** Nombre en español, en plural. */
  name: string
  /** Cuántas unidades base es una de esta (m, kg, l, m/s, byte, s, m²). La temperatura va aparte. */
  factor: number
  aliases: string[]
}

function u(id: string, category: UnitCategory, symbol: string, name: string, factor: number, aliases: string[]): Unit {
  return { id, category, symbol, name, factor, aliases }
}

const DAY = 86_400
const YEAR = 365.2425 * DAY

export const UNITS: Unit[] = [
  // Longitud (metro)
  u('mm', 'length', 'mm', 'milímetros', 0.001, ['mm', 'milimetro', 'milimetros', 'millimeter', 'millimeters', 'millimetre', 'millimetres']),
  u('cm', 'length', 'cm', 'centímetros', 0.01, ['cm', 'centimetro', 'centimetros', 'centimeter', 'centimeters', 'centimetre', 'centimetres']),
  u('m', 'length', 'm', 'metros', 1, ['m', 'metro', 'metros', 'meter', 'meters', 'metre', 'metres']),
  u('km', 'length', 'km', 'kilómetros', 1000, ['km', 'kms', 'kilometro', 'kilometros', 'kilometer', 'kilometers', 'kilometre', 'kilometres']),
  u('in', 'length', 'in', 'pulgadas', 0.0254, ['in', 'inch', 'inches', 'pulgada', 'pulgadas', '"', '″']),
  u('ft', 'length', 'ft', 'pies', 0.3048, ['ft', 'foot', 'feet', 'pie', 'pies', "'", '′']),
  u('yd', 'length', 'yd', 'yardas', 0.9144, ['yd', 'yds', 'yard', 'yards', 'yarda', 'yardas']),
  u('mi', 'length', 'mi', 'millas', 1609.344, ['mi', 'mile', 'miles', 'milla', 'millas']),
  u('nmi', 'length', 'nmi', 'millas náuticas', 1852, ['nmi', 'milla nautica', 'millas nauticas', 'nautical mile', 'nautical miles']),

  // Masa (kilogramo)
  u('mg', 'mass', 'mg', 'miligramos', 1e-6, ['mg', 'miligramo', 'miligramos', 'milligram', 'milligrams']),
  u('g', 'mass', 'g', 'gramos', 0.001, ['g', 'gr', 'gramo', 'gramos', 'gram', 'grams', 'gramme', 'grammes']),
  u('kg', 'mass', 'kg', 'kilos', 1, ['kg', 'kgs', 'kilo', 'kilos', 'kilogramo', 'kilogramos', 'kilogram', 'kilograms']),
  u('t', 'mass', 't', 'toneladas', 1000, ['t', 'tonelada', 'toneladas', 'tonne', 'tonnes', 'ton', 'tons']),
  u('oz', 'mass', 'oz', 'onzas', 0.028349523125, ['oz', 'onza', 'onzas', 'ounce', 'ounces']),
  u('lb', 'mass', 'lb', 'libras', 0.45359237, ['lb', 'lbs', 'libra', 'libras', 'pound', 'pounds']),
  u('st', 'mass', 'st', 'stones', 6.35029318, ['st', 'stone', 'stones']),

  // Temperatura (factor sin uso: se convierte con fórmulas)
  u('c', 'temperature', '°C', 'grados Celsius', 1, ['c', '°c', 'ºc', 'celsius', 'centigrados', 'grados celsius', 'grados centigrados', 'degrees celsius']),
  u('f', 'temperature', '°F', 'grados Fahrenheit', 1, ['f', '°f', 'ºf', 'fahrenheit', 'grados fahrenheit', 'degrees fahrenheit']),
  u('k', 'temperature', 'K', 'kelvin', 1, ['k', 'kelvin', 'kelvins']),

  // Volumen (litro)
  u('ml', 'volume', 'ml', 'mililitros', 0.001, ['ml', 'mililitro', 'mililitros', 'milliliter', 'milliliters', 'millilitre', 'millilitres', 'cm3', 'cc']),
  u('cl', 'volume', 'cl', 'centilitros', 0.01, ['cl', 'centilitro', 'centilitros', 'centiliter', 'centiliters']),
  u('dl', 'volume', 'dl', 'decilitros', 0.1, ['dl', 'decilitro', 'decilitros', 'deciliter', 'deciliters']),
  u('l', 'volume', 'l', 'litros', 1, ['l', 'lt', 'litro', 'litros', 'liter', 'liters', 'litre', 'litres']),
  u('m3', 'volume', 'm³', 'metros cúbicos', 1000, ['m3', 'metro cubico', 'metros cubicos', 'cubic meter', 'cubic meters']),
  u('tsp', 'volume', 'cdta.', 'cucharaditas', 0.00492892159375, ['tsp', 'cucharadita', 'cucharaditas', 'teaspoon', 'teaspoons']),
  u('tbsp', 'volume', 'cda.', 'cucharadas', 0.01478676478125, ['tbsp', 'cucharada', 'cucharadas', 'tablespoon', 'tablespoons']),
  u('floz', 'volume', 'fl oz', 'onzas líquidas', 0.0295735295625, ['floz', 'fl oz', 'onza liquida', 'onzas liquidas', 'fluid ounce', 'fluid ounces']),
  u('cup', 'volume', 'tazas', 'tazas', 0.2365882365, ['cup', 'cups', 'taza', 'tazas']),
  u('pt', 'volume', 'pt', 'pintas', 0.473176473, ['pt', 'pinta', 'pintas', 'pint', 'pints']),
  u('qt', 'volume', 'qt', 'cuartos de galón', 0.946352946, ['qt', 'quart', 'quarts', 'cuarto de galon', 'cuartos de galon']),
  u('gal', 'volume', 'gal', 'galones', 3.785411784, ['gal', 'gals', 'galon', 'galones', 'gallon', 'gallons']),

  // Velocidad (metro por segundo)
  u('mps', 'speed', 'm/s', 'metros por segundo', 1, ['m/s', 'mps', 'metros por segundo', 'meters per second']),
  u('kmh', 'speed', 'km/h', 'kilómetros por hora', 1 / 3.6, ['km/h', 'kmh', 'kph', 'kmph', 'km por hora', 'kilometros por hora', 'kilometers per hour']),
  u('mph', 'speed', 'mph', 'millas por hora', 0.44704, ['mph', 'mi/h', 'millas por hora', 'miles per hour']),
  u('kn', 'speed', 'kn', 'nudos', 1852 / 3600, ['kn', 'kt', 'kts', 'nudo', 'nudos', 'knot', 'knots']),
  u('fps', 'speed', 'ft/s', 'pies por segundo', 0.3048, ['ft/s', 'fps', 'pies por segundo', 'feet per second']),

  // Datos (byte). Con prefijos decimales (kB, MB…) y binarios (KiB, MiB…).
  u('bit', 'data', 'bit', 'bits', 1 / 8, ['bit', 'bits']),
  u('B', 'data', 'B', 'bytes', 1, ['b', 'byte', 'bytes', 'octeto', 'octetos']),
  u('kB', 'data', 'kB', 'kilobytes', 1e3, ['kb', 'kilobyte', 'kilobytes', 'kbyte', 'kbytes']),
  u('MB', 'data', 'MB', 'megabytes', 1e6, ['mb', 'megabyte', 'megabytes', 'mega', 'megas']),
  u('GB', 'data', 'GB', 'gigabytes', 1e9, ['gb', 'gigabyte', 'gigabytes', 'giga', 'gigas']),
  u('TB', 'data', 'TB', 'terabytes', 1e12, ['tb', 'terabyte', 'terabytes', 'tera', 'teras']),
  u('PB', 'data', 'PB', 'petabytes', 1e15, ['pb', 'petabyte', 'petabytes']),
  u('KiB', 'data', 'KiB', 'kibibytes', 1024, ['kib', 'kibibyte', 'kibibytes']),
  u('MiB', 'data', 'MiB', 'mebibytes', 1024 ** 2, ['mib', 'mebibyte', 'mebibytes']),
  u('GiB', 'data', 'GiB', 'gibibytes', 1024 ** 3, ['gib', 'gibibyte', 'gibibytes']),
  u('TiB', 'data', 'TiB', 'tebibytes', 1024 ** 4, ['tib', 'tebibyte', 'tebibytes']),
  u('kbit', 'data', 'kbit', 'kilobits', 1e3 / 8, ['kbit', 'kbits', 'kilobit', 'kilobits']),
  u('Mbit', 'data', 'Mbit', 'megabits', 1e6 / 8, ['mbit', 'mbits', 'megabit', 'megabits']),
  u('Gbit', 'data', 'Gbit', 'gigabits', 1e9 / 8, ['gbit', 'gbits', 'gigabit', 'gigabits']),

  // Tiempo (segundo)
  u('ms', 'time', 'ms', 'milisegundos', 0.001, ['ms', 'milisegundo', 'milisegundos', 'millisecond', 'milliseconds']),
  u('s', 'time', 's', 'segundos', 1, ['s', 'seg', 'segs', 'sec', 'secs', 'segundo', 'segundos', 'second', 'seconds']),
  u('min', 'time', 'min', 'minutos', 60, ['min', 'mins', 'minuto', 'minutos', 'minute', 'minutes']),
  u('h', 'time', 'h', 'horas', 3600, ['h', 'hr', 'hrs', 'hora', 'horas', 'hour', 'hours']),
  u('d', 'time', 'd', 'días', DAY, ['d', 'dia', 'dias', 'day', 'days']),
  u('wk', 'time', 'sem.', 'semanas', 7 * DAY, ['sem', 'semana', 'semanas', 'wk', 'wks', 'week', 'weeks']),
  u('mo', 'time', 'meses', 'meses', YEAR / 12, ['mes', 'meses', 'month', 'months']),
  u('yr', 'time', 'años', 'años', YEAR, ['año', 'años', 'ano', 'anos', 'yr', 'yrs', 'year', 'years']),

  // Superficie (metro cuadrado)
  u('mm2', 'area', 'mm²', 'milímetros cuadrados', 1e-6, ['mm2', 'milimetro cuadrado', 'milimetros cuadrados', 'square millimeter', 'square millimeters']),
  u('cm2', 'area', 'cm²', 'centímetros cuadrados', 1e-4, ['cm2', 'centimetro cuadrado', 'centimetros cuadrados', 'square centimeter', 'square centimeters']),
  u('m2', 'area', 'm²', 'metros cuadrados', 1, ['m2', 'metro cuadrado', 'metros cuadrados', 'square meter', 'square meters', 'sq m']),
  u('km2', 'area', 'km²', 'kilómetros cuadrados', 1e6, ['km2', 'kilometro cuadrado', 'kilometros cuadrados', 'square kilometer', 'square kilometers', 'sq km']),
  u('ha', 'area', 'ha', 'hectáreas', 1e4, ['ha', 'hectarea', 'hectareas', 'hectare', 'hectares']),
  u('in2', 'area', 'in²', 'pulgadas cuadradas', 0.00064516, ['in2', 'sq in', 'pulgada cuadrada', 'pulgadas cuadradas', 'square inch', 'square inches']),
  u('ft2', 'area', 'ft²', 'pies cuadrados', 0.09290304, ['ft2', 'sq ft', 'sqft', 'pie cuadrado', 'pies cuadrados', 'square foot', 'square feet']),
  u('yd2', 'area', 'yd²', 'yardas cuadradas', 0.83612736, ['yd2', 'sq yd', 'yarda cuadrada', 'yardas cuadradas', 'square yard', 'square yards']),
  u('mi2', 'area', 'mi²', 'millas cuadradas', 2589988.110336, ['mi2', 'sq mi', 'milla cuadrada', 'millas cuadradas', 'square mile', 'square miles']),
  u('ac', 'area', 'ac', 'acres', 4046.8564224, ['ac', 'acre', 'acres'])
]

/** Normaliza un nombre de unidad: minúsculas, sin tildes, superíndices como dígitos y espacios simples. */
function normUnit(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/²/g, '2')
    .replace(/³/g, '3')
    .replace(/\s+/g, ' ')
    .replace(/\.$/, '')
    .trim()
}

const BY_ALIAS = new Map<string, Unit>()
for (const unit of UNITS) {
  for (const a of unit.aliases) {
    const key = normUnit(a)
    if (!BY_ALIAS.has(key)) BY_ALIAS.set(key, unit)
  }
}

export function findUnit(name: string): Unit | null {
  const key = normUnit(name)
  return BY_ALIAS.get(key) ?? BY_ALIAS.get(key.replace(/^(grados|degrees) /, '')) ?? null
}

/** «1.234,5» / «1,234.5» / «1,5» / «1.5» → número. */
function parseAmount(raw: string): number | null {
  let s = raw.replace(/\s/g, '').replace(/^\+/, '').replace(/−/g, '-')
  const lastComma = s.lastIndexOf(',')
  const lastDot = s.lastIndexOf('.')
  if (lastComma >= 0 && lastDot >= 0) {
    // El último separador es el decimal; el otro, de miles.
    if (lastComma > lastDot) s = s.replace(/\./g, '').replace(',', '.')
    else s = s.replace(/,/g, '')
  } else if (lastComma >= 0) {
    s = s.replace(',', '.')
  }
  if (!/^-?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(s)) return null
  const n = parseFloat(s)
  return Number.isFinite(n) ? n : null
}

function toCelsius(v: number, from: string): number {
  if (from === 'f') return ((v - 32) * 5) / 9
  if (from === 'k') return v - 273.15
  return v
}
function fromCelsius(c: number, to: string): number {
  if (to === 'f') return (c * 9) / 5 + 32
  if (to === 'k') return c + 273.15
  return c
}

export function convertValue(value: number, from: Unit, to: Unit): number {
  if (from.category === 'temperature') return fromCelsius(toCelsius(value, from.id), to.id)
  return (value * from.factor) / to.factor
}

export interface Conversion {
  value: number
  from: Unit
  to: Unit
  result: number
}

const AMOUNT = String.raw`([-+−]?(?:\d[\d.,]*\d|\d)(?:e[+-]?\d+)?)`
// Conectores entre las dos unidades. «in» también es pulgadas: se prueban todos los cortes posibles.
const CONNECTOR = /\s+(?:a|al|en|to|in|into|como|as|=|->|→)\s+/gi

/** Convierte «10 km a mi». Null si no es una conversión entre unidades de la misma magnitud. */
export function convertUnits(input: string): Conversion | null {
  const q = input.trim().replace(/\s*=\s*$/, '').replace(/\?$/, '')
  if (q.length < 4 || q.length > 80) return null
  const m = q.match(new RegExp(`^${AMOUNT}\\s*(.+)$`, 'i'))
  if (!m) return null
  const value = parseAmount(m[1])
  if (value === null) return null
  const rest = ` ${m[2]}`
  // Los cortes pueden solaparse (« in to »): se avanza de uno en uno.
  const re = new RegExp(CONNECTOR.source, 'gi')
  for (let c = re.exec(rest); c; c = re.exec(rest)) {
    re.lastIndex = c.index + 1
    const left = rest.slice(0, c.index).trim()
    const right = rest.slice(c.index + c[0].length).trim()
    if (!left || !right) continue
    const from = findUnit(left)
    const to = findUnit(right)
    if (from && to && from.category === to.category) {
      return { value, from, to, result: clean(convertValue(value, from, to)) }
    }
  }
  return null
}

/** Quita el ruido de coma flotante (0,30000000000000004). */
function clean(n: number): number {
  return n === 0 ? 0 : parseFloat(n.toPrecision(12))
}

/** Número con formato español y como mucho 10 cifras significativas. */
export function formatAmount(n: number, grouping = true): string {
  const abs = Math.abs(n)
  if (abs !== 0 && (abs >= 1e15 || abs < 1e-9)) {
    return n.toExponential(6).replace(/\.?0+e/, 'e').replace('.', ',')
  }
  return new Intl.NumberFormat('es-ES', { maximumSignificantDigits: 10, useGrouping: grouping }).format(n)
}

/** Para leer de un vistazo: 4 decimales como mucho, o 4 cifras significativas si es menor que 1. */
export function formatShort(n: number): string {
  const abs = Math.abs(n)
  if (abs !== 0 && (abs >= 1e15 || abs < 1e-9)) return formatAmount(n)
  const opts: Intl.NumberFormatOptions = abs >= 1 || abs === 0 ? { maximumFractionDigits: 4 } : { maximumSignificantDigits: 4 }
  return new Intl.NumberFormat('es-ES', opts).format(n)
}
