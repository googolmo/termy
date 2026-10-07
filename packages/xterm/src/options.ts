import type { LinkHandler, TerminalOptions } from '@termysh/web'
import type { ILinkHandler, ITerminalOptions, ITheme } from './types.ts'

/** xterm.js's built-in default colors, so an unthemed terminal looks the same. */
export const XTERM_DEFAULT_THEME: ITheme = {
  foreground: '#ffffff',
  background: '#000000',
  cursor: '#ffffff',
  cursorAccent: '#000000',
  selectionBackground: 'rgba(255, 255, 255, 0.3)',
  black: '#2e3436',
  red: '#cc0000',
  green: '#4e9a06',
  yellow: '#c4a000',
  blue: '#3465a4',
  magenta: '#75507b',
  cyan: '#06989a',
  white: '#d3d7cf',
  brightBlack: '#555753',
  brightRed: '#ef2929',
  brightGreen: '#8ae234',
  brightYellow: '#fce94f',
  brightBlue: '#729fcf',
  brightMagenta: '#ad7fa8',
  brightCyan: '#34e2e2',
  brightWhite: '#eeeeec',
}

/** xterm.js options with no Termy equivalent; accepted and stored only. */
export const IGNORED_OPTIONS: ReadonlySet<string> = new Set([
  'allowProposedApi',
  'logLevel',
  'rescaleOverlappingGlyphs',
  'smoothScrollDuration',
  'tabStopWidth',
  'windowsMode',
  'overviewRulerWidth',
])

/** Translate xterm.js options into `@termysh/web` options. */
export function toTermyOptions(options: ITerminalOptions): TerminalOptions {
  const out: TerminalOptions = {}
  for (const [key, value] of Object.entries(options)) {
    if (value === undefined || IGNORED_OPTIONS.has(key)) continue
    switch (key) {
      case 'theme':
      case 'termyTheme':
        break
      case 'linkHandler':
        out.linkHandler = value === null ? null : adaptLinkHandler(value as ILinkHandler)
        break
      default:
        ;(out as Record<string, unknown>)[key] = value
    }
  }
  if ('theme' in options || 'termyTheme' in options) {
    const { scrollbarSliderBackground, ...theme } = options.theme ?? {}
    const base = options.termyTheme ? { extends: options.termyTheme } : XTERM_DEFAULT_THEME
    out.theme = { ...base, ...theme, scrollbarThumb: scrollbarSliderBackground }
  }
  return out
}

function adaptLinkHandler(handler: ILinkHandler): LinkHandler {
  const range = { start: { x: 0, y: 0 }, end: { x: 0, y: 0 } }
  return {
    activate: (event, uri) => handler.activate(event, uri, range),
    hover: handler.hover ? (event, uri) => handler.hover?.(event, uri, range) : undefined,
    leave: handler.leave ? (event, uri) => handler.leave?.(event, uri, range) : undefined,
    allowNonHttpProtocols: handler.allowNonHttpProtocols,
  }
}
