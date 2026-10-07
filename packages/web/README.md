# @termysh/web

A browser terminal on Termy's WebAssembly engine: damage-driven canvas
rendering, kitty keyboard and graphics protocols, Termy's pixel-exact box and
block glyphs, OSC 8 links, selection, IME, themes and live options.

```ts
import { Terminal } from '@termysh/web'

const term = new Terminal({ theme: 'tokyo-night', fontSize: 13 })
term.open(document.getElementById('terminal')!)
term.onData((data) => socket.send(data))
socket.onmessage = (event) => term.write(event.data)
```

Options, themes and API: https://github.com/termylabs/termy/tree/main/packages
