import { assertInitialized } from './init.ts'
import { themeColors, themeIds } from './wasm/termy_wasm.js'

/** A fully resolved 16-color terminal theme as `#rrggbb` strings. */
export interface TermyThemeColors {
  foreground: string
  background: string
  cursor: string
  /** ANSI 0-15: black, red, green, yellow, blue, magenta, cyan, white, then bright. */
  ansi: string[]
}

/** Ids of the themes bundled with Termy, e.g. `termy`, `tokyo-night`, `dracula`. */
export function builtinThemeIds(): string[] {
  assertInitialized()
  return themeIds()
}

/** A bundled Termy theme by id (case and separator insensitive). */
export function builtinTheme(id: string): TermyThemeColors | undefined {
  assertInitialized()
  const colors = themeColors(id)
  if (!colors) return undefined
  const hex = (value: number | undefined): string => `#${(value ?? 0).toString(16).padStart(6, '0')}`
  return {
    foreground: hex(colors[0]),
    background: hex(colors[1]),
    cursor: hex(colors[2]),
    ansi: Array.from(colors.subarray(3, 19), hex),
  }
}
