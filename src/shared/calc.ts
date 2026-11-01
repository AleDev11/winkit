/**
 * Calculadora del buscador. Un parser propio (nada de eval): números con coma
 * o punto decimal, + − × ÷ ^, paréntesis, porcentajes como en una calculadora
 * de bolsillo ("340 + 15%", "20% de 340") y unas pocas funciones.
 */
type Token = { t: 'num'; v: number } | { t: 'op'; v: string } | { t: 'id'; v: string }

const FUNCS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt,
  raiz: Math.sqrt,
  abs: Math.abs,
  round: Math.round,
  redondear: Math.round,
  floor: Math.floor,
  ceil: Math.ceil,
  ln: Math.log,
  log: Math.log10
}
const CONSTS: Record<string, number> = { pi: Math.PI, e: Math.E }

function normalize(input: string): string {
  let s = input.trim().toLowerCase().replace(/=\s*$/, '')
  s = s.replace(/[×·]/g, '*').replace(/÷/g, '/').replace(/−/g, '-').replace(/√/g, 'sqrt').replace(/raíz/g, 'raiz')
  // "3 x 4": la x entre números es multiplicar.
  s = s.replace(/(\d|\))\s*x\s*(?=[\d(])/g, '$1*')
  // 1.234,5 (es) → 1234.5 ; 3,5 → 3.5 ; 1,234.5 (en) → 1234.5
  if (/\d,\d/.test(s) && /\d\.\d{3}(?!\d)/.test(s) && s.lastIndexOf(',') > s.lastIndexOf('.')) {
    s = s.replace(/(\d)\.(?=\d{3})/g, '$1').replace(/,/g, '.')
  } else if (/\d,\d{3}\.\d/.test(s)) {
    s = s.replace(/(\d),(?=\d{3})/g, '$1')
  } else {
    s = s.replace(/(\d),(?=\d)/g, '$1.')
  }
  // "20% de 340" / "20% of 340"
  s = s.replace(/^(.+?)%\s*(?:de|of)\s+(.+)$/, '($1)/100*($2)')
  return s
}

function tokenize(s: string): Token[] | null {
  const out: Token[] = []
  let i = 0
  while (i < s.length) {
    const c = s[i]
    if (/\s/.test(c)) {
      i++
    } else if (/[\d.]/.test(c)) {
      const m = s.slice(i).match(/^\d*\.?\d+(e[+-]?\d+)?|^\d+\.?/)
      if (!m) return null
      out.push({ t: 'num', v: parseFloat(m[0]) })
      i += m[0].length
    } else if ('+-*/^%()'.includes(c)) {
      out.push({ t: 'op', v: c })
      i++
    } else if (/[a-z]/.test(c)) {
      const m = s.slice(i).match(/^[a-z]+/)!
      out.push({ t: 'id', v: m[0] })
      i += m[0].length
    } else {
      return null
    }
  }
  return out
}

interface Val {
  v: number
  /** Viene de un "N%": en una suma o resta se aplica sobre el operando de la izquierda. */
  pct: boolean
}

function parse(tokens: Token[]): number | null {
  let pos = 0
  const peek = (): Token | undefined => tokens[pos]
  const isOp = (v: string): boolean => peek()?.t === 'op' && peek()!.v === v

  function expr(): Val {
    let left = term()
    while (isOp('+') || isOp('-')) {
      const op = tokens[pos++].v
      const right = term()
      const r = right.pct ? left.v * right.v : right.v
      left = { v: op === '+' ? left.v + r : left.v - r, pct: false }
    }
    return left
  }

  function term(): Val {
    let left = power()
    while (isOp('*') || isOp('/')) {
      const op = tokens[pos++].v
      const right = power()
      left = { v: op === '*' ? left.v * right.v : left.v / right.v, pct: false }
    }
    return left
  }

  function power(): Val {
    const base = unary()
    if (isOp('^')) {
      pos++
      const exp = power()
      return { v: Math.pow(base.v, exp.v), pct: false }
    }
    return base
  }

  function unary(): Val {
    if (isOp('-')) {
      pos++
      const u = unary()
      return { v: -u.v, pct: u.pct }
    }
    if (isOp('+')) {
      pos++
      return unary()
    }
    const p = primary()
    if (isOp('%')) {
      pos++
      return { v: p.v / 100, pct: true }
    }
    return p
  }

  function primary(): Val {
    const tok = tokens[pos++]
    if (!tok) throw new Error('fin inesperado')
    if (tok.t === 'num') return { v: tok.v, pct: false }
    if (tok.t === 'op' && tok.v === '(') {
      const inner = expr()
      if (!isOp(')')) throw new Error('falta )')
      pos++
      return { v: inner.v, pct: false }
    }
    if (tok.t === 'id') {
      if (tok.v in CONSTS) return { v: CONSTS[tok.v], pct: false }
      const fn = FUNCS[tok.v]
      if (fn && isOp('(')) {
        pos++
        const arg = expr()
        if (!isOp(')')) throw new Error('falta )')
        pos++
        return { v: fn(arg.v), pct: false }
      }
    }
    throw new Error('token inesperado')
  }

  try {
    const result = expr()
    if (pos !== tokens.length) return null
    return result.v
  } catch {
    return null
  }
}

/** Resultado si `input` es una operación (no un número suelto); null en cualquier otro caso. */
export function calculate(input: string): number | null {
  const s = normalize(input)
  if (!/\d/.test(s)) return null
  const tokens = tokenize(s)
  if (!tokens) return null
  // Un número solo ("2026") no es una cuenta: que gane la búsqueda.
  const isCalc = tokens.some((t) => (t.t === 'op' && t.v !== '(' && t.v !== ')') || t.t === 'id')
  if (!isCalc) return null
  const v = parse(tokens)
  if (v === null || !Number.isFinite(v)) return null
  // Quita el ruido de coma flotante (0.1 + 0.2).
  return parseFloat(v.toPrecision(12))
}

export function formatNumber(n: number, grouping = true): string {
  return new Intl.NumberFormat('es-ES', { maximumFractionDigits: 10, useGrouping: grouping }).format(n)
}
