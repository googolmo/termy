// Public types mirroring `@xterm/xterm`, so existing code type-checks unchanged.

import type { IDisposable, IEvent } from '@termysh/web'

export type { IDisposable, IEvent }

export type FontWeight =
  | 'normal'
  | 'bold'
  | '100'
  | '200'
  | '300'
  | '400'
  | '500'
  | '600'
  | '700'
  | '800'
  | '900'
  | number

export interface ITheme {
  foreground?: string
  background?: string
  cursor?: string
  cursorAccent?: string
  selectionBackground?: string
  selectionForeground?: string
  selectionInactiveBackground?: string
  scrollbarSliderBackground?: string
  scrollbarSliderHoverBackground?: string
  scrollbarSliderActiveBackground?: string
  overviewRulerBorder?: string
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
  extendedAnsi?: string[]
}

export interface IBufferCellPosition {
  x: number
  y: number
}

export interface IBufferRange {
  start: IBufferCellPosition
  end: IBufferCellPosition
}

export interface ILinkHandler {
  activate(event: MouseEvent, text: string, range: IBufferRange): void
  hover?(event: MouseEvent, text: string, range: IBufferRange): void
  leave?(event: MouseEvent, text: string, range: IBufferRange): void
  allowNonHttpProtocols?: boolean
}

export interface ITerminalOptions {
  allowProposedApi?: boolean
  allowTransparency?: boolean
  altClickMovesCursor?: boolean
  convertEol?: boolean
  cursorBlink?: boolean
  cursorStyle?: 'block' | 'underline' | 'bar'
  cursorWidth?: number
  cursorInactiveStyle?: 'outline' | 'block' | 'bar' | 'underline' | 'none'
  customGlyphs?: boolean
  disableStdin?: boolean
  drawBoldTextInBrightColors?: boolean
  fastScrollModifier?: 'none' | 'alt' | 'ctrl' | 'shift'
  fastScrollSensitivity?: number
  fontSize?: number
  fontFamily?: string
  fontWeight?: FontWeight
  fontWeightBold?: FontWeight
  ignoreBracketedPasteMode?: boolean
  letterSpacing?: number
  lineHeight?: number
  linkHandler?: ILinkHandler | null
  logLevel?: 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'off'
  macOptionIsMeta?: boolean
  macOptionClickForcesSelection?: boolean
  minimumContrastRatio?: number
  rescaleOverlappingGlyphs?: boolean
  rightClickSelectsWord?: boolean
  screenReaderMode?: boolean
  scrollback?: number
  scrollOnUserInput?: boolean
  scrollSensitivity?: number
  smoothScrollDuration?: number
  tabStopWidth?: number
  theme?: ITheme
  windowsMode?: boolean
  wordSeparator?: string
  overviewRulerWidth?: number

  // Termy extensions.
  /** Load a bundled Termy theme by id; `theme` overrides individual colors. */
  termyTheme?: string
  /** Render kitty graphics protocol images. Default true. */
  images?: boolean
  /** Let OSC 52 write the clipboard. Default false. */
  allowClipboardWrite?: boolean
  /** Fit to the container automatically (xterm.js needs FitAddon). Default false. */
  autoFit?: boolean
  padding?: number
  copyOnSelect?: boolean
}

export interface ITerminalInitOnlyOptions {
  cols?: number
  rows?: number
}

export interface ITerminalAddon extends IDisposable {
  activate(terminal: import('./terminal.ts').Terminal): void
}

export interface IMarker extends IDisposable {
  readonly id: number
  readonly isDisposed: boolean
  readonly line: number
  onDispose: IEvent<void>
}

export interface IBufferCell {
  getWidth(): number
  getChars(): string
  getCode(): number
  getFgColorMode(): number
  getBgColorMode(): number
  getFgColor(): number
  getBgColor(): number
  isBold(): number
  isItalic(): number
  isDim(): number
  isUnderline(): number
  isBlink(): number
  isInverse(): number
  isInvisible(): number
  isStrikethrough(): number
  isOverline(): number
  isFgRGB(): boolean
  isBgRGB(): boolean
  isFgPalette(): boolean
  isBgPalette(): boolean
  isFgDefault(): boolean
  isBgDefault(): boolean
  isAttributeDefault(): boolean
}

export interface IBufferLine {
  readonly isWrapped: boolean
  readonly length: number
  getCell(x: number, cell?: IBufferCell): IBufferCell | undefined
  translateToString(trimRight?: boolean, startColumn?: number, endColumn?: number): string
}

export interface IBuffer {
  readonly type: 'normal' | 'alternate'
  readonly cursorY: number
  readonly cursorX: number
  readonly viewportY: number
  readonly baseY: number
  readonly length: number
  getLine(y: number): IBufferLine | undefined
  getNullCell(): IBufferCell
}

export interface IBufferNamespace {
  readonly active: IBuffer
  readonly normal: IBuffer
  readonly alternate: IBuffer
  onBufferChange: IEvent<IBuffer>
}

export interface IModes {
  readonly applicationCursorKeysMode: boolean
  readonly applicationKeypadMode: boolean
  readonly bracketedPasteMode: boolean
  readonly insertMode: boolean
  readonly mouseTrackingMode: 'none' | 'x10' | 'vt200' | 'drag' | 'any'
  readonly originMode: boolean
  readonly reverseWraparoundMode: boolean
  readonly sendFocusMode: boolean
  readonly synchronizedOutputMode: boolean
  readonly wraparoundMode: boolean
}

export interface IUnicodeHandling {
  readonly versions: ReadonlyArray<string>
  activeVersion: string
}
