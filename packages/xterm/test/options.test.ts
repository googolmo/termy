import { describe, expect, it } from 'vitest'
import { XTERM_DEFAULT_THEME, toTermyOptions } from '../src/options.ts'

describe('xterm option mapping', () => {
  it('passes through shared options and drops xterm-only ones', () => {
    const options = toTermyOptions({ fontSize: 13, cursorStyle: 'bar', scrollback: 5000, logLevel: 'off', windowsMode: true })
    expect(options).toEqual({ fontSize: 13, cursorStyle: 'bar', scrollback: 5000 })
  })

  it('defaults to xterm.js colors and layers theme overrides', () => {
    const { theme } = toTermyOptions({ theme: { background: '#101010' } })
    expect(theme).toMatchObject({ ...XTERM_DEFAULT_THEME, background: '#101010' })
  })

  it('can start from a bundled Termy theme', () => {
    const { theme } = toTermyOptions({ termyTheme: 'nord', theme: { cursor: '#fff' } })
    expect(theme).toMatchObject({ extends: 'nord', cursor: '#fff' })
  })

  it('adapts xterm link handlers', () => {
    const calls: string[] = []
    const { linkHandler } = toTermyOptions({ linkHandler: { activate: (_e, text) => calls.push(text) } })
    linkHandler?.activate({} as MouseEvent, 'https://termy.sh')
    expect(calls).toEqual(['https://termy.sh'])
  })
})
