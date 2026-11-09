import { useId } from 'react'

/**
 * El icono de Winkit: tres casillas (Windows, el «kit») y una chispa (Claude)
 * en la cuarta. Mismo dibujo que resources/logo.svg, en SVG para que se vea
 * nítido a cualquier tamaño.
 */
export function Logo({ size = 20 }: { size?: number }): React.JSX.Element {
  // Ids únicos: puede haber varios logos en la misma página.
  const id = useId().replace(/:/g, '')
  const tile = `${id}-tile`
  const cell = `${id}-cell`
  const spark = `${id}-spark`
  return (
    <svg className="logo" width={size} height={size} viewBox="32 32 960 960" aria-hidden="true">
      <defs>
        <linearGradient id={tile} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#2B3442" />
          <stop offset="1" stopColor="#151A22" />
        </linearGradient>
        <linearGradient id={cell} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#FFFFFF" />
          <stop offset="1" stopColor="#DCE3EC" />
        </linearGradient>
        <linearGradient id={spark} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#7AF0DD" />
          <stop offset="1" stopColor="#14B8A6" />
        </linearGradient>
      </defs>
      <rect x="32" y="32" width="960" height="960" rx="216" fill={`url(#${tile})`} />
      <rect x="216" y="216" width="264" height="264" rx="64" fill={`url(#${cell})`} />
      <rect x="544" y="216" width="264" height="264" rx="64" fill={`url(#${cell})`} opacity="0.82" />
      <rect x="216" y="544" width="264" height="264" rx="64" fill={`url(#${cell})`} opacity="0.64" />
      <path
        d="M676 528 C694 618 722 646 804 676 C722 706 694 734 676 824 C658 734 630 706 548 676 C630 646 658 618 676 528 Z"
        fill={`url(#${spark})`}
      />
    </svg>
  )
}
