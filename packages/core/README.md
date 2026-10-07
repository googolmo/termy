# @termysh/core

Termy's terminal engine compiled to WebAssembly, with a typed headless API.
Works in browsers, workers, Node and Bun. No DOM, no I/O.

```ts
import { init, TermyCore } from '@termysh/core'

await init()
const term = new TermyCore({ cols: 80, rows: 24, scrollback: 1000 })
term.write('\x1b[1;32mhello\x1b[0m\r\n')
term.lineText(0)                 // 'hello'
term.encodeKey('up', undefined, 0) // bytes for the host, mode-aware
const damage = term.takeDamage()
const cells = term.readRows(0, term.rows)
```

For a ready-made browser terminal use `@termysh/web`; to replace xterm.js use
`@termysh/xterm`. Design and full docs: https://github.com/termylabs/termy/tree/main/packages
