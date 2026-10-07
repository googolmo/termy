/**
 * Flat cell layout produced by `TermyCore.readRows`. Mirrors
 * `crates/wasm/src/cells.rs`; each cell is `CELL_STRIDE` u32 slots.
 */
export const CELL_STRIDE = 6

export const Slot = {
  Text: 0,
  Foreground: 1,
  Background: 2,
  UnderlineColor: 3,
  Style: 4,
  Link: 5,
} as const

/** Slot 0 flag: the low bits index the read's string table (grapheme clusters). */
export const STRING_FLAG = 0x8000_0000

/** Style bits in slot 4 (low 16 bits). */
export const Attr = {
  Bold: 1,
  Dim: 2,
  Italic: 4,
  Inverse: 8,
  Hidden: 16,
  Strike: 32,
  Blink: 64,
} as const

/** Cell width flags in slot 4, bits 16..19. */
export const Width = {
  Wide: 1,
  WideSpacer: 2,
  LeadingWideSpacer: 4,
} as const

export type UnderlineStyle = 'none' | 'single' | 'double' | 'curly' | 'dotted' | 'dashed'
const UNDERLINES: readonly UnderlineStyle[] = ['none', 'single', 'double', 'curly', 'dotted', 'dashed']

export function attributes(style: number): number {
  return style & 0xffff
}

export function widthFlags(style: number): number {
  return (style >>> 16) & 0x7
}

export function underlineStyle(style: number): UnderlineStyle {
  return UNDERLINES[(style >>> 20) & 0x7] ?? 'none'
}

/** A raw engine color: default, an indexed palette slot, or 24-bit RGB. */
export type RawColor =
  | { kind: 'default' }
  | { kind: 'indexed'; index: number }
  | { kind: 'rgb'; rgb: number }

export const ColorTag = { Default: 0, Indexed: 1, Rgb: 2 } as const

export function colorTag(raw: number): number {
  return raw >>> 24
}

export function decodeColor(raw: number): RawColor {
  switch (raw >>> 24) {
    case ColorTag.Indexed:
      return { kind: 'indexed', index: raw & 0xff }
    case ColorTag.Rgb:
      return { kind: 'rgb', rgb: raw & 0xffffff }
    default:
      return { kind: 'default' }
  }
}

/** A block of viewport rows with accessors over the flat cell data. */
export class CellRows {
  readonly data: Uint32Array
  readonly cols: number
  readonly startRow: number
  readonly strings: readonly string[]

  constructor(data: Uint32Array, cols: number, startRow: number, strings: readonly string[]) {
    this.data = data
    this.cols = cols
    this.startRow = startRow
    this.strings = strings
  }

  get rowCount(): number {
    return this.cols === 0 ? 0 : this.data.length / (this.cols * CELL_STRIDE)
  }

  /** Slot offset of a cell; `row` is relative to `startRow`. */
  offset(row: number, col: number): number {
    return (row * this.cols + col) * CELL_STRIDE
  }

  text(row: number, col: number): string {
    const value = this.data[this.offset(row, col)] ?? 32
    if (value & STRING_FLAG) return this.strings[value & ~STRING_FLAG & 0x7fffffff] ?? ''
    return value === 0 ? ' ' : String.fromCodePoint(value)
  }

  link(row: number, col: number): string | undefined {
    const value = this.data[this.offset(row, col) + Slot.Link] ?? 0
    return value === 0 ? undefined : this.strings[value - 1]
  }
}
