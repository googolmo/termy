import type { TermyCore } from '@termysh/core'

/**
 * A buffer position. `line` is absolute: 0 is the oldest scrollback line and
 * `historySize` is the first live-screen line, so it stays put while the
 * viewport scrolls.
 */
export interface BufferPoint {
  line: number
  col: number
}

export interface SelectionRange {
  start: BufferPoint
  /** Exclusive column on the end line. */
  end: BufferPoint
}

export type SelectionMode = 'char' | 'word' | 'line'

export function comparePoints(a: BufferPoint, b: BufferPoint): number {
  return a.line - b.line || a.col - b.col
}

export class Selection {
  #anchor: BufferPoint | undefined
  #head: BufferPoint | undefined
  #mode: SelectionMode = 'char'
  #wordSeparator: string

  constructor(wordSeparator: string) {
    this.#wordSeparator = wordSeparator
  }

  set wordSeparator(value: string) {
    this.#wordSeparator = value
  }

  get active(): boolean {
    const range = this.range()
    return !!range && comparePoints(range.start, range.end) < 0
  }

  start(core: TermyCore, point: BufferPoint, mode: SelectionMode): void {
    this.#mode = mode
    this.#anchor = point
    this.#head = point
    if (mode !== 'char') this.extend(core, point)
  }

  extend(_core: TermyCore, point: BufferPoint): void {
    if (!this.#anchor) return
    this.#head = point
  }

  set(range: SelectionRange): void {
    this.#mode = 'char'
    this.#anchor = range.start
    this.#head = range.end
  }

  clear(): void {
    this.#anchor = undefined
    this.#head = undefined
  }

  range(core?: TermyCore): SelectionRange | undefined {
    if (!this.#anchor || !this.#head) return undefined
    let start = this.#anchor
    let end = this.#head
    if (comparePoints(end, start) < 0) [start, end] = [end, start]
    if (this.#mode === 'char') return { start, end }
    if (this.#mode === 'line' || !core) {
      return { start: { line: start.line, col: 0 }, end: { line: end.line, col: Number.MAX_SAFE_INTEGER } }
    }
    return {
      start: { line: start.line, col: this.#wordBounds(core, start)[0] },
      end: { line: end.line, col: this.#wordBounds(core, end)[1] },
    }
  }

  /** Whether viewport cell `(row, col)` is selected. */
  contains(core: TermyCore, range: SelectionRange, row: number, col: number): boolean {
    const line = core.historySize + row - core.displayOffset
    if (line < range.start.line || line > range.end.line) return false
    if (line === range.start.line && col < range.start.col) return false
    if (line === range.end.line && col >= range.end.col) return false
    return true
  }

  text(core: TermyCore): string {
    const range = this.range(core)
    if (!range) return ''
    const parts: string[] = []
    for (let line = range.start.line; line <= range.end.line; line++) {
      const engineLine = line - core.historySize
      const start = line === range.start.line ? range.start.col : 0
      const end = line === range.end.line ? range.end.col : core.cols
      const wrapped = line < range.end.line && core.lineWrapped(engineLine)
      parts.push(core.lineTextRange(engineLine, start, Math.min(end, core.cols), !wrapped) ?? '')
      if (line < range.end.line && !wrapped) parts.push('\n')
    }
    return parts.join('')
  }

  #wordBounds(core: TermyCore, point: BufferPoint): [number, number] {
    const engineLine = point.line - core.historySize
    const cols = core.cols
    const charAt = (col: number): string => core.lineTextRange(engineLine, col, col + 1, false) ?? ' '
    const isWord = (col: number): boolean => {
      const ch = charAt(col)
      return ch !== '' && !this.#wordSeparator.includes(ch)
    }
    let start = Math.min(point.col, cols - 1)
    let end = start
    if (!isWord(start)) return [start, start + 1]
    while (start > 0 && isWord(start - 1)) start--
    while (end < cols - 1 && isWord(end + 1)) end++
    return [start, end + 1]
  }
}
