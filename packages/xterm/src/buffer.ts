import { Attr, CELL_STRIDE, type CellRows, ColorTag, Slot, Width, underlineStyle } from '@termysh/core'
import type { Terminal as TermyTerminal } from '@termysh/web'
import type { IBuffer, IBufferCell, IBufferLine } from './types.ts'

// xterm.js color modes (Attributes.CM_*).
const CM_DEFAULT = 0
const CM_P16 = 0x1000000
const CM_P256 = 0x2000000
const CM_RGB = 0x3000000

class BufferCell implements IBufferCell {
  constructor(
    private readonly rows: CellRows | undefined,
    private readonly col: number,
  ) {}

  #slot(slot: number): number {
    return this.rows?.data[this.col * CELL_STRIDE + slot] ?? 0
  }

  #attr(flag: number): number {
    return this.#slot(Slot.Style) & flag ? 1 : 0
  }

  getWidth(): number {
    const width = (this.#slot(Slot.Style) >>> 16) & 0x7
    if (width & (Width.WideSpacer | Width.LeadingWideSpacer)) return 0
    return width & Width.Wide ? 2 : 1
  }

  getChars(): string {
    if (!this.rows || this.getWidth() === 0) return ''
    const text = this.rows.text(0, this.col)
    return text === ' ' && this.#slot(Slot.Text) === 0 ? '' : text
  }

  getCode(): number {
    return this.getChars().codePointAt(0) ?? 0
  }

  getFgColorMode(): number {
    return colorMode(this.#slot(Slot.Foreground))
  }

  getBgColorMode(): number {
    return colorMode(this.#slot(Slot.Background))
  }

  getFgColor(): number {
    return colorValue(this.#slot(Slot.Foreground))
  }

  getBgColor(): number {
    return colorValue(this.#slot(Slot.Background))
  }

  isBold(): number {
    return this.#attr(Attr.Bold)
  }

  isItalic(): number {
    return this.#attr(Attr.Italic)
  }

  isDim(): number {
    return this.#attr(Attr.Dim)
  }

  isUnderline(): number {
    return underlineStyle(this.#slot(Slot.Style)) === 'none' ? 0 : 1
  }

  isBlink(): number {
    return this.#attr(Attr.Blink)
  }

  isInverse(): number {
    return this.#attr(Attr.Inverse)
  }

  isInvisible(): number {
    return this.#attr(Attr.Hidden)
  }

  isStrikethrough(): number {
    return this.#attr(Attr.Strike)
  }

  isOverline(): number {
    return 0
  }

  isFgRGB(): boolean {
    return this.getFgColorMode() === CM_RGB
  }

  isBgRGB(): boolean {
    return this.getBgColorMode() === CM_RGB
  }

  isFgPalette(): boolean {
    const mode = this.getFgColorMode()
    return mode === CM_P16 || mode === CM_P256
  }

  isBgPalette(): boolean {
    const mode = this.getBgColorMode()
    return mode === CM_P16 || mode === CM_P256
  }

  isFgDefault(): boolean {
    return this.getFgColorMode() === CM_DEFAULT
  }

  isBgDefault(): boolean {
    return this.getBgColorMode() === CM_DEFAULT
  }

  isAttributeDefault(): boolean {
    return (this.#slot(Slot.Style) & 0xffff) === 0 && this.isFgDefault() && this.isBgDefault() && this.isUnderline() === 0
  }
}

function colorMode(raw: number): number {
  switch (raw >>> 24) {
    case ColorTag.Indexed:
      return (raw & 0xff) < 16 ? CM_P16 : CM_P256
    case ColorTag.Rgb:
      return CM_RGB
    default:
      return CM_DEFAULT
  }
}

function colorValue(raw: number): number {
  switch (raw >>> 24) {
    case ColorTag.Indexed:
      return raw & 0xff
    case ColorTag.Rgb:
      return raw & 0xffffff
    default:
      return -1
  }
}

class BufferLine implements IBufferLine {
  #rows: CellRows | undefined

  constructor(
    private readonly term: TermyTerminal,
    private readonly line: number,
  ) {}

  #cells(): CellRows | undefined {
    const core = this.term.core
    this.#rows ??= core?.readLine(this.line - core.historySize)
    return this.#rows
  }

  get isWrapped(): boolean {
    // xterm.js marks the continuation line; Termy marks the line that wraps.
    return this.line > 0 && this.term.isLineWrapped(this.line - 1)
  }

  get length(): number {
    return this.term.cols
  }

  getCell(x: number): IBufferCell | undefined {
    if (x < 0 || x >= this.term.cols) return undefined
    return new BufferCell(this.#cells(), x)
  }

  translateToString(trimRight = false, startColumn = 0, endColumn: number = this.term.cols): string {
    const core = this.term.core
    if (!core) return ''
    return core.lineTextRange(this.line - core.historySize, startColumn, endColumn, trimRight) ?? ''
  }
}

export class Buffer implements IBuffer {
  constructor(
    private readonly term: TermyTerminal,
    readonly type: 'normal' | 'alternate',
  ) {}

  get cursorX(): number {
    return this.term.cursor?.col ?? 0
  }

  get cursorY(): number {
    return this.term.cursor?.row ?? 0
  }

  get viewportY(): number {
    return this.term.viewportY
  }

  get baseY(): number {
    return this.term.historySize
  }

  get length(): number {
    return this.term.historySize + this.term.rows
  }

  getLine(y: number): IBufferLine | undefined {
    if (y < 0 || y >= this.length) return undefined
    return new BufferLine(this.term, y)
  }

  getNullCell(): IBufferCell {
    return new BufferCell(undefined, 0)
  }
}
