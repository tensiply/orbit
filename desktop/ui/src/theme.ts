// ── Terminal palettes ─────────────────────────────────────────────────────────
// xterm.js requires hex — these cannot use CSS variables

export const TERMINAL_THEME_DARK = {
  background:          '#0a0a0b',
  foreground:          '#f4f4f5',
  cursor:              '#a1a1aa',
  cursorAccent:        '#09090b',
  selectionBackground: '#3f3f4660',
  black:               '#18181b',
  red:                 '#ef4444',
  green:               '#22c55e',
  yellow:              '#eab308',
  blue:                '#3b82f6',
  magenta:             '#a855f7',
  cyan:                '#06b6d4',
  white:               '#d4d4d8',
  brightBlack:         '#52525b',
  brightRed:           '#f87171',
  brightGreen:         '#4ade80',
  brightYellow:        '#fde047',
  brightBlue:          '#60a5fa',
  brightMagenta:       '#c084fc',
  brightCyan:          '#22d3ee',
  brightWhite:         '#f4f4f5',
}

export const TERMINAL_THEME_LIGHT = {
  background:          '#f9f9fc',
  foreground:          '#1a1a28',
  cursor:              '#6b6b80',
  cursorAccent:        '#f5f5f8',
  selectionBackground: '#9090a040',
  black:               '#1a1a28',
  red:                 '#dc2626',
  green:               '#16a34a',
  yellow:              '#b45309',
  blue:                '#2563eb',
  magenta:             '#7c3aed',
  cyan:                '#0891b2',
  white:               '#a0a0b4',
  brightBlack:         '#6b6b80',
  brightRed:           '#ef4444',
  brightGreen:         '#22c55e',
  brightYellow:        '#d97706',
  brightBlue:          '#3b82f6',
  brightMagenta:       '#8b5cf6',
  brightCyan:          '#06b6d4',
  brightWhite:         '#9090a8',
}

// ── CSS variable → hex ────────────────────────────────────────────────────────

export function cssVarToHex(varName: string): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(varName).trim()
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = 1
  const ctx = canvas.getContext('2d')!
  ctx.fillStyle = raw
  ctx.fillRect(0, 0, 1, 1)
  const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data
  return '#' + [r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('')
}

// ── Semantic tokens ───────────────────────────────────────────────────────────
// Inline style values that cannot be expressed as CSS variables or Tailwind
// classes, organized by domain.

export const STATUS_COLORS = {
  working: '#eab308',
  active:  '#22c55e',
} as const

export const ARCH_KIND_COLORS: Record<string, string> = {
  Services:       '#3b82f6',
  Databases:      '#10b981',
  Integrations:   '#8b5cf6',
  Infrastructure: '#f97316',
  APIs:           '#06b6d4',
  Pipelines:      '#f59e0b',
  Secrets:        '#ef4444',
  IAM:            '#ec4899',
  Teams:          '#6b7280',
  _default:       '#52525b',
}
