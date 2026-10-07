import { Modifier } from '@termysh/core'

/** A DOM keyboard event translated into the native encoder's vocabulary. */
export interface TermyKey {
  key: string
  text: string | undefined
  modifiers: number
  /** Option produced a composed character that should be sent as Alt+base. */
  optionAsAlt: boolean
}

const NAMED: Record<string, string> = {
  Enter: 'enter',
  Tab: 'tab',
  Escape: 'escape',
  Backspace: 'backspace',
  Delete: 'delete',
  Insert: 'insert',
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  Home: 'home',
  End: 'end',
  PageUp: 'pageup',
  PageDown: 'pagedown',
  ' ': 'space',
  ContextMenu: 'menu',
  Pause: 'pause',
  PrintScreen: 'printscreen',
  ScrollLock: 'scrolllock',
  NumLock: 'numlock',
  CapsLock: 'capslock',
  Shift: 'shift',
  Control: 'control',
  Alt: 'alt',
  Meta: 'super',
}

/** US-layout base characters, used for shifted symbols and macOS Option-as-Meta. */
const CODE_BASE: Record<string, string> = {
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Space: ' ',
}

function baseFromCode(code: string): string | undefined {
  if (code.startsWith('Key') && code.length === 4) return code.slice(3).toLowerCase()
  if (code.startsWith('Digit') && code.length === 6) return code.slice(5)
  if (code.startsWith('Numpad') && /^\d$/.test(code.slice(6))) return code.slice(6)
  return CODE_BASE[code]
}

export function isMac(): boolean {
  return typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent)
}

export function modifiersOf(event: KeyboardEvent | MouseEvent): number {
  return (
    (event.ctrlKey ? Modifier.Ctrl : 0) |
    (event.altKey ? Modifier.Alt : 0) |
    (event.shiftKey ? Modifier.Shift : 0) |
    (event.metaKey ? Modifier.Meta : 0)
  )
}

export function translateKey(event: KeyboardEvent, macOptionIsMeta: boolean): TermyKey | undefined {
  const modifiers = modifiersOf(event)
  const named = NAMED[event.key]
  if (named) {
    return { key: named, text: event.key === ' ' ? ' ' : undefined, modifiers, optionAsAlt: false }
  }
  if (/^F\d{1,2}$/.test(event.key)) {
    return { key: event.key.toLowerCase(), text: undefined, modifiers, optionAsAlt: false }
  }
  if ([...event.key].length !== 1) return undefined

  const optionAsAlt = macOptionIsMeta && event.altKey && isMac()
  const base = baseFromCode(event.code)
  let key: string
  if (optionAsAlt && base) key = base
  else if (/^\p{L}$/u.test(event.key)) key = event.key.toLowerCase()
  else key = event.shiftKey && base ? base : event.key
  return { key, text: event.key, modifiers, optionAsAlt }
}
