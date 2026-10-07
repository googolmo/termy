import type { FontWeight } from '../options.ts'

export interface FontOptions {
  fontFamily: string
  fontSize: number
  fontWeight: FontWeight
  fontWeightBold: FontWeight
  lineHeight: number
  letterSpacing: number
}

/** Cell geometry. Device sizes are integers so rows and columns never seam. */
export interface CellMetrics {
  dpr: number
  deviceWidth: number
  deviceHeight: number
  /** CSS pixels: `device / dpr`. */
  width: number
  height: number
  /** Device pixels from the cell top to the text baseline. */
  baseline: number
  /** Device pixels. */
  fontSize: number
  underlinePosition: number
  lineThickness: number
  fonts: { regular: string; bold: string; italic: string; boldItalic: string }
}

let measureContext: CanvasRenderingContext2D | null | undefined

export function cssFont(weight: FontWeight, italic: boolean, sizePx: number, family: string): string {
  return `${italic ? 'italic ' : ''}${weight} ${sizePx}px ${family}`
}

export function measureCell(options: FontOptions, dpr: number): CellMetrics {
  const size = options.fontSize * dpr
  const fonts = {
    regular: cssFont(options.fontWeight, false, size, options.fontFamily),
    bold: cssFont(options.fontWeightBold, false, size, options.fontFamily),
    italic: cssFont(options.fontWeight, true, size, options.fontFamily),
    boldItalic: cssFont(options.fontWeightBold, true, size, options.fontFamily),
  }
  measureContext ??= document.createElement('canvas').getContext('2d')
  let advance = size * 0.6
  let ascent = size * 0.8
  let descent = size * 0.2
  if (measureContext) {
    measureContext.font = fonts.regular
    const sample = measureContext.measureText('W'.repeat(32))
    if (sample.width > 0) advance = sample.width / 32
    const box = measureContext.measureText('Mg│█')
    ascent = box.fontBoundingBoxAscent || box.actualBoundingBoxAscent || ascent
    descent = box.fontBoundingBoxDescent || box.actualBoundingBoxDescent || descent
  }
  const deviceWidth = Math.max(1, Math.round(advance + options.letterSpacing * dpr))
  const natural = Math.ceil(ascent + descent)
  const deviceHeight = Math.max(1, Math.round(natural * options.lineHeight))
  const baseline = Math.round((deviceHeight - natural) / 2 + ascent)
  const lineThickness = Math.max(1, Math.round(dpr))
  return {
    dpr,
    deviceWidth,
    deviceHeight,
    width: deviceWidth / dpr,
    height: deviceHeight / dpr,
    baseline,
    fontSize: size,
    underlinePosition: Math.min(deviceHeight - lineThickness, baseline + Math.max(lineThickness, Math.round(descent / 2))),
    lineThickness,
    fonts,
  }
}
