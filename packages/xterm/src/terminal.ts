import { Emitter, Terminal as TermyTerminal, type TerminalOptions } from '@termysh/web'
import { Buffer } from './buffer.ts'
import { toTermyOptions } from './options.ts'
import type {
  IBuffer,
  IBufferNamespace,
  IBufferRange,
  IDisposable,
  IEvent,
  IMarker,
  IModes,
  ITerminalAddon,
  ITerminalInitOnlyOptions,
  ITerminalOptions,
  IUnicodeHandling,
} from './types.ts'

let nextMarkerId = 1

/**
 * xterm.js-compatible terminal running on Termy's WebAssembly engine.
 *
 * Replace `import { Terminal } from '@xterm/xterm'` with
 * `import { Terminal } from '@termysh/xterm'`. The engine loads in the
 * background; calls made before it is ready are queued.
 */
export class Terminal implements IDisposable {
  /** The underlying `@termysh/web` terminal, for Termy-only features. */
  readonly termy: TermyTerminal
  readonly markers: IMarker[] = []
  readonly unicode: IUnicodeHandling = { versions: ['6', '11', '15'], activeVersion: '15' }

  #options: ITerminalOptions & ITerminalInitOnlyOptions
  #optionsProxy: ITerminalOptions
  #addons: ITerminalAddon[] = []
  #buffer: IBufferNamespace
  #onLineFeed = new Emitter<void>()
  #onBufferChange = new Emitter<IBuffer>()
  #customWheelHandler: ((event: WheelEvent) => boolean) | undefined
  #lastAlternate = false

  readonly onData: IEvent<string>
  readonly onBinary: IEvent<string>
  readonly onCursorMove: IEvent<void>
  readonly onLineFeed: IEvent<void> = this.#onLineFeed.event
  readonly onScroll: IEvent<number>
  readonly onSelectionChange: IEvent<void>
  readonly onRender: IEvent<{ start: number; end: number }>
  readonly onResize: IEvent<{ cols: number; rows: number }>
  readonly onTitleChange: IEvent<string>
  readonly onKey: IEvent<{ key: string; domEvent: KeyboardEvent }>
  readonly onBell: IEvent<void>
  readonly onWriteParsed: IEvent<void>

  constructor(options: ITerminalOptions & ITerminalInitOnlyOptions = {}) {
    this.#options = { ...options }
    const termyOptions: TerminalOptions = {
      // xterm.js keeps its size until FitAddon or resize() changes it.
      autoFit: false,
      ...toTermyOptions({ ...options, theme: options.theme ?? {} }),
    }
    this.termy = new TermyTerminal(termyOptions)
    this.onData = this.termy.onData
    this.onBinary = this.termy.onBinary
    this.onCursorMove = this.termy.onCursorMove
    this.onScroll = this.termy.onScroll
    this.onSelectionChange = this.termy.onSelectionChange
    this.onRender = this.termy.onRender
    this.onResize = this.termy.onResize
    this.onTitleChange = this.termy.onTitleChange
    this.onKey = this.termy.onKey
    this.onBell = this.termy.onBell
    this.onWriteParsed = this.termy.onWriteParsed

    const normal = new Buffer(this.termy, 'normal')
    const alternate = new Buffer(this.termy, 'alternate')
    const termy = this.termy
    this.#buffer = {
      get active() {
        return termy.modes?.alternateScreen ? alternate : normal
      },
      normal,
      alternate,
      onBufferChange: this.#onBufferChange.event,
    }
    this.termy.onWriteParsed(() => {
      const alternateNow = this.termy.modes?.alternateScreen ?? false
      if (alternateNow !== this.#lastAlternate) {
        this.#lastAlternate = alternateNow
        this.#onBufferChange.fire(this.#buffer.active)
      }
    })

    this.#optionsProxy = new Proxy(this.#options, {
      set: (target, key, value) => {
        ;(target as Record<string | symbol, unknown>)[key] = value
        this.#applyOptions({ [key]: value } as ITerminalOptions)
        return true
      },
    })
  }

  /** Resolves once the wasm engine is loaded. Not part of xterm.js. */
  get ready(): Promise<void> {
    return this.termy.ready
  }

  get element(): HTMLElement | undefined {
    return this.termy.element
  }

  get textarea(): HTMLTextAreaElement | undefined {
    return this.termy.textarea
  }

  get rows(): number {
    return this.termy.rows
  }

  get cols(): number {
    return this.termy.cols
  }

  get buffer(): IBufferNamespace {
    return this.#buffer
  }

  get modes(): IModes {
    const modes = this.termy.modes
    return {
      applicationCursorKeysMode: modes?.applicationCursor ?? false,
      applicationKeypadMode: modes?.applicationKeypad ?? false,
      bracketedPasteMode: modes?.bracketedPaste ?? false,
      insertMode: false,
      mouseTrackingMode: modes?.mouseTracking ? 'vt200' : 'none',
      originMode: false,
      reverseWraparoundMode: false,
      sendFocusMode: modes?.focusEvents ?? false,
      synchronizedOutputMode: modes?.synchronizedUpdate ?? false,
      wraparoundMode: true,
    }
  }

  get options(): ITerminalOptions {
    return this.#optionsProxy
  }

  set options(options: ITerminalOptions) {
    Object.assign(this.#options, options)
    this.#applyOptions(options)
  }

  open(parent: HTMLElement): void {
    this.termy.open(parent)
    const element = this.termy.element
    if (element) {
      element.classList.add('xterm')
      element.addEventListener(
        'wheel',
        (event) => {
          if (this.#customWheelHandler && this.#customWheelHandler(event) === false) event.stopImmediatePropagation()
        },
        { capture: true },
      )
    }
  }

  write(data: string | Uint8Array, callback?: () => void): void {
    if (this.#onLineFeed.hasListeners) {
      const lineFeeds = typeof data === 'string' ? data.split('\n').length - 1 : data.filter((b) => b === 10).length
      this.termy.write(data, () => {
        for (let i = 0; i < lineFeeds; i++) this.#onLineFeed.fire()
        callback?.()
      })
      return
    }
    this.termy.write(data, callback)
  }

  writeln(data: string | Uint8Array, callback?: () => void): void {
    if (typeof data === 'string') this.write(`${data}\r\n`, callback)
    else {
      this.write(data)
      this.write('\r\n', callback)
    }
  }

  paste(data: string): void {
    this.termy.paste(data)
  }

  input(data: string, wasUserInput = true): void {
    this.termy.input(data, wasUserInput)
  }

  resize(columns: number, rows: number): void {
    this.termy.resize(columns, rows)
  }

  focus(): void {
    this.termy.focus()
  }

  blur(): void {
    this.termy.blur()
  }

  clear(): void {
    this.termy.clear()
  }

  reset(): void {
    this.termy.reset()
  }

  refresh(_start: number, _end: number): void {
    this.termy.setOptions({})
  }

  clearTextureAtlas(): void {}

  scrollLines(amount: number): void {
    this.termy.scrollLines(amount)
  }

  scrollPages(pageCount: number): void {
    this.termy.scrollPages(pageCount)
  }

  scrollToTop(): void {
    this.termy.scrollToTop()
  }

  scrollToBottom(): void {
    this.termy.scrollToBottom()
  }

  scrollToLine(line: number): void {
    this.termy.scrollToLine(line)
  }

  hasSelection(): boolean {
    return this.termy.hasSelection()
  }

  getSelection(): string {
    return this.termy.getSelection()
  }

  getSelectionPosition(): IBufferRange | undefined {
    const range = this.termy.getSelectionPosition()
    if (!range) return undefined
    return {
      start: { x: range.start.col, y: range.start.line },
      end: { x: Math.min(range.end.col, this.cols), y: range.end.line },
    }
  }

  clearSelection(): void {
    this.termy.clearSelection()
  }

  select(column: number, row: number, length: number): void {
    this.termy.select(column, row, length)
  }

  selectAll(): void {
    this.termy.selectAll()
  }

  selectLines(start: number, end: number): void {
    this.termy.selectLines(start, end)
  }

  attachCustomKeyEventHandler(handler: (event: KeyboardEvent) => boolean): void {
    this.termy.attachCustomKeyEventHandler(handler)
  }

  attachCustomWheelEventHandler(handler: (event: WheelEvent) => boolean): void {
    this.#customWheelHandler = handler
  }

  /** Track a line relative to the cursor. Termy markers do not move on trim. */
  registerMarker(cursorYOffset = 0): IMarker {
    const onDispose = new Emitter<void>()
    const line = this.termy.historySize + (this.termy.cursor?.row ?? 0) + cursorYOffset
    const marker: IMarker = {
      id: nextMarkerId++,
      isDisposed: false,
      line,
      onDispose: onDispose.event,
      dispose: () => {
        if (marker.isDisposed) return
        ;(marker as { isDisposed: boolean }).isDisposed = true
        const index = this.markers.indexOf(marker)
        if (index >= 0) this.markers.splice(index, 1)
        onDispose.fire()
        onDispose.dispose()
      },
    }
    this.markers.push(marker)
    return marker
  }

  /** Decorations are not supported yet. */
  registerDecoration(_options: unknown): undefined {
    return undefined
  }

  /**
   * Custom link providers are not supported yet; OSC 8 links and detected
   * URLs are handled natively through `options.linkHandler`.
   */
  registerLinkProvider(_provider: unknown): IDisposable {
    return { dispose() {} }
  }

  /** Ligature joiners are not supported; returns a dummy id. */
  registerCharacterJoiner(_handler: (text: string) => [number, number][]): number {
    return -1
  }

  deregisterCharacterJoiner(_joinerId: number): void {}

  loadAddon(addon: ITerminalAddon): void {
    this.#addons.push(addon)
    addon.activate(this)
  }

  dispose(): void {
    for (const addon of this.#addons.splice(0)) addon.dispose()
    for (const marker of [...this.markers]) marker.dispose()
    this.#onLineFeed.dispose()
    this.#onBufferChange.dispose()
    this.termy.dispose()
  }

  #applyOptions(options: ITerminalOptions): void {
    const themeChange = 'theme' in options || 'termyTheme' in options
    const mapped = toTermyOptions(
      themeChange ? { ...options, theme: this.#options.theme, termyTheme: this.#options.termyTheme } : options,
    )
    this.termy.setOptions(mapped)
  }
}
