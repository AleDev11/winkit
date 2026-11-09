/** Anillo de progreso indeterminado de Fluent. */
export function ProgressRing({ size = 16 }: { size?: number }): React.JSX.Element {
  const stroke = size <= 16 ? 1.75 : 2.5
  const r = (size - stroke) / 2
  return (
    <svg className="ring" width={size} height={size} viewBox={`0 0 ${size} ${size}`} role="progressbar" aria-label="Cargando">
      <circle cx={size / 2} cy={size / 2} r={r} strokeWidth={stroke} pathLength={100} />
    </svg>
  )
}
