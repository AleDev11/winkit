/** La «W» del icono de la app, en SVG para que se vea nítida a cualquier tamaño. */
export function Logo({ size = 20 }: { size?: number }): React.JSX.Element {
  return (
    <svg className="logo" width={size} height={size} viewBox="0 0 100 100" aria-hidden="true">
      <defs>
        <linearGradient id="winkit-logo" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#f6b26b" />
          <stop offset="1" stopColor="#e07846" />
        </linearGradient>
      </defs>
      <rect width="100" height="100" rx="24" fill="url(#winkit-logo)" />
      <polyline
        points="22,31 36,70 50,40 64,70 78,31"
        fill="none"
        stroke="#1c1410"
        strokeWidth="11"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}
