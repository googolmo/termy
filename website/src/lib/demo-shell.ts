// A tiny in-page shell for the /demo terminal. It runs entirely in the
// browser and only exists to exercise the renderer: colors, styles, glyphs,
// links and kitty graphics.

import { gitConfig } from './shared';

const ESC = '\x1b';
const CSI = `${ESC}[`;
const reset = `${CSI}0m`;
const dim = (text: string) => `${CSI}2m${text}${reset}`;
const bold = (text: string) => `${CSI}1m${text}${reset}`;
const fg = (hex: number, text: string) =>
  `${CSI}38;2;${(hex >> 16) & 255};${(hex >> 8) & 255};${hex & 255}m${text}${reset}`;
const link = (url: string, text: string) => `${ESC}]8;;${url}${ESC}\\${text}${ESC}]8;;${ESC}\\`;

const PROMPT = `${fg(0x7aa2f7, '❯')} `;

export interface DemoHost {
  write(data: string): void;
  clear(): void;
  setTheme(id: string): void;
  themes(): string[];
  readonly cols: number;
}

interface Command {
  summary: string;
  run(host: DemoHost, args: string[]): void;
}

const COMMANDS: Record<string, Command> = {
  help: {
    summary: 'list commands',
    run(host) {
      host.write(`${bold('Commands')}\r\n`);
      for (const [name, command] of Object.entries(COMMANDS)) {
        host.write(`  ${fg(0x9ece6a, name.padEnd(10))}${dim(command.summary)}\r\n`);
      }
      host.write(`\r\n${dim('↑/↓ history · Ctrl+L clear · Ctrl+C cancel · scroll and select with the mouse')}\r\n`);
    },
  },
  colors: {
    summary: '16, 256 and truecolor palettes',
    run(host) {
      const swatch = (code: number) => `${CSI}48;5;${code}m  ${reset}`;
      host.write(`${dim('ansi')}\r\n`);
      host.write(`${Array.from({ length: 8 }, (_, i) => swatch(i)).join('')}\r\n`);
      host.write(`${Array.from({ length: 8 }, (_, i) => swatch(i + 8)).join('')}\r\n`);
      host.write(`${dim('256')}\r\n`);
      for (let row = 0; row < 6; row++) {
        host.write(`${Array.from({ length: 36 }, (_, i) => `${CSI}48;5;${16 + row * 36 + i}m ${reset}`).join('')}\r\n`);
      }
      host.write(`${Array.from({ length: 24 }, (_, i) => `${CSI}48;5;${232 + i}m ${reset}`).join('')}\r\n`);
      host.write(`${dim('truecolor')}\r\n`);
      const width = Math.min(72, host.cols - 2);
      let bar = '';
      for (let i = 0; i < width; i++) {
        const t = i / (width - 1);
        const r = Math.round(122 + (247 - 122) * t);
        const g = Math.round(162 - (162 - 118) * t);
        const b = Math.round(247 - (247 - 142) * t);
        bar += `${CSI}48;2;${r};${g};${b}m `;
      }
      host.write(`${bar}${reset}\r\n`);
    },
  },
  styles: {
    summary: 'text attributes and underline styles',
    run(host) {
      host.write(
        [
          `${CSI}1mbold${reset}  ${CSI}2mdim${reset}  ${CSI}3mitalic${reset}  ${CSI}7minverse${reset}  ${CSI}9mstrike${reset}`,
          `${CSI}4msingle${reset}  ${CSI}4:2mdouble${reset}  ${CSI}4:3m${CSI}58;2;247;118;142mcurly${reset}  ${CSI}4:4mdotted${reset}  ${CSI}4:5mdashed${reset}`,
          `${link('https://termy.sh', 'OSC 8 hyperlink')}  and a detected URL: https://github.com/${gitConfig.user}/${gitConfig.repo}`,
          `wide: 漢字 かな 한글   emoji: 🦀 🚀   combining: é ä`,
          '',
        ].join('\r\n'),
      );
    },
  },
  glyphs: {
    summary: "box drawing, blocks and Braille with Termy's geometry",
    run(host) {
      host.write(
        [
          '╭─────────────┬──────────────╮   ┏━━━━━┳━━━━━┓',
          '│ rounded box │ ▁▂▃▄▅▆▇█     │   ┃ ╲ ╱ ┃ ░▒▓ ┃',
          '├─────────────┼──────────────┤   ┣━━━━━╋━━━━━┫',
          '│ ▏▎▍▌▋▊▉█    │ ⣀⣤⣶⣿⣿⣶⣤⣀     │   ┃ ╱ ╲ ┃ ▚▞▙ ┃',
          '╰─────────────┴──────────────╯   ┗━━━━━┻━━━━━┛',
          '🬀🬁🬂🬃🬄🬅🬆🬇🬈🬉🬊🬋  ╔══╦══╗  ═╪═  ┼┼┼',
          '',
        ].join('\r\n'),
      );
    },
  },
  image: {
    summary: 'draw an image with the kitty graphics protocol',
    run(host) {
      const size = 48;
      const pixels = new Uint8Array(size * size * 4);
      for (let y = 0; y < size; y++) {
        for (let x = 0; x < size; x++) {
          const dx = x - size / 2;
          const dy = y - size / 2;
          const inside = dx * dx + dy * dy < (size / 2) ** 2;
          const i = (y * size + x) * 4;
          pixels[i] = 122 + Math.round((125 * x) / size);
          pixels[i + 1] = 162 - Math.round((60 * y) / size);
          pixels[i + 2] = 247;
          pixels[i + 3] = inside ? 255 : 0;
        }
      }
      let binary = '';
      for (const byte of pixels) binary += String.fromCharCode(byte);
      const payload = btoa(binary);
      // Kitty sends large payloads in 4096-byte chunks with m=1 until the last.
      const chunks = payload.match(/.{1,4096}/g) ?? [];
      chunks.forEach((chunk, index) => {
        const more = index < chunks.length - 1 ? 1 : 0;
        const control = index === 0 ? `a=T,f=32,s=${size},v=${size},c=10,r=5,m=${more}` : `m=${more}`;
        host.write(`${ESC}_G${control};${chunk}${ESC}\\`);
      });
      host.write(`\r\n${dim('48×48 RGBA, placed over 10×5 cells')}\r\n`);
    },
  },
  themes: {
    summary: 'list bundled themes',
    run(host) {
      host.write(`${host.themes().join('  ')}\r\n${dim('switch with: theme <name>')}\r\n`);
    },
  },
  theme: {
    summary: 'switch theme, e.g. theme dracula',
    run(host, [id]) {
      if (!id || !host.themes().includes(id)) {
        host.write(`unknown theme ${id ?? ''}. Try ${fg(0x9ece6a, 'themes')}\r\n`);
        return;
      }
      host.setTheme(id);
    },
  },
  echo: {
    summary: 'print arguments',
    run(host, args) {
      host.write(`${args.join(' ')}\r\n`);
    },
  },
  clear: {
    summary: 'clear the screen',
    run(host) {
      host.clear();
    },
  },
  about: {
    summary: 'what is running here',
    run(host) {
      host.write(
        [
          `This is ${bold('@termysh/web')}: Termy's Rust terminal engine compiled to`,
          'WebAssembly, drawn on a canvas. There is no server; this shell runs in',
          `your browser. Docs: ${link('/docs/developer/web', 'termy.sh/docs/developer/web')}`,
          '',
        ].join('\r\n'),
      );
    },
  },
};

export function banner(): string {
  return [
    `${fg(0x7aa2f7, bold('❯_ termy'))} ${dim('· web terminal demo')}`,
    `Type ${fg(0x9ece6a, 'help')} to see what it can do.`,
    '',
    '',
  ].join('\r\n');
}

/** Line editor and command dispatch. Feed it `onData` input. */
export class DemoShell {
  #host: DemoHost;
  #line = '';
  #history: string[] = [];
  #historyIndex = 0;

  constructor(host: DemoHost) {
    this.#host = host;
  }

  start(): void {
    this.#host.write(banner());
    this.#host.write(PROMPT);
  }

  run(command: string): void {
    this.#host.write(`${command}\r\n`);
    this.#execute(command);
  }

  input(data: string): void {
    if (data === `${CSI}A` || data === `${ESC}OA`) return this.#recall(-1);
    if (data === `${CSI}B` || data === `${ESC}OB`) return this.#recall(1);
    if (data.startsWith(ESC)) return;
    for (const ch of data) {
      if (ch === '\r') {
        this.#host.write('\r\n');
        this.#execute(this.#line);
        this.#line = '';
      } else if (ch === '\x7f' || ch === '\b') {
        if (this.#line) {
          this.#line = [...this.#line].slice(0, -1).join('');
          this.#host.write('\b \b');
        }
      } else if (ch === '\x03') {
        this.#host.write('^C\r\n' + PROMPT);
        this.#line = '';
      } else if (ch === '\x0c') {
        this.#host.clear();
        this.#host.write(PROMPT + this.#line);
      } else if (ch >= ' ') {
        this.#line += ch;
        this.#host.write(ch);
      }
    }
  }

  #execute(line: string): void {
    const [name = '', ...args] = line.trim().split(/\s+/);
    if (name) {
      this.#history.push(line.trim());
      const command = COMMANDS[name];
      if (command) command.run(this.#host, args);
      else this.#host.write(`command not found: ${name}. Try ${fg(0x9ece6a, 'help')}\r\n`);
    }
    this.#historyIndex = this.#history.length;
    this.#host.write(PROMPT);
  }

  #recall(step: number): void {
    const index = Math.max(0, Math.min(this.#history.length, this.#historyIndex + step));
    if (index === this.#historyIndex) return;
    this.#historyIndex = index;
    const next = this.#history[index] ?? '';
    this.#host.write(`\r${CSI}2K${PROMPT}${next}`);
    this.#line = next;
  }
}

export const DEMO_COMMANDS: string[] = ['colors', 'styles', 'glyphs', 'image', 'about'];
