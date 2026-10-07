import { builtinTheme } from '@termysh/core'

/**
 * Terminal colors. Any CSS color works (`#rgb`, `#rrggbbaa`, `rgb()`, names).
 * Field names match xterm.js `ITheme`, so xterm themes can be reused as is.
 */
export interface Theme {
  foreground?: string
  background?: string
  cursor?: string
  /** Text color under a block cursor. */
  cursorAccent?: string
  selectionBackground?: string
  /** Defaults to the cell's own foreground. */
  selectionForeground?: string
  /** Selection color while the terminal is not focused. */
  selectionInactiveBackground?: string
  black?: string
  red?: string
  green?: string
  yellow?: string
  blue?: string
  magenta?: string
  cyan?: string
  white?: string
  brightBlack?: string
  brightRed?: string
  brightGreen?: string
  brightYellow?: string
  brightBlue?: string
  brightMagenta?: string
  brightCyan?: string
  brightWhite?: string
  /** Palette entries 16-255; missing entries use the xterm 256-color cube. */
  extendedAnsi?: string[]
  /** Scrollbar thumb color. */
  scrollbarThumb?: string
}

/**
 * A bundled Termy theme id (`termy`, `termy-light`, `tokyo-night`, `dracula`,
 * `catppuccin-mocha`, ...), a full theme object, or a bundled theme with
 * overrides: `{ extends: 'dracula', cursor: '#ff0' }`.
 */
export type ThemeInput = string | (Theme & { extends?: string })

/** A color as packed `0xRRGGBBAA` plus its CSS form. */
export interface Rgba {
  readonly value: number
  readonly css: string
}

export interface ResolvedTheme {
  foreground: Rgba
  background: Rgba
  cursor: Rgba
  cursorAccent: Rgba
  selectionBackground: Rgba
  selectionForeground: Rgba | undefined
  selectionInactiveBackground: Rgba
  scrollbarThumb: Rgba
  /** 256 entries. */
  palette: Rgba[]
}

export const DEFAULT_THEME = 'termy'

const ANSI_KEYS = [
  'black',
  'red',
  'green',
  'yellow',
  'blue',
  'magenta',
  'cyan',
  'white',
  'brightBlack',
  'brightRed',
  'brightGreen',
  'brightYellow',
  'brightBlue',
  'brightMagenta',
  'brightCyan',
  'brightWhite',
] as const

export function rgba(r: number, g: number, b: number, a = 255): Rgba {
  const value = (((r & 255) << 24) | ((g & 255) << 16) | ((b & 255) << 8) | (a & 255)) >>> 0
  const css = a === 255 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${(a / 255).toFixed(3)})`
  return { value, css }
}

export function channels(color: Rgba): [number, number, number, number] {
  const v = color.value
  return [v >>> 24, (v >>> 16) & 255, (v >>> 8) & 255, v & 255]
}

export function fromRgb24(rgb: number): Rgba {
  return rgba((rgb >>> 16) & 255, (rgb >>> 8) & 255, rgb & 255)
}

let probe: CanvasRenderingContext2D | null | undefined

/** Parse any CSS color. Hex and rgb() are parsed directly; others go through canvas. */
export function parseColor(input: string): Rgba | undefined {
  const value = input.trim()
  const hex = /^#([0-9a-f]{3,8})$/i.exec(value)?.[1]
  if (hex && [3, 4, 6, 8].includes(hex.length)) {
    const full = hex.length <= 4 ? [...hex].map((c) => c + c).join('') : hex
    const n = (i: number): number => Number.parseInt(full.slice(i, i + 2), 16)
    return rgba(n(0), n(2), n(4), full.length === 8 ? n(6) : 255)
  }
  const fn = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+%?))?\s*\)$/i.exec(value)
  if (fn) {
    const alpha = fn[4] === undefined ? 1 : fn[4].endsWith('%') ? Number.parseFloat(fn[4]) / 100 : Number(fn[4])
    return rgba(Number(fn[1]), Number(fn[2]), Number(fn[3]), Math.round(alpha * 255))
  }
  if (typeof document === 'undefined') return undefined
  probe ??= document.createElement('canvas').getContext('2d')
  if (!probe) return undefined
  probe.fillStyle = '#000'
  probe.fillStyle = value
  const normalized = String(probe.fillStyle)
  return normalized === value ? undefined : parseColor(normalized)
}

/** Linear blend of `top` over `bottom` with `amount` in 0..1. */
export function blend(bottom: Rgba, top: Rgba, amount: number): Rgba {
  const [r1, g1, b1, a1] = channels(bottom)
  const [r2, g2, b2] = channels(top)
  const mix = (a: number, b: number): number => Math.round(a + (b - a) * amount)
  return rgba(mix(r1, r2), mix(g1, g2), mix(b1, b2), a1)
}

export function withAlpha(color: Rgba, alpha: number): Rgba {
  const [r, g, b] = channels(color)
  return rgba(r, g, b, Math.round(alpha * 255))
}

function luminance(color: Rgba): number {
  const [r, g, b] = channels(color).map((c) => {
    const s = c / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }) as [number, number, number]
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

export function contrastRatio(a: Rgba, b: Rgba): number {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number]
  return (l1 + 0.05) / (l2 + 0.05)
}

/** Move `fg` toward black or white until it reaches `ratio` against `bg`. */
export function ensureContrast(fg: Rgba, bg: Rgba, ratio: number): Rgba {
  if (ratio <= 1 || contrastRatio(fg, bg) >= ratio) return fg
  const target = luminance(bg) > 0.5 ? rgba(0, 0, 0) : rgba(255, 255, 255)
  for (let step = 0.1; step <= 1; step += 0.1) {
    const candidate = blend(fg, target, step)
    if (contrastRatio(candidate, bg) >= ratio) return candidate
  }
  return target
}

/** The xterm 256-color palette above the 16 theme colors. */
function extendedPalette(index: number): Rgba {
  if (index < 232) {
    const n = index - 16
    const level = (v: number): number => (v === 0 ? 0 : 55 + v * 40)
    return rgba(level(Math.floor(n / 36)), level(Math.floor(n / 6) % 6), level(n % 6))
  }
  const gray = 8 + (index - 232) * 10
  return rgba(gray, gray, gray)
}

function baseTheme(id: string): Theme {
  const colors = builtinTheme(id) ?? builtinTheme(DEFAULT_THEME)
  if (!colors) return {}
  const theme: Theme = {
    foreground: colors.foreground,
    background: colors.background,
    cursor: colors.cursor,
  }
  ANSI_KEYS.forEach((key, index) => {
    theme[key] = colors.ansi[index]
  })
  return theme
}

export function resolveTheme(input: ThemeInput | undefined): ResolvedTheme {
  const overrides: Theme & { extends?: string } = typeof input === 'string' ? {} : (input ?? {})
  const baseId = typeof input === 'string' ? input : (overrides.extends ?? DEFAULT_THEME)
  const theme: Theme = { ...baseTheme(baseId), ...stripUndefined(overrides) }
  const color = (value: string | undefined, fallback: Rgba): Rgba => (value && parseColor(value)) || fallback

  const foreground = color(theme.foreground, rgba(0xe5, 0xe5, 0xe5))
  const background = color(theme.background, rgba(0, 0, 0))
  const palette: Rgba[] = []
  for (let index = 0; index < 256; index++) {
    if (index < 16) palette.push(color(theme[ANSI_KEYS[index]!], extendedPalette(16)))
    else palette.push(color(theme.extendedAnsi?.[index - 16], extendedPalette(index)))
  }
  const selectionBackground = color(theme.selectionBackground, withAlpha(foreground, 0.3))
  return {
    foreground,
    background,
    cursor: color(theme.cursor, foreground),
    cursorAccent: color(theme.cursorAccent, background),
    selectionBackground,
    selectionForeground: theme.selectionForeground ? parseColor(theme.selectionForeground) : undefined,
    selectionInactiveBackground: color(theme.selectionInactiveBackground, withAlpha(foreground, 0.15)),
    scrollbarThumb: color(theme.scrollbarThumb, withAlpha(foreground, 0.35)),
    palette,
  }
}

/** `[fg, bg, cursor, ansi0..15]` as 0xRRGGBB for the engine's color query replies. */
export function queryColors(theme: ResolvedTheme): number[] {
  const rgb = (color: Rgba): number => color.value >>> 8
  return [theme.foreground, theme.background, theme.cursor, ...theme.palette.slice(0, 16)].map(rgb)
}

function stripUndefined<T extends object>(value: T): Partial<T> {
  return Object.fromEntries(Object.entries(value).filter(([, v]) => v !== undefined)) as Partial<T>
}
