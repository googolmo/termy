import { Terminal, builtinThemeIds } from '@termysh/web'
import { Terminal as XTerm, FitAddon, WebLinksAddon } from '@termysh/xterm'

const ESC = '\x1b'
const sample = [
  `${ESC}[1mTermy${ESC}[0m on WebAssembly — ${ESC}]8;;https://termy.sh${ESC}\\termy.sh${ESC}]8;;${ESC}\\ and https://github.com/termylabs/termy`,
  '',
  [...Array(16).keys()].map((i) => `${ESC}[48;5;${i}m  `).join('') + `${ESC}[0m`,
  [...Array(36).keys()].map((i) => `${ESC}[48;2;${i * 7};${120 - i * 3};${255 - i * 7}m `).join('') + `${ESC}[0m`,
  `${ESC}[1mbold${ESC}[0m ${ESC}[3mitalic${ESC}[0m ${ESC}[4munderline${ESC}[0m ${ESC}[4:3m${ESC}[58;2;255;80;80mcurly${ESC}[0m ${ESC}[9mstrike${ESC}[0m ${ESC}[7minverse${ESC}[0m ${ESC}[2mdim${ESC}[0m`,
  '╭──────────┬─────────╮',
  '│ box      │ ▀▄█▌▐░▒▓ │',
  '├──────────┼─────────┤',
  '│ wide 漢字 │ ⣿⣶⣤⣀🬗🬤 │',
  '╰──────────┴─────────╯',
  '🦀 emoji, wide 漢字 and e\u0301 combining',
  '',
].join('\r\n')

function kittyImage(term) {
  // 16x16 RGBA gradient via the kitty graphics protocol.
  const size = 16
  const pixels = new Uint8Array(size * size * 4)
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const i = (y * size + x) * 4
    pixels.set([x * 16, y * 16, 200, 255], i)
  }
  const b64 = btoa(String.fromCharCode(...pixels))
  term.write(`kitty image: ${ESC}_Ga=T,f=32,s=${size},v=${size},c=8,r=4;${b64}${ESC}\\\r\n\r\n\r\n\r\n`)
}

function shell(term, prompt) {
  let line = ''
  term.write(prompt)
  term.onData((data) => {
    for (const ch of data) {
      if (ch === '\r') { term.write(`\r\nyou typed: ${line}\r\n${prompt}`); line = '' }
      else if (ch === '\x7f') { if (line) { line = line.slice(0, -1); term.write('\b \b') } }
      else if (ch >= ' ') { line += ch; term.write(ch) }
    }
  })
}

const web = new Terminal({ theme: 'tokyo-night', fontSize: 14, padding: 8 })
web.open(document.getElementById('web'))
await web.ready
web.write(sample)
kittyImage(web)
for (let i = 0; i < 40; i++) web.writeln(`scrollback line ${i}`)
shell(web, `${ESC}[32m❯${ESC}[0m `)
web.focus()
window.web = web

const xterm = new XTerm({ cols: 60, rows: 20, cursorBlink: true })
const fit = new FitAddon()
xterm.loadAddon(fit)
xterm.loadAddon(new WebLinksAddon())
xterm.open(document.getElementById('xterm'))
await xterm.ready
fit.fit()
xterm.write(sample)
shell(xterm, '$ ')
window.xterm = xterm

const themeSelect = document.getElementById('theme')
for (const id of builtinThemeIds()) themeSelect.add(new Option(id, id, false, id === 'tokyo-night'))
themeSelect.onchange = () => web.setOptions({ theme: themeSelect.value })
document.getElementById('size').oninput = (e) => {
  web.setOptions({ fontSize: Number(e.target.value) })
  xterm.options.fontSize = Number(e.target.value)
  fit.fit()
}
document.getElementById('cursor').onchange = (e) => web.setOptions({ cursorStyle: e.target.value })
document.getElementById('blink').onchange = (e) => web.setOptions({ cursorBlink: e.target.checked })
window.addEventListener('resize', () => fit.fit())
