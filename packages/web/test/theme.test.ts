import { readFileSync } from 'node:fs'
import { beforeAll, describe, expect, it } from 'vitest'
import { initSync } from '@termysh/core'
import { contrastRatio, ensureContrast, parseColor, queryColors, resolveTheme, rgba } from '../src/theme.ts'
import { translateKey } from '../src/input/keys.ts'

beforeAll(() => {
  initSync(readFileSync(new URL('../../core/src/wasm/termy_wasm_bg.wasm', import.meta.url)))
})

describe('theme', () => {
  it('parses CSS colors', () => {
    expect(parseColor('#f00')?.value).toBe(0xff0000ff)
    expect(parseColor('#11223380')?.value).toBe(0x11223380)
    expect(parseColor('rgba(1, 2, 3, 0.5)')?.value).toBe(0x01020380)
  })

  it('resolves bundled themes with overrides and a full 256 palette', () => {
    const base = resolveTheme('dracula')
    const theme = resolveTheme({ extends: 'dracula', cursor: '#00ff00' })
    expect(theme.background).toEqual(base.background)
    expect(theme.cursor.value).toBe(0x00ff00ff)
    expect(theme.palette).toHaveLength(256)
    expect(theme.palette[16]?.value).toBe(0x000000ff)
    expect(theme.palette[231]?.value).toBe(0xffffffff)
    expect(queryColors(theme)).toHaveLength(19)
  })

  it('enforces a minimum contrast ratio', () => {
    const bg = rgba(0, 0, 0)
    const fixed = ensureContrast(rgba(20, 20, 20), bg, 4.5)
    expect(contrastRatio(fixed, bg)).toBeGreaterThanOrEqual(4.5)
  })
})

describe('keys', () => {
  const key = (init: KeyboardEventInit): KeyboardEvent => init as unknown as KeyboardEvent
  it('maps DOM keys to native encoder names', () => {
    expect(translateKey(key({ key: 'ArrowUp', code: 'ArrowUp' }), false)).toMatchObject({ key: 'up' })
    expect(translateKey(key({ key: 'A', code: 'KeyA', shiftKey: true }), false)).toMatchObject({ key: 'a', text: 'A', modifiers: 4 })
    expect(translateKey(key({ key: '!', code: 'Digit1', shiftKey: true }), false)).toMatchObject({ key: '1', text: '!' })
    expect(translateKey(key({ key: 'F5', code: 'F5' }), false)).toMatchObject({ key: 'f5' })
    expect(translateKey(key({ key: 'Dead', code: 'Quote' }), false)).toBeUndefined()
  })
})
