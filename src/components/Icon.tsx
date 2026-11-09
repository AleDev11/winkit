/**
 * Glifos de Segoe Fluent Icons (con Segoe MDL2 Assets de respaldo), la fuente
 * de iconos de Windows 11. Nada de paquetes de iconos: así se ven idénticos al sistema.
 */
export const G = {
  add: '',
  app: '',
  back: '',
  book: '',
  briefcase: '',
  calculator: '',
  cancel: '',
  check: '',
  checkCircle: '',
  chevronDown: '',
  chevronRight: '',
  chip: '',
  close: '',
  color: '',
  copy: '',
  delete: '',
  edit: '',
  enter: '',
  error: '',
  eyedropper: '',
  globe: '',
  info: '',
  key: '',
  keyboard: '',
  language: '',
  lightning: '',
  message: '',
  openExternal: '',
  power: '',
  record: '',
  refresh: '',
  ruler: '',
  search: '',
  scissors: '',
  selectAll: '',
  settings: '',
  smile: '',
  sparkle: '',
  spelling: '',
  swap: '',
  translate: '',
  warning: '',
  // Clips
  monitor: '',
  equalizer: '',
  folder: '',
  hardDrive: '',
  history: '',
  microphone: '',
  save: '',
  speed: '',
  video: '',
  volume: ''
} as const

export function Icon({ glyph, size, className }: { glyph: string; size?: number; className?: string }): React.JSX.Element {
  return (
    <span className={className ? `icon ${className}` : 'icon'} style={size ? { fontSize: size } : undefined} aria-hidden="true">
      {glyph}
    </span>
  )
}
