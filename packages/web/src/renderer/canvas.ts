import {
  Attr,
  CELL_STRIDE,
  type CellRows,
  ColorTag,
  type CursorState,
  STRING_FLAG,
  Slot,
  type TermyCore,
  Width,
  underlineStyle,
} from '@termysh/core'
import type { CursorInactiveStyle, CursorStyle } from '../options.ts'
import { type ResolvedTheme, type Rgba, blend, ensureContrast, fromRgb24 } from '../theme.ts'
import { GlyphCache, drawGlyphPlan, isCustomGlyphCandidate } from './glyphs.ts'
import type { CellMetrics } from './metrics.ts'

export interface RenderOptions {
  cursorStyle: CursorStyle
  cursorInactiveStyle: CursorInactiveStyle
  cursorWidth: number
  drawBoldTextInBrightColors: boolean
  minimumContrastRatio: number
  customGlyphs: boolean
  transparentBackground: boolean
}

export interface CursorPaint {
  cursor: CursorState
  focused: boolean
  /** False during the "off" phase of a blinking cursor. */
  blinkOn: boolean
}

/** Selection test in viewport coordinates. */
export type SelectionTest = (row: number, col: number) => boolean

/** OSC 10/11/12 overrides: foreground, background, cursor. */
type DynamicColors = [Rgba | undefined, Rgba | undefined, Rgba | undefined]

export interface LinkHighlight {
  row: number
  start: number
  end: number
}

/**
 * Damage-driven Canvas 2D renderer. Only rows reported dirty by the engine (or
 * invalidated by cursor, selection and theme changes) are repainted.
 */
export class CanvasRenderer {
  readonly canvas: HTMLCanvasElement
  #ctx: CanvasRenderingContext2D
  #core: TermyCore
  #metrics: CellMetrics | undefined
  #theme: ResolvedTheme | undefined
  #options: RenderOptions | undefined
  #glyphs = new GlyphCache()
  #dirty = new Set<number>()
  #full = true
  #paletteRevision = -1
  #palette: Array<Rgba | undefined> = new Array(256)
  #dynamic: DynamicColors = [undefined, undefined, undefined]
  #rgbCache = new Map<number, Rgba>()
  #lastCursorRow = -1

  constructor(core: TermyCore) {
    this.#core = core
    this.canvas = document.createElement('canvas')
    this.canvas.style.position = 'absolute'
    this.canvas.style.left = '0'
    this.canvas.style.top = '0'
    const ctx = this.canvas.getContext('2d', { alpha: true })
    if (!ctx) throw new Error('@termysh/web: Canvas 2D is not available')
    this.#ctx = ctx
  }

  get metrics(): CellMetrics | undefined {
    return this.#metrics
  }

  setMetrics(metrics: CellMetrics): void {
    this.#metrics = metrics
    this.#glyphs.setMetrics(metrics)
    this.resize()
  }

  setTheme(theme: ResolvedTheme): void {
    this.#theme = theme
    this.invalidate()
  }

  setOptions(options: RenderOptions): void {
    this.#options = options
    this.invalidate()
  }

  /** Match the canvas to the engine's grid. */
  resize(): void {
    const metrics = this.#metrics
    if (!metrics) return
    const width = this.#core.cols * metrics.deviceWidth
    const height = this.#core.rows * metrics.deviceHeight
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width
      this.canvas.height = height
    }
    this.canvas.style.width = `${width / metrics.dpr}px`
    this.canvas.style.height = `${height / metrics.dpr}px`
    this.invalidate()
  }

  invalidate(): void {
    this.#full = true
  }

  invalidateRow(row: number): void {
    this.#dirty.add(row)
  }

  /** Paint pending damage. Returns the viewport rows that were repainted. */
  render(cursor: CursorPaint, selected: SelectionTest | undefined, link: LinkHighlight | undefined): number {
    const metrics = this.#metrics
    const theme = this.#theme
    const options = this.#options
    if (!metrics || !theme || !options) return 0
    const core = this.#core
    const rows = core.rows

    const damage = core.takeDamage()
    if (damage.full) this.#full = true
    for (const scroll of damage.scrolls) {
      for (let row = scroll.top; row < scroll.bottom; row++) this.#dirty.add(row)
    }
    for (const span of damage.spans) this.#dirty.add(span.row)

    if (core.paletteRevision !== this.#paletteRevision) {
      this.#paletteRevision = core.paletteRevision
      const overrides = core.paletteOverrides()
      for (let i = 0; i < 256; i++) this.#palette[i] = overrides[i] ? this.#raw(overrides[i]!) : undefined
      const dynamic = core.dynamicColorOverrides()
      this.#dynamic = [0, 1, 2].map((i) => (dynamic[i] ? this.#raw(dynamic[i]!) : undefined)) as DynamicColors
      this.#full = true
    }

    const cursorRow = cursor.cursor.row + core.displayOffset
    if (cursorRow !== this.#lastCursorRow) {
      this.#dirty.add(this.#lastCursorRow)
      this.#lastCursorRow = cursorRow
    }
    this.#dirty.add(cursorRow)

    let first: number
    let last: number
    if (this.#full) {
      first = 0
      last = rows - 1
    } else {
      first = rows
      last = -1
      for (const row of this.#dirty) {
        if (row < 0 || row >= rows) continue
        first = Math.min(first, row)
        last = Math.max(last, row)
      }
    }
    if (last < first) {
      this.#dirty.clear()
      return 0
    }

    const cells = core.readRows(first, last + 1)
    let painted = 0
    for (let row = first; row <= last; row++) {
      if (!this.#full && !this.#dirty.has(row)) continue
      this.#paintRow(row, cells, row - first, cursor, cursorRow, selected, link)
      painted++
    }
    this.#full = false
    this.#dirty.clear()
    return painted
  }

  #paintRow(
    row: number,
    cells: CellRows,
    rel: number,
    paint: CursorPaint,
    cursorRow: number,
    selected: SelectionTest | undefined,
    link: LinkHighlight | undefined,
  ): void {
    const ctx = this.#ctx
    const metrics = this.#metrics!
    const theme = this.#theme!
    const options = this.#options!
    const { deviceWidth: cw, deviceHeight: ch } = metrics
    const cols = cells.cols
    const data = cells.data
    const y = row * ch
    const defaultBg = this.#dynamic[1] ?? theme.background

    ctx.save()
    ctx.beginPath()
    ctx.rect(0, y, cols * cw, ch)
    ctx.clip()
    if (options.transparentBackground) ctx.clearRect(0, y, cols * cw, ch)
    else {
      ctx.fillStyle = defaultBg.css
      ctx.fillRect(0, y, cols * cw, ch)
    }

    // Backgrounds, coalesced into runs.
    let runStart = 0
    let runColor: string | undefined
    const flush = (end: number): void => {
      if (runColor) {
        ctx.fillStyle = runColor
        ctx.fillRect(runStart * cw, y, (end - runStart) * cw, ch)
      }
    }
    const fgs: Rgba[] = new Array(cols)
    for (let col = 0; col < cols; col++) {
      const at = (rel * cols + col) * CELL_STRIDE
      const style = data[at + Slot.Style]!
      const attrs = style & 0xffff
      const isSelected = selected?.(row, col) ?? false
      let fg = this.#foreground(data[at + Slot.Foreground]!, attrs)
      let bg = this.#background(data[at + Slot.Background]!)
      if (attrs & Attr.Inverse) [fg, bg] = [bg ?? defaultBg, fg]
      if (isSelected) {
        const selection = paint.focused ? theme.selectionBackground : theme.selectionInactiveBackground
        bg = blend(bg ?? defaultBg, selection, (selection.value & 255) / 255)
        if (theme.selectionForeground) fg = theme.selectionForeground
      }
      const effectiveBg = bg ?? defaultBg
      if (attrs & Attr.Dim) fg = blend(effectiveBg, fg, 0.5)
      if (attrs & Attr.Hidden) fg = effectiveBg
      if (options.minimumContrastRatio > 1) fg = ensureContrast(fg, effectiveBg, options.minimumContrastRatio)
      fgs[col] = fg
      const css = bg?.css
      if (css !== runColor) {
        flush(col)
        runStart = col
        runColor = css
      }
    }
    flush(cols)

    // Text and decorations.
    ctx.textBaseline = 'alphabetic'
    let currentFont = ''
    for (let col = 0; col < cols; col++) {
      const at = (rel * cols + col) * CELL_STRIDE
      const style = data[at + Slot.Style]!
      const width = (style >>> 16) & 0x7
      if (width & (Width.WideSpacer | Width.LeadingWideSpacer)) continue
      const value = data[at + Slot.Text]!
      const attrs = style & 0xffff
      const fg = fgs[col]!
      const x = col * cw
      const span = width & Width.Wide ? 2 : 1
      ctx.fillStyle = fg.css

      if (value !== 32 && value !== 0 && !(attrs & Attr.Hidden)) {
        const plan =
          options.customGlyphs && !(value & STRING_FLAG) && isCustomGlyphCandidate(value)
            ? this.#glyphs.plan(value, neighbors(data, rel, cols, col))
            : undefined
        if (plan) {
          drawGlyphPlan(ctx, plan, x, y, cw * span, ch)
        } else {
          const font = fontFor(metrics, attrs)
          if (font !== currentFont) {
            ctx.font = font
            currentFont = font
          }
          ctx.fillText(cells.text(rel, col), x, y + metrics.baseline)
        }
      }

      const underline = underlineStyle(style)
      const linked = link && link.row === row && col >= link.start && col < link.end
      if (underline !== 'none' || linked) {
        const color = data[at + Slot.UnderlineColor]!
        ctx.fillStyle = color ? (this.#foreground(color, 0).css ?? fg.css) : fg.css
        drawUnderline(ctx, linked && underline === 'none' ? 'dashed' : underline, x, y, cw * span, metrics)
        ctx.fillStyle = fg.css
      }
      if (attrs & Attr.Strike) {
        ctx.fillRect(x, y + Math.round(metrics.baseline - metrics.fontSize * 0.3), cw * span, metrics.lineThickness)
      }
    }

    if (row === cursorRow) this.#paintCursor(paint, cells, rel, row, currentFont)
    ctx.restore()
  }

  #paintCursor(paint: CursorPaint, cells: CellRows, rel: number, row: number, currentFont: string): void {
    const { cursor, focused, blinkOn } = paint
    if (!cursor.visible || (cursor.blinking && !blinkOn && focused)) return
    const options = this.#options!
    const theme = this.#theme!
    const metrics = this.#metrics!
    const ctx = this.#ctx
    const { deviceWidth: cw, deviceHeight: ch, lineThickness } = metrics
    const col = Math.min(cursor.col, cells.cols - 1)
    const at = (rel * cells.cols + col) * CELL_STRIDE
    const wide = ((cells.data[at + Slot.Style]! >>> 16) & Width.Wide) !== 0
    const x = col * cw
    const y = row * ch
    const width = cw * (wide ? 2 : 1)
    const color = this.#dynamic[2] ?? theme.cursor
    const style: CursorInactiveStyle = focused ? (cursor.shape ?? options.cursorStyle) : options.cursorInactiveStyle
    ctx.fillStyle = color.css
    switch (style) {
      case 'block': {
        ctx.fillRect(x, y, width, ch)
        const value = cells.data[at + Slot.Text]!
        if (value !== 32 && value !== 0) {
          ctx.fillStyle = theme.cursorAccent.css
          ctx.font = fontFor(metrics, cells.data[at + Slot.Style]! & 0xffff) || currentFont
          ctx.fillText(cells.text(rel, col), x, y + metrics.baseline)
        }
        break
      }
      case 'bar':
        ctx.fillRect(x, y, Math.max(1, Math.round(options.cursorWidth * metrics.dpr)), ch)
        break
      case 'underline':
        ctx.fillRect(x, y + ch - lineThickness * 2, width, lineThickness * 2)
        break
      case 'outline':
        ctx.strokeStyle = color.css
        ctx.lineWidth = lineThickness
        ctx.strokeRect(x + lineThickness / 2, y + lineThickness / 2, width - lineThickness, ch - lineThickness)
        break
      case 'none':
        break
    }
  }

  #raw(raw: number): Rgba {
    return raw >>> 24 === ColorTag.Rgb ? this.#rgb(raw & 0xffffff) : this.#indexed(raw & 0xff)
  }

  #rgb(rgb: number): Rgba {
    let color = this.#rgbCache.get(rgb)
    if (!color) {
      if (this.#rgbCache.size > 4096) this.#rgbCache.clear()
      color = fromRgb24(rgb)
      this.#rgbCache.set(rgb, color)
    }
    return color
  }

  #indexed(index: number): Rgba {
    return this.#palette[index] ?? this.#theme!.palette[index]!
  }

  #foreground(raw: number, attrs: number): Rgba {
    switch (raw >>> 24) {
      case ColorTag.Indexed: {
        let index = raw & 0xff
        if (attrs & Attr.Bold && index < 8 && this.#options!.drawBoldTextInBrightColors) index += 8
        return this.#indexed(index)
      }
      case ColorTag.Rgb:
        return this.#rgb(raw & 0xffffff)
      default:
        return this.#dynamic[0] ?? this.#theme!.foreground
    }
  }

  #background(raw: number): Rgba | undefined {
    switch (raw >>> 24) {
      case ColorTag.Indexed:
        return this.#indexed(raw & 0xff)
      case ColorTag.Rgb:
        return this.#rgb(raw & 0xffffff)
      default:
        return undefined
    }
  }
}

function fontFor(metrics: CellMetrics, attrs: number): string {
  const bold = (attrs & Attr.Bold) !== 0
  const italic = (attrs & Attr.Italic) !== 0
  if (bold && italic) return metrics.fonts.boldItalic
  if (bold) return metrics.fonts.bold
  if (italic) return metrics.fonts.italic
  return metrics.fonts.regular
}

function neighbors(data: Uint32Array, rel: number, cols: number, col: number): [number, number, number, number] {
  const at = (c: number): number => {
    if (c < 0 || c >= cols) return 0
    const value = data[(rel * cols + c) * CELL_STRIDE + Slot.Text]!
    return value & STRING_FLAG ? 0 : value
  }
  return [at(col - 2), at(col - 1), at(col + 1), at(col + 2)]
}

function drawUnderline(
  ctx: CanvasRenderingContext2D,
  style: string,
  x: number,
  y: number,
  width: number,
  metrics: CellMetrics,
): void {
  const t = metrics.lineThickness
  const top = y + metrics.underlinePosition
  switch (style) {
    case 'double':
      ctx.fillRect(x, top - t, width, t)
      ctx.fillRect(x, top + t, width, t)
      break
    case 'curly': {
      ctx.save()
      ctx.strokeStyle = ctx.fillStyle
      ctx.lineWidth = t
      ctx.beginPath()
      const amplitude = Math.max(1, t)
      for (let i = 0; i <= width; i++) {
        const py = top + Math.sin((i / width) * Math.PI * 2) * amplitude
        if (i === 0) ctx.moveTo(x + i, py)
        else ctx.lineTo(x + i, py)
      }
      ctx.stroke()
      ctx.restore()
      break
    }
    case 'dotted':
      for (let i = 0; i < width; i += t * 2) ctx.fillRect(x + i, top, t, t)
      break
    case 'dashed':
      for (let i = 0; i < width; i += t * 6) ctx.fillRect(x + i, top, Math.min(t * 3, width - i), t)
      break
    default:
      ctx.fillRect(x, top, width, t)
  }
}
