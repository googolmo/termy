import type { WasmSource } from '@termysh/core'
import type { ThemeInput } from './theme.ts'

export type FontWeight = 'normal' | 'bold' | `${number}` | number
export type CursorStyle = 'block' | 'bar' | 'underline'
export type CursorInactiveStyle = CursorStyle | 'outline' | 'none'
export type ScrollModifier = 'alt' | 'ctrl' | 'shift' | 'none'

export interface LinkHandler {
  /** Called when a link is clicked. Defaults to opening it in a new tab. */
  activate(event: MouseEvent, uri: string): void
  hover?(event: MouseEvent, uri: string): void
  leave?(event: MouseEvent, uri: string): void
  /** Also allow schemes other than http(s) and mailto (e.g. `file:`). */
  allowNonHttpProtocols?: boolean
}

export interface TerminalOptions {
  /** Initial size; `autoFit` replaces it once the terminal is attached. */
  cols?: number
  rows?: number
  /** Scrollback lines. Default 1000. */
  scrollback?: number

  fontFamily?: string
  /** CSS pixels. Default 14. */
  fontSize?: number
  fontWeight?: FontWeight
  fontWeightBold?: FontWeight
  /** Multiplier of the font's natural line height. Default 1. */
  lineHeight?: number
  /** Extra CSS pixels between cells. Default 0. */
  letterSpacing?: number

  /** Bundled theme id, a theme object, or `{ extends: id, ...overrides }`. */
  theme?: ThemeInput
  drawBoldTextInBrightColors?: boolean
  /** WCAG contrast ratio enforced for text, 1 (off) to 21. Default 1. */
  minimumContrastRatio?: number
  /** Let the page show through cells that use the default background. */
  allowTransparency?: boolean
  /** Inner padding in CSS pixels. Default 0. */
  padding?: number

  cursorStyle?: CursorStyle
  cursorBlink?: boolean
  /** Bar cursor width in CSS pixels. Default 1. */
  cursorWidth?: number
  cursorInactiveStyle?: CursorInactiveStyle

  /** Draw box drawing, blocks, sextants and Braille with Termy's geometry. Default true. */
  customGlyphs?: boolean
  /** Render kitty graphics protocol images. Default true. */
  images?: boolean

  scrollSensitivity?: number
  fastScrollSensitivity?: number
  fastScrollModifier?: ScrollModifier
  /** Wheel sends arrow keys in the alternate screen when mouse tracking is off. Default true. */
  alternateScroll?: boolean
  /** Jump to the bottom when the user types. Default true. */
  scrollOnUserInput?: boolean
  /** Show the overlay scrollbar. Default true. */
  scrollbar?: boolean

  /** macOS: Option acts as Meta/Alt instead of composing characters. */
  macOptionIsMeta?: boolean
  /** macOS: Option+click selects even when the application tracks the mouse. */
  macOptionClickForcesSelection?: boolean
  rightClickSelectsWord?: boolean
  /** Alt+click on the cursor row moves the cursor with arrow keys. */
  altClickMovesCursor?: boolean
  /** Characters that end a double-click word selection. */
  wordSeparator?: string
  copyOnSelect?: boolean

  /** Treat `\n` written to the terminal as `\r\n`. */
  convertEol?: boolean
  /** Ignore all user input. */
  disableStdin?: boolean
  /** Never wrap pastes in bracketed paste markers. */
  ignoreBracketedPasteMode?: boolean

  /** Handles OSC 8 hyperlinks and detected URLs. `null` disables links. */
  linkHandler?: LinkHandler | null
  /** Detect plain-text URLs in addition to OSC 8 links. Default true. */
  linkDetection?: boolean
  /** Let OSC 52 write the system clipboard. Default false. */
  allowClipboardWrite?: boolean
  /** Visual flash on BEL. Default false. */
  bellStyle?: 'none' | 'visual'
  /** Mirror output into an aria-live region. Default false. */
  screenReaderMode?: boolean

  /** Resize to the container automatically. Default true. */
  autoFit?: boolean
  /** Override the device pixel ratio (defaults to `window.devicePixelRatio`). */
  devicePixelRatio?: number
  /** Where to load `termy.wasm` from when it is not initialized yet. */
  wasm?: WasmSource
}

export type ResolvedOptions = Required<Omit<TerminalOptions, 'linkHandler' | 'devicePixelRatio' | 'wasm' | 'theme'>> & {
  linkHandler: LinkHandler | null
  devicePixelRatio: number | undefined
  wasm: WasmSource | undefined
  theme: ThemeInput | undefined
}

export const DEFAULT_FONT_FAMILY =
  '"JetBrains Mono", "SF Mono", Menlo, Monaco, Consolas, "Liberation Mono", "DejaVu Sans Mono", monospace'

export const defaultLinkHandler: LinkHandler = {
  activate(_event, uri) {
    window.open(uri, '_blank', 'noopener,noreferrer')
  },
}

export const DEFAULT_OPTIONS: ResolvedOptions = {
  cols: 80,
  rows: 24,
  scrollback: 1000,
  fontFamily: DEFAULT_FONT_FAMILY,
  fontSize: 14,
  fontWeight: 'normal',
  fontWeightBold: 'bold',
  lineHeight: 1,
  letterSpacing: 0,
  theme: undefined,
  drawBoldTextInBrightColors: true,
  minimumContrastRatio: 1,
  allowTransparency: false,
  padding: 0,
  cursorStyle: 'block',
  cursorBlink: false,
  cursorWidth: 1,
  cursorInactiveStyle: 'outline',
  customGlyphs: true,
  images: true,
  scrollSensitivity: 1,
  fastScrollSensitivity: 5,
  fastScrollModifier: 'alt',
  alternateScroll: true,
  scrollOnUserInput: true,
  scrollbar: true,
  macOptionIsMeta: false,
  macOptionClickForcesSelection: false,
  rightClickSelectsWord: false,
  altClickMovesCursor: true,
  wordSeparator: ' ()[]{}\',"`',
  copyOnSelect: false,
  convertEol: false,
  disableStdin: false,
  ignoreBracketedPasteMode: false,
  linkHandler: defaultLinkHandler,
  linkDetection: true,
  allowClipboardWrite: false,
  bellStyle: 'none',
  screenReaderMode: false,
  autoFit: true,
  devicePixelRatio: undefined,
  wasm: undefined,
}

export function resolveOptions(options: TerminalOptions): ResolvedOptions {
  const resolved = { ...DEFAULT_OPTIONS }
  for (const [key, value] of Object.entries(options)) {
    if (value !== undefined) (resolved as Record<string, unknown>)[key] = value
  }
  return resolved
}

/** Options whose change needs new font metrics (and a fit). */
export const METRIC_OPTIONS: ReadonlySet<keyof TerminalOptions> = new Set([
  'fontFamily',
  'fontSize',
  'fontWeight',
  'fontWeightBold',
  'lineHeight',
  'letterSpacing',
  'padding',
  'devicePixelRatio',
])
