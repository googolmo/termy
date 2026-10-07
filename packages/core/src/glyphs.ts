import { glyphPlan as wasmGlyphPlan } from './wasm/termy_wasm.js'

export type GlyphKind = 'block' | 'box' | 'sextant' | 'braille' | 'roundedCorner' | 'diagonal'

/** Cell-normalized rectangle (0..1 on both axes). */
export interface GlyphRect {
  left: number
  top: number
  right: number
  bottom: number
  alpha: number
  /** `outward` floors left/top and ceils right/bottom to avoid seams. */
  snap: 'nearest' | 'outward'
}

/**
 * A normalized stroke. Lines have two points; rounded corners have six:
 * start, curve start, control A, control B, curve end, end.
 */
export interface GlyphStroke {
  kind: 'line' | 'roundedCorner'
  /** Fraction of the cell width. */
  width: number
  points: Array<{ x: number; y: number }>
}

export interface GlyphPlan {
  kind: GlyphKind
  rects: GlyphRect[]
  strokes: GlyphStroke[]
}

const KINDS: readonly GlyphKind[] = ['block', 'box', 'sextant', 'braille', 'roundedCorner', 'diagonal']
const MAX_STROKE_POINTS = 6
const RECT_STRIDE = 6
const STROKE_STRIDE = 3 + MAX_STROKE_POINTS * 2

/**
 * Termy's pixel-exact geometry for block elements, box drawing, sextants,
 * Braille runs, rounded corners and diagonals. Returns undefined for
 * characters that should be shaped as normal text. `neighbors` are the code
 * points two before, before, after and two after in the row (0 when absent).
 */
export function glyphPlan(
  codePoint: number,
  neighbors: readonly [number, number, number, number],
  cellWidth: number,
  cellHeight: number,
  fontSize: number,
): GlyphPlan | undefined {
  const raw = wasmGlyphPlan(codePoint, Uint32Array.from(neighbors), cellWidth, cellHeight, fontSize)
  if (!raw) return undefined
  const rectCount = raw[1] ?? 0
  const strokeCount = raw[2] ?? 0
  const rects: GlyphRect[] = []
  let offset = 3
  for (let i = 0; i < rectCount; i++, offset += RECT_STRIDE) {
    rects.push({
      left: raw[offset]!,
      top: raw[offset + 1]!,
      right: raw[offset + 2]!,
      bottom: raw[offset + 3]!,
      alpha: raw[offset + 4]!,
      snap: raw[offset + 5] === 1 ? 'outward' : 'nearest',
    })
  }
  const strokes: GlyphStroke[] = []
  for (let i = 0; i < strokeCount; i++, offset += STROKE_STRIDE) {
    const pointCount = raw[offset + 2]!
    const points: Array<{ x: number; y: number }> = []
    for (let p = 0; p < pointCount; p++) {
      points.push({ x: raw[offset + 3 + p * 2]!, y: raw[offset + 4 + p * 2]! })
    }
    strokes.push({ kind: raw[offset] === 1 ? 'roundedCorner' : 'line', width: raw[offset + 1]!, points })
  }
  return { kind: KINDS[raw[0] ?? 0] ?? 'block', rects, strokes }
}
