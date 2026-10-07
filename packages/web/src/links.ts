import { CELL_STRIDE, Slot, type TermyCore, Width } from '@termysh/core'

export interface LinkMatch {
  uri: string
  row: number
  /** Viewport columns, end exclusive. */
  start: number
  end: number
  /** OSC 8 hyperlink rather than a detected URL. */
  explicit: boolean
}

const URL_PATTERN = /(?:https?:\/\/|mailto:|file:\/\/)[^\s<>"'`]*[^\s<>"'`.,;:!?)\]}]/g
const SAFE_SCHEMES = /^(https?:|mailto:)/i

export function isSafeLink(uri: string, allowNonHttp: boolean): boolean {
  return allowNonHttp || SAFE_SCHEMES.test(uri)
}

/** The link under viewport cell `(row, col)`: OSC 8 first, then detected URLs. */
export function linkAt(core: TermyCore, row: number, col: number, detect: boolean): LinkMatch | undefined {
  const cells = core.readRows(row, row + 1)
  const uri = cells.link(0, col)
  if (uri) {
    let start = col
    let end = col + 1
    while (start > 0 && cells.link(0, start - 1) === uri) start--
    while (end < cells.cols && cells.link(0, end) === uri) end++
    return { uri, row, start, end, explicit: true }
  }
  if (!detect) return undefined

  // Map string offsets back to columns, skipping wide-character spacers.
  let text = ''
  const columns: number[] = []
  for (let c = 0; c < cells.cols; c++) {
    const width = (cells.data[c * CELL_STRIDE + Slot.Style]! >>> 16) & 0x7
    if (width & (Width.WideSpacer | Width.LeadingWideSpacer)) continue
    const ch = cells.text(0, c)
    for (let i = 0; i < ch.length; i++) columns.push(c)
    text += ch
  }
  columns.push(cells.cols)
  for (const match of text.matchAll(URL_PATTERN)) {
    const start = columns[match.index] ?? 0
    const end = columns[match.index + match[0].length] ?? cells.cols
    if (col >= start && col < end) return { uri: match[0], row, start, end, explicit: false }
  }
  return undefined
}
