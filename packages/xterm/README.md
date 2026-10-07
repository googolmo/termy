# @termysh/xterm

xterm.js-compatible API on Termy's WebAssembly terminal. Swap the import:

```diff
-import { Terminal } from '@xterm/xterm'
-import { FitAddon } from '@xterm/addon-fit'
+import { Terminal, FitAddon } from '@termysh/xterm'
```

Includes `FitAddon`, `WebLinksAddon` and `SearchAddon` drop-ins; WebGL, Canvas
and Unicode11 addons become no-ops because Termy renders and measures widths
natively. Not yet supported: decorations, custom link providers and parser
hooks. Details: https://github.com/termylabs/termy/tree/main/packages
