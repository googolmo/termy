export { Terminal } from './terminal.ts'
export {
  CanvasAddon,
  FitAddon,
  NoopAddon,
  SearchAddon,
  Unicode11Addon,
  WebLinksAddon,
  WebglAddon,
  type ISearchOptions,
} from './addons.ts'
export { XTERM_DEFAULT_THEME, toTermyOptions } from './options.ts'
export type {
  FontWeight,
  IBuffer,
  IBufferCell,
  IBufferCellPosition,
  IBufferLine,
  IBufferNamespace,
  IBufferRange,
  IDisposable,
  IEvent,
  ILinkHandler,
  IMarker,
  IModes,
  ITerminalAddon,
  ITerminalInitOnlyOptions,
  ITerminalOptions,
  ITheme,
  IUnicodeHandling,
} from './types.ts'
export { init, initSync, builtinThemeIds } from '@termysh/web'
