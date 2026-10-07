import { Attr, CELL_STRIDE, ColorTag, Modifier, STRING_FLAG, Slot, TermyCore, Width, builtinTheme, builtinTheme as builtinTheme$1, builtinThemeIds, glyphPlan, init, init as init$1, initSync, isInitialized, isInitialized as isInitialized$1, underlineStyle } from "./core.js";
//#region src/emitter.ts
var Emitter = class {
	#listeners = /* @__PURE__ */ new Set();
	event = (listener) => {
		this.#listeners.add(listener);
		return { dispose: () => this.#listeners.delete(listener) };
	};
	get hasListeners() {
		return this.#listeners.size > 0;
	}
	fire(value) {
		for (const listener of [...this.#listeners]) listener(value);
	}
	dispose() {
		this.#listeners.clear();
	}
};
function toDisposable(dispose) {
	return { dispose };
}
//#endregion
//#region src/input/keys.ts
const NAMED = {
	Enter: "enter",
	Tab: "tab",
	Escape: "escape",
	Backspace: "backspace",
	Delete: "delete",
	Insert: "insert",
	ArrowUp: "up",
	ArrowDown: "down",
	ArrowLeft: "left",
	ArrowRight: "right",
	Home: "home",
	End: "end",
	PageUp: "pageup",
	PageDown: "pagedown",
	" ": "space",
	ContextMenu: "menu",
	Pause: "pause",
	PrintScreen: "printscreen",
	ScrollLock: "scrolllock",
	NumLock: "numlock",
	CapsLock: "capslock",
	Shift: "shift",
	Control: "control",
	Alt: "alt",
	Meta: "super"
};
/** US-layout base characters, used for shifted symbols and macOS Option-as-Meta. */
const CODE_BASE = {
	Backquote: "`",
	Minus: "-",
	Equal: "=",
	BracketLeft: "[",
	BracketRight: "]",
	Backslash: "\\",
	Semicolon: ";",
	Quote: "'",
	Comma: ",",
	Period: ".",
	Slash: "/",
	Space: " "
};
function baseFromCode(code) {
	if (code.startsWith("Key") && code.length === 4) return code.slice(3).toLowerCase();
	if (code.startsWith("Digit") && code.length === 6) return code.slice(5);
	if (code.startsWith("Numpad") && /^\d$/.test(code.slice(6))) return code.slice(6);
	return CODE_BASE[code];
}
function isMac() {
	return typeof navigator !== "undefined" && /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent);
}
function modifiersOf(event) {
	return (event.ctrlKey ? Modifier.Ctrl : 0) | (event.altKey ? Modifier.Alt : 0) | (event.shiftKey ? Modifier.Shift : 0) | (event.metaKey ? Modifier.Meta : 0);
}
function translateKey(event, macOptionIsMeta) {
	const modifiers = modifiersOf(event);
	const named = NAMED[event.key];
	if (named) return {
		key: named,
		text: event.key === " " ? " " : void 0,
		modifiers,
		optionAsAlt: false
	};
	if (/^F\d{1,2}$/.test(event.key)) return {
		key: event.key.toLowerCase(),
		text: void 0,
		modifiers,
		optionAsAlt: false
	};
	if ([...event.key].length !== 1) return void 0;
	const optionAsAlt = macOptionIsMeta && event.altKey && isMac();
	const base = baseFromCode(event.code);
	let key;
	if (optionAsAlt && base) key = base;
	else if (/^\p{L}$/u.test(event.key)) key = event.key.toLowerCase();
	else key = event.shiftKey && base ? base : event.key;
	return {
		key,
		text: event.key,
		modifiers,
		optionAsAlt
	};
}
//#endregion
//#region src/links.ts
const URL_PATTERN = /(?:https?:\/\/|mailto:|file:\/\/)[^\s<>"'`]*[^\s<>"'`.,;:!?)\]}]/g;
const SAFE_SCHEMES = /^(https?:|mailto:)/i;
function isSafeLink(uri, allowNonHttp) {
	return allowNonHttp || SAFE_SCHEMES.test(uri);
}
/** The link under viewport cell `(row, col)`: OSC 8 first, then detected URLs. */
function linkAt(core, row, col, detect) {
	const cells = core.readRows(row, row + 1);
	const uri = cells.link(0, col);
	if (uri) {
		let start = col;
		let end = col + 1;
		while (start > 0 && cells.link(0, start - 1) === uri) start--;
		while (end < cells.cols && cells.link(0, end) === uri) end++;
		return {
			uri,
			row,
			start,
			end,
			explicit: true
		};
	}
	if (!detect) return void 0;
	let text = "";
	const columns = [];
	for (let c = 0; c < cells.cols; c++) {
		if (cells.data[c * CELL_STRIDE + Slot.Style] >>> 16 & 7 & (Width.WideSpacer | Width.LeadingWideSpacer)) continue;
		const ch = cells.text(0, c);
		for (let i = 0; i < ch.length; i++) columns.push(c);
		text += ch;
	}
	columns.push(cells.cols);
	for (const match of text.matchAll(URL_PATTERN)) {
		const start = columns[match.index] ?? 0;
		const end = columns[match.index + match[0].length] ?? cells.cols;
		if (col >= start && col < end) return {
			uri: match[0],
			row,
			start,
			end,
			explicit: false
		};
	}
}
//#endregion
//#region src/options.ts
const DEFAULT_FONT_FAMILY = "\"JetBrains Mono\", \"SF Mono\", Menlo, Monaco, Consolas, \"Liberation Mono\", \"DejaVu Sans Mono\", monospace";
const DEFAULT_OPTIONS = {
	cols: 80,
	rows: 24,
	scrollback: 1e3,
	fontFamily: DEFAULT_FONT_FAMILY,
	fontSize: 14,
	fontWeight: "normal",
	fontWeightBold: "bold",
	lineHeight: 1,
	letterSpacing: 0,
	theme: void 0,
	drawBoldTextInBrightColors: true,
	minimumContrastRatio: 1,
	allowTransparency: false,
	padding: 0,
	cursorStyle: "block",
	cursorBlink: false,
	cursorWidth: 1,
	cursorInactiveStyle: "outline",
	customGlyphs: true,
	images: true,
	scrollSensitivity: 1,
	fastScrollSensitivity: 5,
	fastScrollModifier: "alt",
	alternateScroll: true,
	scrollOnUserInput: true,
	scrollbar: true,
	macOptionIsMeta: false,
	macOptionClickForcesSelection: false,
	rightClickSelectsWord: false,
	altClickMovesCursor: true,
	wordSeparator: " ()[]{}',\"`",
	copyOnSelect: false,
	convertEol: false,
	disableStdin: false,
	ignoreBracketedPasteMode: false,
	linkHandler: { activate(_event, uri) {
		window.open(uri, "_blank", "noopener,noreferrer");
	} },
	linkDetection: true,
	allowClipboardWrite: false,
	bellStyle: "none",
	screenReaderMode: false,
	autoFit: true,
	devicePixelRatio: void 0,
	wasm: void 0
};
function resolveOptions(options) {
	const resolved = { ...DEFAULT_OPTIONS };
	for (const [key, value] of Object.entries(options)) if (value !== void 0) resolved[key] = value;
	return resolved;
}
/** Options whose change needs new font metrics (and a fit). */
const METRIC_OPTIONS = /* @__PURE__ */ new Set([
	"fontFamily",
	"fontSize",
	"fontWeight",
	"fontWeightBold",
	"lineHeight",
	"letterSpacing",
	"padding",
	"devicePixelRatio"
]);
//#endregion
//#region src/theme.ts
const DEFAULT_THEME = "termy";
const ANSI_KEYS = [
	"black",
	"red",
	"green",
	"yellow",
	"blue",
	"magenta",
	"cyan",
	"white",
	"brightBlack",
	"brightRed",
	"brightGreen",
	"brightYellow",
	"brightBlue",
	"brightMagenta",
	"brightCyan",
	"brightWhite"
];
function rgba(r, g, b, a = 255) {
	return {
		value: ((r & 255) << 24 | (g & 255) << 16 | (b & 255) << 8 | a & 255) >>> 0,
		css: a === 255 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${(a / 255).toFixed(3)})`
	};
}
function channels(color) {
	const v = color.value;
	return [
		v >>> 24,
		v >>> 16 & 255,
		v >>> 8 & 255,
		v & 255
	];
}
function fromRgb24(rgb) {
	return rgba(rgb >>> 16 & 255, rgb >>> 8 & 255, rgb & 255);
}
let probe;
/** Parse any CSS color. Hex and rgb() are parsed directly; others go through canvas. */
function parseColor(input) {
	const value = input.trim();
	const hex = /^#([0-9a-f]{3,8})$/i.exec(value)?.[1];
	if (hex && [
		3,
		4,
		6,
		8
	].includes(hex.length)) {
		const full = hex.length <= 4 ? [...hex].map((c) => c + c).join("") : hex;
		const n = (i) => Number.parseInt(full.slice(i, i + 2), 16);
		return rgba(n(0), n(2), n(4), full.length === 8 ? n(6) : 255);
	}
	const fn = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+%?))?\s*\)$/i.exec(value);
	if (fn) {
		const alpha = fn[4] === void 0 ? 1 : fn[4].endsWith("%") ? Number.parseFloat(fn[4]) / 100 : Number(fn[4]);
		return rgba(Number(fn[1]), Number(fn[2]), Number(fn[3]), Math.round(alpha * 255));
	}
	if (typeof document === "undefined") return void 0;
	probe ??= document.createElement("canvas").getContext("2d");
	if (!probe) return void 0;
	probe.fillStyle = "#000";
	probe.fillStyle = value;
	const normalized = String(probe.fillStyle);
	return normalized === value ? void 0 : parseColor(normalized);
}
/** Linear blend of `top` over `bottom` with `amount` in 0..1. */
function blend(bottom, top, amount) {
	const [r1, g1, b1, a1] = channels(bottom);
	const [r2, g2, b2] = channels(top);
	const mix = (a, b) => Math.round(a + (b - a) * amount);
	return rgba(mix(r1, r2), mix(g1, g2), mix(b1, b2), a1);
}
function withAlpha(color, alpha) {
	const [r, g, b] = channels(color);
	return rgba(r, g, b, Math.round(alpha * 255));
}
function luminance(color) {
	const [r, g, b] = channels(color).map((c) => {
		const s = c / 255;
		return s <= .03928 ? s / 12.92 : ((s + .055) / 1.055) ** 2.4;
	});
	return .2126 * r + .7152 * g + .0722 * b;
}
function contrastRatio(a, b) {
	const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (l1 + .05) / (l2 + .05);
}
/** Move `fg` toward black or white until it reaches `ratio` against `bg`. */
function ensureContrast(fg, bg, ratio) {
	if (ratio <= 1 || contrastRatio(fg, bg) >= ratio) return fg;
	const target = luminance(bg) > .5 ? rgba(0, 0, 0) : rgba(255, 255, 255);
	for (let step = .1; step <= 1; step += .1) {
		const candidate = blend(fg, target, step);
		if (contrastRatio(candidate, bg) >= ratio) return candidate;
	}
	return target;
}
/** The xterm 256-color palette above the 16 theme colors. */
function extendedPalette(index) {
	if (index < 232) {
		const n = index - 16;
		const level = (v) => v === 0 ? 0 : 55 + v * 40;
		return rgba(level(Math.floor(n / 36)), level(Math.floor(n / 6) % 6), level(n % 6));
	}
	const gray = 8 + (index - 232) * 10;
	return rgba(gray, gray, gray);
}
function baseTheme(id) {
	const colors = builtinTheme$1(id) ?? builtinTheme$1("termy");
	if (!colors) return {};
	const theme = {
		foreground: colors.foreground,
		background: colors.background,
		cursor: colors.cursor
	};
	ANSI_KEYS.forEach((key, index) => {
		theme[key] = colors.ansi[index];
	});
	return theme;
}
function resolveTheme(input) {
	const overrides = typeof input === "string" ? {} : input ?? {};
	const theme = {
		...baseTheme(typeof input === "string" ? input : overrides.extends ?? "termy"),
		...stripUndefined(overrides)
	};
	const color = (value, fallback) => value && parseColor(value) || fallback;
	const foreground = color(theme.foreground, rgba(229, 229, 229));
	const background = color(theme.background, rgba(0, 0, 0));
	const palette = [];
	for (let index = 0; index < 256; index++) if (index < 16) palette.push(color(theme[ANSI_KEYS[index]], extendedPalette(16)));
	else palette.push(color(theme.extendedAnsi?.[index - 16], extendedPalette(index)));
	const selectionBackground = color(theme.selectionBackground, withAlpha(foreground, .3));
	return {
		foreground,
		background,
		cursor: color(theme.cursor, foreground),
		cursorAccent: color(theme.cursorAccent, background),
		selectionBackground,
		selectionForeground: theme.selectionForeground ? parseColor(theme.selectionForeground) : void 0,
		selectionInactiveBackground: color(theme.selectionInactiveBackground, withAlpha(foreground, .15)),
		scrollbarThumb: color(theme.scrollbarThumb, withAlpha(foreground, .35)),
		palette
	};
}
/** `[fg, bg, cursor, ansi0..15]` as 0xRRGGBB for the engine's color query replies. */
function queryColors(theme) {
	const rgb = (color) => color.value >>> 8;
	return [
		theme.foreground,
		theme.background,
		theme.cursor,
		...theme.palette.slice(0, 16)
	].map(rgb);
}
function stripUndefined(value) {
	return Object.fromEntries(Object.entries(value).filter(([, v]) => v !== void 0));
}
//#endregion
//#region src/renderer/glyphs.ts
/** Code points that may use Termy's geometry instead of the font. */
function isCustomGlyphCandidate(cp) {
	return cp >= 9472 && cp <= 9631 || cp >= 10240 && cp <= 10495 || cp >= 9698 && cp <= 9701 || cp >= 129792 && cp <= 130047;
}
const MAX_CACHE = 4096;
var GlyphCache = class {
	#plans = /* @__PURE__ */ new Map();
	#metrics;
	setMetrics(metrics) {
		this.#metrics = metrics;
		this.#plans.clear();
	}
	plan(cp, neighbors) {
		const metrics = this.#metrics;
		if (!metrics) return void 0;
		const key = `${cp},${neighbors.join(",")}`;
		let plan = this.#plans.get(key);
		if (plan === void 0) {
			plan = glyphPlan(cp, neighbors, metrics.deviceWidth, metrics.deviceHeight, metrics.fontSize) ?? null;
			if (this.#plans.size >= MAX_CACHE) this.#plans.clear();
			this.#plans.set(key, plan);
		}
		return plan ?? void 0;
	}
};
/** Paint a normalized plan into the cell at device pixel `(x, y)`. */
function drawGlyphPlan(ctx, plan, x, y, width, height) {
	for (const rect of plan.rects) {
		const outward = rect.snap === "outward";
		const left = outward ? Math.floor(x + rect.left * width) : Math.round(x + rect.left * width);
		const top = outward ? Math.floor(y + rect.top * height) : Math.round(y + rect.top * height);
		const right = outward ? Math.ceil(x + rect.right * width) : Math.round(x + rect.right * width);
		const bottom = outward ? Math.ceil(y + rect.bottom * height) : Math.round(y + rect.bottom * height);
		if (right <= left || bottom <= top) continue;
		ctx.globalAlpha = rect.alpha;
		ctx.fillRect(left, top, right - left, bottom - top);
	}
	ctx.globalAlpha = 1;
	if (plan.strokes.length === 0) return;
	ctx.save();
	ctx.strokeStyle = ctx.fillStyle;
	ctx.lineCap = "butt";
	for (const stroke of plan.strokes) {
		const p = stroke.points.map((point) => [x + point.x * width, y + point.y * height]);
		ctx.lineWidth = Math.max(1, stroke.width * width);
		ctx.beginPath();
		ctx.moveTo(p[0][0], p[0][1]);
		if (stroke.kind === "roundedCorner" && p.length >= 6) {
			ctx.lineTo(p[1][0], p[1][1]);
			ctx.bezierCurveTo(p[2][0], p[2][1], p[3][0], p[3][1], p[4][0], p[4][1]);
			ctx.lineTo(p[5][0], p[5][1]);
		} else for (const point of p.slice(1)) ctx.lineTo(point[0], point[1]);
		ctx.stroke();
	}
	ctx.restore();
}
//#endregion
//#region src/renderer/canvas.ts
/**
* Damage-driven Canvas 2D renderer. Only rows reported dirty by the engine (or
* invalidated by cursor, selection and theme changes) are repainted.
*/
var CanvasRenderer = class {
	canvas;
	#ctx;
	#core;
	#metrics;
	#theme;
	#options;
	#glyphs = new GlyphCache();
	#dirty = /* @__PURE__ */ new Set();
	#full = true;
	#paletteRevision = -1;
	#palette = new Array(256);
	#dynamic = [
		void 0,
		void 0,
		void 0
	];
	#rgbCache = /* @__PURE__ */ new Map();
	#lastCursorRow = -1;
	constructor(core) {
		this.#core = core;
		this.canvas = document.createElement("canvas");
		this.canvas.style.position = "absolute";
		this.canvas.style.left = "0";
		this.canvas.style.top = "0";
		const ctx = this.canvas.getContext("2d", { alpha: true });
		if (!ctx) throw new Error("@termysh/web: Canvas 2D is not available");
		this.#ctx = ctx;
	}
	get metrics() {
		return this.#metrics;
	}
	setMetrics(metrics) {
		this.#metrics = metrics;
		this.#glyphs.setMetrics(metrics);
		this.resize();
	}
	setTheme(theme) {
		this.#theme = theme;
		this.invalidate();
	}
	setOptions(options) {
		this.#options = options;
		this.invalidate();
	}
	/** Match the canvas to the engine's grid. */
	resize() {
		const metrics = this.#metrics;
		if (!metrics) return;
		const width = this.#core.cols * metrics.deviceWidth;
		const height = this.#core.rows * metrics.deviceHeight;
		if (this.canvas.width !== width || this.canvas.height !== height) {
			this.canvas.width = width;
			this.canvas.height = height;
		}
		this.canvas.style.width = `${width / metrics.dpr}px`;
		this.canvas.style.height = `${height / metrics.dpr}px`;
		this.invalidate();
	}
	invalidate() {
		this.#full = true;
	}
	invalidateRow(row) {
		this.#dirty.add(row);
	}
	/** Paint pending damage. Returns the viewport rows that were repainted. */
	render(cursor, selected, link) {
		const metrics = this.#metrics;
		const theme = this.#theme;
		const options = this.#options;
		if (!metrics || !theme || !options) return 0;
		const core = this.#core;
		const rows = core.rows;
		const damage = core.takeDamage();
		if (damage.full) this.#full = true;
		for (const scroll of damage.scrolls) for (let row = scroll.top; row < scroll.bottom; row++) this.#dirty.add(row);
		for (const span of damage.spans) this.#dirty.add(span.row);
		if (core.paletteRevision !== this.#paletteRevision) {
			this.#paletteRevision = core.paletteRevision;
			const overrides = core.paletteOverrides();
			for (let i = 0; i < 256; i++) this.#palette[i] = overrides[i] ? this.#raw(overrides[i]) : void 0;
			const dynamic = core.dynamicColorOverrides();
			this.#dynamic = [
				0,
				1,
				2
			].map((i) => dynamic[i] ? this.#raw(dynamic[i]) : void 0);
			this.#full = true;
		}
		const cursorRow = cursor.cursor.row + core.displayOffset;
		if (cursorRow !== this.#lastCursorRow) {
			this.#dirty.add(this.#lastCursorRow);
			this.#lastCursorRow = cursorRow;
		}
		this.#dirty.add(cursorRow);
		let first;
		let last;
		if (this.#full) {
			first = 0;
			last = rows - 1;
		} else {
			first = rows;
			last = -1;
			for (const row of this.#dirty) {
				if (row < 0 || row >= rows) continue;
				first = Math.min(first, row);
				last = Math.max(last, row);
			}
		}
		if (last < first) {
			this.#dirty.clear();
			return 0;
		}
		const cells = core.readRows(first, last + 1);
		let painted = 0;
		for (let row = first; row <= last; row++) {
			if (!this.#full && !this.#dirty.has(row)) continue;
			this.#paintRow(row, cells, row - first, cursor, cursorRow, selected, link);
			painted++;
		}
		this.#full = false;
		this.#dirty.clear();
		return painted;
	}
	#paintRow(row, cells, rel, paint, cursorRow, selected, link) {
		const ctx = this.#ctx;
		const metrics = this.#metrics;
		const theme = this.#theme;
		const options = this.#options;
		const { deviceWidth: cw, deviceHeight: ch } = metrics;
		const cols = cells.cols;
		const data = cells.data;
		const y = row * ch;
		const defaultBg = this.#dynamic[1] ?? theme.background;
		ctx.save();
		ctx.beginPath();
		ctx.rect(0, y, cols * cw, ch);
		ctx.clip();
		if (options.transparentBackground) ctx.clearRect(0, y, cols * cw, ch);
		else {
			ctx.fillStyle = defaultBg.css;
			ctx.fillRect(0, y, cols * cw, ch);
		}
		let runStart = 0;
		let runColor;
		const flush = (end) => {
			if (runColor) {
				ctx.fillStyle = runColor;
				ctx.fillRect(runStart * cw, y, (end - runStart) * cw, ch);
			}
		};
		const fgs = new Array(cols);
		for (let col = 0; col < cols; col++) {
			const at = (rel * cols + col) * CELL_STRIDE;
			const attrs = data[at + Slot.Style] & 65535;
			const isSelected = selected?.(row, col) ?? false;
			let fg = this.#foreground(data[at + Slot.Foreground], attrs);
			let bg = this.#background(data[at + Slot.Background]);
			if (attrs & Attr.Inverse) [fg, bg] = [bg ?? defaultBg, fg];
			if (isSelected) {
				const selection = paint.focused ? theme.selectionBackground : theme.selectionInactiveBackground;
				bg = blend(bg ?? defaultBg, selection, (selection.value & 255) / 255);
				if (theme.selectionForeground) fg = theme.selectionForeground;
			}
			const effectiveBg = bg ?? defaultBg;
			if (attrs & Attr.Dim) fg = blend(effectiveBg, fg, .5);
			if (attrs & Attr.Hidden) fg = effectiveBg;
			if (options.minimumContrastRatio > 1) fg = ensureContrast(fg, effectiveBg, options.minimumContrastRatio);
			fgs[col] = fg;
			const css = bg?.css;
			if (css !== runColor) {
				flush(col);
				runStart = col;
				runColor = css;
			}
		}
		flush(cols);
		ctx.textBaseline = "alphabetic";
		let currentFont = "";
		for (let col = 0; col < cols; col++) {
			const at = (rel * cols + col) * CELL_STRIDE;
			const style = data[at + Slot.Style];
			const width = style >>> 16 & 7;
			if (width & (Width.WideSpacer | Width.LeadingWideSpacer)) continue;
			const value = data[at + Slot.Text];
			const attrs = style & 65535;
			const fg = fgs[col];
			const x = col * cw;
			const span = width & Width.Wide ? 2 : 1;
			ctx.fillStyle = fg.css;
			if (value !== 32 && value !== 0 && !(attrs & Attr.Hidden)) {
				const plan = options.customGlyphs && !(value & STRING_FLAG) && isCustomGlyphCandidate(value) ? this.#glyphs.plan(value, neighbors(data, rel, cols, col)) : void 0;
				if (plan) drawGlyphPlan(ctx, plan, x, y, cw * span, ch);
				else {
					const font = fontFor(metrics, attrs);
					if (font !== currentFont) {
						ctx.font = font;
						currentFont = font;
					}
					ctx.fillText(cells.text(rel, col), x, y + metrics.baseline);
				}
			}
			const underline = underlineStyle(style);
			const linked = link && link.row === row && col >= link.start && col < link.end;
			if (underline !== "none" || linked) {
				const color = data[at + Slot.UnderlineColor];
				ctx.fillStyle = color ? this.#foreground(color, 0).css ?? fg.css : fg.css;
				drawUnderline(ctx, linked && underline === "none" ? "dashed" : underline, x, y, cw * span, metrics);
				ctx.fillStyle = fg.css;
			}
			if (attrs & Attr.Strike) ctx.fillRect(x, y + Math.round(metrics.baseline - metrics.fontSize * .3), cw * span, metrics.lineThickness);
		}
		if (row === cursorRow) this.#paintCursor(paint, cells, rel, row, currentFont);
		ctx.restore();
	}
	#paintCursor(paint, cells, rel, row, currentFont) {
		const { cursor, focused, blinkOn } = paint;
		if (!cursor.visible || cursor.blinking && !blinkOn && focused) return;
		const options = this.#options;
		const theme = this.#theme;
		const metrics = this.#metrics;
		const ctx = this.#ctx;
		const { deviceWidth: cw, deviceHeight: ch, lineThickness } = metrics;
		const col = Math.min(cursor.col, cells.cols - 1);
		const at = (rel * cells.cols + col) * CELL_STRIDE;
		const wide = (cells.data[at + Slot.Style] >>> 16 & Width.Wide) !== 0;
		const x = col * cw;
		const y = row * ch;
		const width = cw * (wide ? 2 : 1);
		const color = this.#dynamic[2] ?? theme.cursor;
		const style = focused ? cursor.shape ?? options.cursorStyle : options.cursorInactiveStyle;
		ctx.fillStyle = color.css;
		switch (style) {
			case "block": {
				ctx.fillRect(x, y, width, ch);
				const value = cells.data[at + Slot.Text];
				if (value !== 32 && value !== 0) {
					ctx.fillStyle = theme.cursorAccent.css;
					ctx.font = fontFor(metrics, cells.data[at + Slot.Style] & 65535) || currentFont;
					ctx.fillText(cells.text(rel, col), x, y + metrics.baseline);
				}
				break;
			}
			case "bar":
				ctx.fillRect(x, y, Math.max(1, Math.round(options.cursorWidth * metrics.dpr)), ch);
				break;
			case "underline":
				ctx.fillRect(x, y + ch - lineThickness * 2, width, lineThickness * 2);
				break;
			case "outline":
				ctx.strokeStyle = color.css;
				ctx.lineWidth = lineThickness;
				ctx.strokeRect(x + lineThickness / 2, y + lineThickness / 2, width - lineThickness, ch - lineThickness);
		}
	}
	#raw(raw) {
		return raw >>> 24 === ColorTag.Rgb ? this.#rgb(raw & 16777215) : this.#indexed(raw & 255);
	}
	#rgb(rgb) {
		let color = this.#rgbCache.get(rgb);
		if (!color) {
			if (this.#rgbCache.size > 4096) this.#rgbCache.clear();
			color = fromRgb24(rgb);
			this.#rgbCache.set(rgb, color);
		}
		return color;
	}
	#indexed(index) {
		return this.#palette[index] ?? this.#theme.palette[index];
	}
	#foreground(raw, attrs) {
		switch (raw >>> 24) {
			case ColorTag.Indexed: {
				let index = raw & 255;
				if (attrs & Attr.Bold && index < 8 && this.#options.drawBoldTextInBrightColors) index += 8;
				return this.#indexed(index);
			}
			case ColorTag.Rgb: return this.#rgb(raw & 16777215);
			default: return this.#dynamic[0] ?? this.#theme.foreground;
		}
	}
	#background(raw) {
		switch (raw >>> 24) {
			case ColorTag.Indexed: return this.#indexed(raw & 255);
			case ColorTag.Rgb: return this.#rgb(raw & 16777215);
			default: return;
		}
	}
};
function fontFor(metrics, attrs) {
	const bold = (attrs & Attr.Bold) !== 0;
	const italic = (attrs & Attr.Italic) !== 0;
	if (bold && italic) return metrics.fonts.boldItalic;
	if (bold) return metrics.fonts.bold;
	if (italic) return metrics.fonts.italic;
	return metrics.fonts.regular;
}
function neighbors(data, rel, cols, col) {
	const at = (c) => {
		if (c < 0 || c >= cols) return 0;
		const value = data[(rel * cols + c) * CELL_STRIDE + Slot.Text];
		return value & STRING_FLAG ? 0 : value;
	};
	return [
		at(col - 2),
		at(col - 1),
		at(col + 1),
		at(col + 2)
	];
}
function drawUnderline(ctx, style, x, y, width, metrics) {
	const t = metrics.lineThickness;
	const top = y + metrics.underlinePosition;
	switch (style) {
		case "double":
			ctx.fillRect(x, top - t, width, t);
			ctx.fillRect(x, top + t, width, t);
			break;
		case "curly": {
			ctx.save();
			ctx.strokeStyle = ctx.fillStyle;
			ctx.lineWidth = t;
			ctx.beginPath();
			const amplitude = Math.max(1, t);
			for (let i = 0; i <= width; i++) {
				const py = top + Math.sin(i / width * Math.PI * 2) * amplitude;
				if (i === 0) ctx.moveTo(x + i, py);
				else ctx.lineTo(x + i, py);
			}
			ctx.stroke();
			ctx.restore();
			break;
		}
		case "dotted":
			for (let i = 0; i < width; i += t * 2) ctx.fillRect(x + i, top, t, t);
			break;
		case "dashed":
			for (let i = 0; i < width; i += t * 6) ctx.fillRect(x + i, top, Math.min(t * 3, width - i), t);
			break;
		default: ctx.fillRect(x, top, width, t);
	}
}
//#endregion
//#region src/renderer/images.ts
/** Kitty graphics placements with z below this paint under cell backgrounds. */
const BELOW_TEXT = 0;
/**
* Paints kitty graphics into two canvases: one under the text layer (negative
* z-index) and one over it. Decoded images are cached per image generation.
*/
var ImageLayer = class {
	under;
	over;
	#bitmaps = /* @__PURE__ */ new Map();
	#placements = [];
	#revision = -1;
	#viewKey = "";
	#onReady;
	constructor(onReady) {
		this.under = layerCanvas();
		this.over = layerCanvas();
		this.#onReady = onReady;
	}
	get hasUnderImages() {
		return this.#placements.some((placement) => placement.zIndex < BELOW_TEXT);
	}
	setSize(width, height, cssWidth, cssHeight) {
		for (const canvas of [this.under, this.over]) {
			canvas.width = width;
			canvas.height = height;
			canvas.style.width = `${cssWidth}px`;
			canvas.style.height = `${cssHeight}px`;
		}
		this.#viewKey = "";
	}
	/** Re-read placements when graphics, scroll position or geometry changed. */
	update(core, metrics, enabled) {
		if (!enabled) {
			if (this.#placements.length > 0) {
				this.#placements = [];
				this.#paint(metrics);
			}
			return;
		}
		const revision = core.graphicsRevision;
		const viewKey = `${core.displayOffset},${core.historySize},${core.cols},${core.rows},${metrics.deviceWidth},${metrics.deviceHeight}`;
		if (revision === this.#revision && viewKey === this.#viewKey) return;
		this.#revision = revision;
		this.#viewKey = viewKey;
		this.#placements = core.readGraphics(metrics.width, metrics.height);
		const live = /* @__PURE__ */ new Set();
		for (const placement of this.#placements) {
			const key = imageKey(placement);
			live.add(key);
			if (!this.#bitmaps.has(key)) this.#decode(core, placement, key);
		}
		for (const [key, bitmap] of this.#bitmaps) if (!live.has(key)) {
			bitmap?.close();
			this.#bitmaps.delete(key);
		}
		this.#paint(metrics);
	}
	/** Milliseconds until the next animation frame of a visible image, or -1. */
	deadline(core) {
		return this.#placements.length === 0 ? -1 : core.graphicsDeadline();
	}
	repaint(metrics) {
		this.#paint(metrics);
	}
	dispose() {
		for (const bitmap of this.#bitmaps.values()) bitmap?.close();
		this.#bitmaps.clear();
		this.under.remove();
		this.over.remove();
	}
	#decode(core, placement, key) {
		const image = core.graphicsImage(placement.index);
		if (!image || typeof createImageBitmap !== "function") return;
		this.#bitmaps.set(key, null);
		const source = image.format === "png" ? new Blob([image.data], { type: "image/png" }) : new ImageData(new Uint8ClampedArray(image.data), placement.imageWidth, placement.imageHeight);
		createImageBitmap(source).then((bitmap) => {
			if (!this.#bitmaps.has(key)) {
				bitmap.close();
				return;
			}
			this.#bitmaps.set(key, bitmap);
			this.#onReady();
		}, () => this.#bitmaps.delete(key));
	}
	#paint(metrics) {
		const dpr = metrics.dpr;
		const contexts = [this.under.getContext("2d"), this.over.getContext("2d")];
		for (const ctx of contexts) ctx?.clearRect(0, 0, this.under.width, this.under.height);
		const ordered = [...this.#placements].sort((a, b) => a.zIndex - b.zIndex || a.placementSerial - b.placementSerial);
		for (const placement of ordered) {
			const bitmap = this.#bitmaps.get(imageKey(placement));
			const ctx = contexts[placement.zIndex < BELOW_TEXT ? 0 : 1];
			if (!bitmap || !ctx || placement.clip.width <= 0 || placement.clip.height <= 0) continue;
			ctx.save();
			ctx.beginPath();
			ctx.rect(placement.clip.left * dpr, placement.clip.top * dpr, placement.clip.width * dpr, placement.clip.height * dpr);
			ctx.clip();
			ctx.drawImage(bitmap, placement.sourceX, placement.sourceY, Math.max(1, placement.sourceWidth), Math.max(1, placement.sourceHeight), placement.draw.left * dpr, placement.draw.top * dpr, placement.draw.width * dpr, placement.draw.height * dpr);
			ctx.restore();
		}
	}
};
function imageKey(placement) {
	return `${placement.imageId}:${placement.imageGeneration}:${placement.imageWidth}x${placement.imageHeight}`;
}
function layerCanvas() {
	const canvas = document.createElement("canvas");
	canvas.style.position = "absolute";
	canvas.style.left = "0";
	canvas.style.top = "0";
	canvas.style.pointerEvents = "none";
	return canvas;
}
//#endregion
//#region src/renderer/metrics.ts
let measureContext;
function cssFont(weight, italic, sizePx, family) {
	return `${italic ? "italic " : ""}${weight} ${sizePx}px ${family}`;
}
function measureCell(options, dpr) {
	const size = options.fontSize * dpr;
	const fonts = {
		regular: cssFont(options.fontWeight, false, size, options.fontFamily),
		bold: cssFont(options.fontWeightBold, false, size, options.fontFamily),
		italic: cssFont(options.fontWeight, true, size, options.fontFamily),
		boldItalic: cssFont(options.fontWeightBold, true, size, options.fontFamily)
	};
	measureContext ??= document.createElement("canvas").getContext("2d");
	let advance = size * .6;
	let ascent = size * .8;
	let descent = size * .2;
	if (measureContext) {
		measureContext.font = fonts.regular;
		const sample = measureContext.measureText("W".repeat(32));
		if (sample.width > 0) advance = sample.width / 32;
		const box = measureContext.measureText("Mg│█");
		ascent = box.fontBoundingBoxAscent || box.actualBoundingBoxAscent || ascent;
		descent = box.fontBoundingBoxDescent || box.actualBoundingBoxDescent || descent;
	}
	const deviceWidth = Math.max(1, Math.round(advance + options.letterSpacing * dpr));
	const natural = Math.ceil(ascent + descent);
	const deviceHeight = Math.max(1, Math.round(natural * options.lineHeight));
	const baseline = Math.round((deviceHeight - natural) / 2 + ascent);
	const lineThickness = Math.max(1, Math.round(dpr));
	return {
		dpr,
		deviceWidth,
		deviceHeight,
		width: deviceWidth / dpr,
		height: deviceHeight / dpr,
		baseline,
		fontSize: size,
		underlinePosition: Math.min(deviceHeight - lineThickness, baseline + Math.max(lineThickness, Math.round(descent / 2))),
		lineThickness,
		fonts
	};
}
//#endregion
//#region src/selection.ts
function comparePoints(a, b) {
	return a.line - b.line || a.col - b.col;
}
var Selection = class {
	#anchor;
	#head;
	#mode = "char";
	#wordSeparator;
	constructor(wordSeparator) {
		this.#wordSeparator = wordSeparator;
	}
	set wordSeparator(value) {
		this.#wordSeparator = value;
	}
	get active() {
		const range = this.range();
		return !!range && comparePoints(range.start, range.end) < 0;
	}
	start(core, point, mode) {
		this.#mode = mode;
		this.#anchor = point;
		this.#head = point;
		if (mode !== "char") this.extend(core, point);
	}
	extend(_core, point) {
		if (!this.#anchor) return;
		this.#head = point;
	}
	set(range) {
		this.#mode = "char";
		this.#anchor = range.start;
		this.#head = range.end;
	}
	clear() {
		this.#anchor = void 0;
		this.#head = void 0;
	}
	range(core) {
		if (!this.#anchor || !this.#head) return void 0;
		let start = this.#anchor;
		let end = this.#head;
		if (comparePoints(end, start) < 0) [start, end] = [end, start];
		if (this.#mode === "char") return {
			start,
			end
		};
		if (this.#mode === "line" || !core) return {
			start: {
				line: start.line,
				col: 0
			},
			end: {
				line: end.line,
				col: Number.MAX_SAFE_INTEGER
			}
		};
		return {
			start: {
				line: start.line,
				col: this.#wordBounds(core, start)[0]
			},
			end: {
				line: end.line,
				col: this.#wordBounds(core, end)[1]
			}
		};
	}
	/** Whether viewport cell `(row, col)` is selected. */
	contains(core, range, row, col) {
		const line = core.historySize + row - core.displayOffset;
		if (line < range.start.line || line > range.end.line) return false;
		if (line === range.start.line && col < range.start.col) return false;
		if (line === range.end.line && col >= range.end.col) return false;
		return true;
	}
	text(core) {
		const range = this.range(core);
		if (!range) return "";
		const parts = [];
		for (let line = range.start.line; line <= range.end.line; line++) {
			const engineLine = line - core.historySize;
			const start = line === range.start.line ? range.start.col : 0;
			const end = line === range.end.line ? range.end.col : core.cols;
			const wrapped = line < range.end.line && core.lineWrapped(engineLine);
			parts.push(core.lineTextRange(engineLine, start, Math.min(end, core.cols), !wrapped) ?? "");
			if (line < range.end.line && !wrapped) parts.push("\n");
		}
		return parts.join("");
	}
	#wordBounds(core, point) {
		const engineLine = point.line - core.historySize;
		const cols = core.cols;
		const charAt = (col) => core.lineTextRange(engineLine, col, col + 1, false) ?? " ";
		const isWord = (col) => {
			const ch = charAt(col);
			return ch !== "" && !this.#wordSeparator.includes(ch);
		};
		let start = Math.min(point.col, cols - 1);
		let end = start;
		if (!isWord(start)) return [start, start + 1];
		while (start > 0 && isWord(start - 1)) start--;
		while (end < cols - 1 && isWord(end + 1)) end++;
		return [start, end + 1];
	}
};
//#endregion
//#region src/terminal.ts
const BLINK_INTERVAL = 600;
const COMPACT_DELAY = 250;
const utf8 = new TextDecoder("utf-8", { fatal: true });
const encoder = new TextEncoder();
/**
* A browser terminal backed by Termy's WebAssembly engine.
*
* ```ts
* const term = new Terminal({ theme: 'tokyo-night', fontSize: 13 })
* term.open(document.getElementById('terminal')!)
* term.onData((data) => socket.send(data))
* socket.onmessage = (event) => term.write(event.data)
* ```
*
* The constructor is synchronous. Until the wasm module is loaded, writes and
* resizes are queued; `await terminal.ready` when you need the engine.
*/
var Terminal = class Terminal {
	/** Resolves once the wasm engine is loaded and the terminal is usable. */
	ready;
	#options;
	#core;
	#pending = [];
	#disposed = false;
	#disposables = [];
	#root;
	#screen;
	#textarea;
	#scrollbar;
	#thumb;
	#liveRegion;
	#renderer;
	#images;
	#metrics;
	#theme;
	#frame = 0;
	#syncTimer;
	#compactTimer;
	#imageTimer;
	#blinkTimer;
	#blinkOn = true;
	#focused = false;
	#composing = false;
	#lastCursor;
	#lastViewport = -1;
	#title = "";
	#selection;
	#selecting = false;
	#mouseButton;
	#lastMouseCell;
	#hoverLink;
	#wheelRemainder = 0;
	#customKeyHandler;
	#onData = new Emitter();
	#onBinary = new Emitter();
	#onBytes = new Emitter();
	#onResize = new Emitter();
	#onTitleChange = new Emitter();
	#onBell = new Emitter();
	#onSelectionChange = new Emitter();
	#onScroll = new Emitter();
	#onRender = new Emitter();
	#onKey = new Emitter();
	#onCursorMove = new Emitter();
	#onWriteParsed = new Emitter();
	#onCwdChange = new Emitter();
	#onProgress = new Emitter();
	#onClipboard = new Emitter();
	#onShellIntegration = new Emitter();
	#onFocus = new Emitter();
	#onBlur = new Emitter();
	/** Text input for the host, UTF-8 decoded (keys, paste, protocol replies). */
	onData = this.#onData.event;
	/** Non-UTF-8 input (legacy X10 mouse reports) as a binary string. */
	onBinary = this.#onBinary.event;
	/** Every byte sent to the host, regardless of encoding. */
	onBytes = this.#onBytes.event;
	onResize = this.#onResize.event;
	onTitleChange = this.#onTitleChange.event;
	onBell = this.#onBell.event;
	onSelectionChange = this.#onSelectionChange.event;
	/** Fires with the new top line (absolute, 0 = oldest scrollback). */
	onScroll = this.#onScroll.event;
	onRender = this.#onRender.event;
	onKey = this.#onKey.event;
	onCursorMove = this.#onCursorMove.event;
	onWriteParsed = this.#onWriteParsed.event;
	/** OSC 7 working directory. */
	onCwdChange = this.#onCwdChange.event;
	/** OSC 9;4 progress. */
	onProgress = this.#onProgress.event;
	/** OSC 52 clipboard writes, whether or not `allowClipboardWrite` is on. */
	onClipboard = this.#onClipboard.event;
	/** OSC 133 shell integration marks. */
	onShellIntegration = this.#onShellIntegration.event;
	onFocus = this.#onFocus.event;
	onBlur = this.#onBlur.event;
	constructor(options = {}) {
		this.#options = resolveOptions(options);
		this.#selection = new Selection(this.#options.wordSeparator);
		this.#theme = { palette: [] };
		this.ready = (isInitialized$1() ? Promise.resolve() : init$1(this.#options.wasm)).then(() => {
			if (this.#disposed) return;
			this.#core = new TermyCore({
				cols: this.#options.cols,
				rows: this.#options.rows,
				scrollback: this.#options.scrollback
			});
			this.#applyTheme();
			this.#core.setDefaultCursorShape(this.#options.cursorStyle);
			if (this.#root) this.#attach();
			const pending = this.#pending;
			this.#pending = [];
			for (const operation of pending) operation();
		});
	}
	/** Construct and wait for the engine. */
	static async create(options = {}) {
		const terminal = new Terminal(options);
		await terminal.ready;
		return terminal;
	}
	get cols() {
		return this.#core?.cols ?? this.#options.cols;
	}
	get rows() {
		return this.#core?.rows ?? this.#options.rows;
	}
	/** The headless engine, for advanced integrations. Undefined until `ready`. */
	get core() {
		return this.#core;
	}
	get element() {
		return this.#root;
	}
	get textarea() {
		return this.#textarea;
	}
	get options() {
		return { ...this.#options };
	}
	get title() {
		return this.#title;
	}
	get modes() {
		return this.#core?.modes();
	}
	/** Cell size in CSS pixels. */
	get cellSize() {
		return this.#metrics ? {
			width: this.#metrics.width,
			height: this.#metrics.height
		} : void 0;
	}
	getOption(key) {
		return this.#options[key];
	}
	setOption(key, value) {
		this.setOptions({ [key]: value });
	}
	/** Update options live; fonts, theme, cursor and scrollback apply immediately. */
	setOptions(options) {
		const changed = /* @__PURE__ */ new Set();
		for (const [key, value] of Object.entries(options)) if (this.#options[key] !== value) changed.add(key);
		this.#options = resolveOptions({
			...this.#options,
			...options
		});
		if (changed.size === 0) return;
		this.#whenReady(() => {
			const core = this.#core;
			if (changed.has("theme")) this.#applyTheme();
			if (changed.has("scrollback")) core.setScrollback(this.#options.scrollback);
			if (changed.has("cursorStyle")) core.setDefaultCursorShape(this.#options.cursorStyle);
			if (changed.has("wordSeparator")) this.#selection.wordSeparator = this.#options.wordSeparator;
			if (changed.has("cursorBlink")) this.#restartBlink();
			if (changed.has("scrollbar")) this.#updateScrollbar();
			if ((changed.has("cols") || changed.has("rows")) && !this.#options.autoFit) this.resize(this.#options.cols, this.#options.rows);
			if ([...changed].some((key) => METRIC_OPTIONS.has(key))) this.#remeasure();
			else this.#configureRenderer();
			if (changed.has("autoFit") && this.#options.autoFit) this.fit();
			this.#scheduleRender();
		});
	}
	/** Mount into `parent`. The terminal fills it and, with `autoFit`, follows its size. */
	open(parent) {
		if (this.#root) throw new Error("@termysh/web: terminal is already open");
		const root = document.createElement("div");
		root.className = "termy";
		Object.assign(root.style, {
			position: "relative",
			width: "100%",
			height: "100%",
			overflow: "hidden",
			outline: "none",
			cursor: "text",
			userSelect: "none",
			webkitUserSelect: "none",
			contain: "strict"
		});
		parent.appendChild(root);
		this.#root = root;
		if (this.#core) this.#attach();
	}
	dispose() {
		if (this.#disposed) return;
		this.#disposed = true;
		cancelAnimationFrame(this.#frame);
		clearTimeout(this.#syncTimer);
		clearTimeout(this.#compactTimer);
		clearTimeout(this.#imageTimer);
		clearInterval(this.#blinkTimer);
		for (const dispose of this.#disposables) dispose();
		this.#images?.dispose();
		this.#root?.remove();
		this.#core?.dispose();
		for (const emitter of [
			this.#onData,
			this.#onBinary,
			this.#onBytes,
			this.#onResize,
			this.#onTitleChange,
			this.#onBell,
			this.#onSelectionChange,
			this.#onScroll,
			this.#onRender,
			this.#onKey,
			this.#onCursorMove,
			this.#onWriteParsed,
			this.#onCwdChange,
			this.#onProgress,
			this.#onClipboard,
			this.#onShellIntegration,
			this.#onFocus,
			this.#onBlur
		]) emitter.dispose();
	}
	/** Write program output. `callback` runs once it has been parsed. */
	write(data, callback) {
		this.#whenReady(() => {
			const core = this.#core;
			const converted = this.#options.convertEol ? convertEol(data) : data;
			core.write(converted);
			if (this.#options.screenReaderMode) this.#announce(converted);
			this.#afterOutput();
			this.#onWriteParsed.fire();
			callback?.();
		});
	}
	writeln(data, callback) {
		if (typeof data === "string") this.write(`${data}\r\n`, callback);
		else {
			this.write(data);
			this.write("\r\n", callback);
		}
	}
	/** Send text as if the user typed it (fires `onData`). */
	input(data, wasUserInput = true) {
		this.#emit(encoder.encode(data), wasUserInput);
	}
	/** Paste text, honoring bracketed paste mode. */
	paste(text) {
		this.#whenReady(() => {
			const bracketed = !this.#options.ignoreBracketedPasteMode;
			this.#emit(this.#core.encodePaste(text, bracketed), true);
		});
	}
	attachCustomKeyEventHandler(handler) {
		this.#customKeyHandler = handler;
	}
	focus() {
		this.#textarea?.focus({ preventScroll: true });
	}
	blur() {
		this.#textarea?.blur();
	}
	get hasFocus() {
		return this.#focused;
	}
	resize(cols, rows) {
		cols = Math.max(2, Math.floor(cols));
		rows = Math.max(1, Math.floor(rows));
		this.#whenReady(() => {
			const core = this.#core;
			if (core.cols === cols && core.rows === rows) return;
			core.resize(cols, rows);
			this.#options.cols = cols;
			this.#options.rows = rows;
			this.#renderer?.resize();
			this.#layout();
			this.#onResize.fire({
				cols,
				rows
			});
			this.#afterOutput();
		});
	}
	/** The grid size that fits the container, or undefined when not measurable. */
	proposeDimensions() {
		const root = this.#root;
		const metrics = this.#metrics;
		if (!root || !metrics) return void 0;
		const padding = this.#options.padding * 2;
		const width = root.clientWidth - padding;
		const height = root.clientHeight - padding;
		if (width <= 0 || height <= 0) return void 0;
		return {
			cols: Math.max(2, Math.floor(width / metrics.width)),
			rows: Math.max(1, Math.floor(height / metrics.height))
		};
	}
	/** Resize the grid to the container. */
	fit() {
		const size = this.proposeDimensions();
		if (size) this.resize(size.cols, size.rows);
		return size;
	}
	/** Lines of scrollback above the live screen. */
	get historySize() {
		return this.#core?.historySize ?? 0;
	}
	/** Absolute line at the top of the viewport (0 = oldest scrollback). */
	get viewportY() {
		const core = this.#core;
		return core ? core.historySize - core.displayOffset : 0;
	}
	/** Text of an absolute buffer line. */
	getLine(line, trimEnd = true) {
		const core = this.#core;
		return core?.lineText(line - core.historySize, trimEnd);
	}
	isLineWrapped(line) {
		const core = this.#core;
		return core ? core.lineWrapped(line - core.historySize) : false;
	}
	get cursor() {
		return this.#core?.cursor();
	}
	/** Clear scrollback and the screen, keeping the cursor line at the top. */
	clear() {
		this.#whenReady(() => {
			const core = this.#core;
			const { row } = core.cursor();
			if (row > 0 && !core.modes().alternateScreen) core.write(`\x1b[${row}S\x1b[${row}A`);
			core.clearScrollback();
			this.clearSelection();
			this.#renderer?.invalidate();
			this.#afterOutput();
		});
	}
	/** Full terminal reset (RIS). */
	reset() {
		this.#whenReady(() => {
			this.#core.write("\x1Bc");
			this.#core.clearScrollback();
			this.clearSelection();
			this.#afterOutput();
		});
	}
	/** Scroll by lines; positive moves toward the bottom (xterm.js convention). */
	scrollLines(amount) {
		const core = this.#core;
		if (core && amount !== 0 && core.scrollDisplay(-Math.trunc(amount))) this.#afterScroll();
	}
	scrollPages(pages) {
		this.scrollLines(pages * (this.rows - 1));
	}
	scrollToTop() {
		this.scrollLines(-this.historySize);
	}
	scrollToBottom() {
		if (this.#core?.scrollToBottom()) this.#afterScroll();
	}
	/** Scroll so absolute `line` is at the top of the viewport. */
	scrollToLine(line) {
		this.scrollLines(line - this.viewportY);
	}
	hasSelection() {
		return this.#selection.active;
	}
	getSelection() {
		return this.#core ? this.#selection.text(this.#core) : "";
	}
	getSelectionPosition() {
		return this.#core && this.#selection.active ? this.#selection.range(this.#core) : void 0;
	}
	/** Select `length` cells starting at absolute `line`, `col`, wrapping across lines. */
	select(col, line, length) {
		const cols = this.cols;
		const endOffset = col + length;
		this.#setSelection({
			start: {
				line,
				col
			},
			end: {
				line: line + Math.floor(endOffset / cols),
				col: endOffset % cols
			}
		});
	}
	selectLines(start, end) {
		this.#setSelection({
			start: {
				line: start,
				col: 0
			},
			end: {
				line: end,
				col: this.cols
			}
		});
	}
	selectAll() {
		this.#setSelection({
			start: {
				line: 0,
				col: 0
			},
			end: {
				line: this.historySize + this.rows - 1,
				col: this.cols
			}
		});
	}
	clearSelection() {
		if (!this.#selection.active) return;
		this.#selection.clear();
		this.#renderer?.invalidate();
		this.#scheduleRender();
		this.#onSelectionChange.fire();
	}
	#whenReady(operation) {
		if (this.#disposed) return;
		if (this.#core) operation();
		else this.#pending.push(operation);
	}
	#setSelection(range) {
		this.#selection.set(range);
		this.#renderer?.invalidate();
		this.#scheduleRender();
		this.#onSelectionChange.fire();
	}
	#applyTheme() {
		this.#theme = resolveTheme(this.#options.theme);
		this.#core?.setQueryColors(queryColors(this.#theme));
		if (this.#root) this.#root.style.background = this.#options.allowTransparency ? "transparent" : this.#theme.background.css;
		if (this.#thumb) this.#thumb.style.background = this.#theme.scrollbarThumb.css;
		this.#renderer?.setTheme(this.#theme);
	}
	#attach() {
		const root = this.#root;
		const core = this.#core;
		const screen = document.createElement("div");
		screen.style.position = "absolute";
		root.appendChild(screen);
		this.#screen = screen;
		this.#images = new ImageLayer(() => {
			if (this.#metrics) this.#images?.repaint(this.#metrics);
		});
		this.#renderer = new CanvasRenderer(core);
		screen.append(this.#images.under, this.#renderer.canvas, this.#images.over);
		const textarea = document.createElement("textarea");
		textarea.setAttribute("aria-label", "Terminal input");
		textarea.setAttribute("autocorrect", "off");
		textarea.setAttribute("autocapitalize", "off");
		textarea.setAttribute("spellcheck", "false");
		textarea.tabIndex = 0;
		Object.assign(textarea.style, {
			position: "absolute",
			opacity: "0",
			width: "1px",
			height: "1px",
			padding: "0",
			border: "0",
			margin: "0",
			resize: "none",
			overflow: "hidden",
			whiteSpace: "nowrap",
			zIndex: "-5"
		});
		root.appendChild(textarea);
		this.#textarea = textarea;
		const scrollbar = document.createElement("div");
		Object.assign(scrollbar.style, {
			position: "absolute",
			top: "0",
			right: "0",
			bottom: "0",
			width: "10px",
			opacity: "0",
			transition: "opacity 150ms",
			cursor: "default"
		});
		const thumb = document.createElement("div");
		Object.assign(thumb.style, {
			position: "absolute",
			right: "2px",
			width: "6px",
			borderRadius: "3px"
		});
		scrollbar.appendChild(thumb);
		root.appendChild(scrollbar);
		this.#scrollbar = scrollbar;
		this.#thumb = thumb;
		const live = document.createElement("div");
		live.setAttribute("aria-live", "polite");
		live.setAttribute("role", "log");
		Object.assign(live.style, {
			position: "absolute",
			width: "1px",
			height: "1px",
			overflow: "hidden",
			clipPath: "inset(50%)"
		});
		root.appendChild(live);
		this.#liveRegion = live;
		this.#applyTheme();
		this.#remeasure();
		this.#bindInput(root, screen, textarea);
		this.#bindScrollbar(scrollbar, thumb);
		if (typeof ResizeObserver !== "undefined") {
			const observer = new ResizeObserver(() => {
				if (this.#options.autoFit) this.fit();
			});
			observer.observe(root);
			this.#disposables.push(() => observer.disconnect());
		}
		if (typeof window !== "undefined" && typeof matchMedia === "function") {
			let query;
			const watch = () => {
				query?.removeEventListener("change", onChange);
				query = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
				query.addEventListener("change", onChange);
			};
			const onChange = () => {
				this.#remeasure();
				watch();
			};
			watch();
			this.#disposables.push(() => query?.removeEventListener("change", onChange));
		}
		if (typeof document !== "undefined" && document.fonts) {
			const onFonts = () => this.#remeasure();
			document.fonts.addEventListener("loadingdone", onFonts);
			this.#disposables.push(() => document.fonts.removeEventListener("loadingdone", onFonts));
		}
		if (this.#options.autoFit) this.fit();
		this.#restartBlink();
		this.#scheduleRender();
	}
	#remeasure() {
		if (!this.#renderer || !this.#core) return;
		const dpr = this.#options.devicePixelRatio ?? (typeof window === "undefined" ? 1 : window.devicePixelRatio || 1);
		this.#metrics = measureCell(this.#options, dpr);
		this.#core.setCellPixels(this.#metrics.width, this.#metrics.height);
		this.#configureRenderer();
		this.#renderer.setMetrics(this.#metrics);
		this.#layout();
		if (this.#options.autoFit) this.fit();
		this.#scheduleRender();
	}
	#configureRenderer() {
		const options = this.#options;
		this.#renderer?.setOptions({
			cursorStyle: options.cursorStyle,
			cursorInactiveStyle: options.cursorInactiveStyle,
			cursorWidth: options.cursorWidth,
			drawBoldTextInBrightColors: options.drawBoldTextInBrightColors,
			minimumContrastRatio: options.minimumContrastRatio,
			customGlyphs: options.customGlyphs,
			transparentBackground: options.allowTransparency || (this.#images?.hasUnderImages ?? false)
		});
		if (this.#root) this.#root.style.background = options.allowTransparency ? "transparent" : this.#theme.background.css;
	}
	#layout() {
		const metrics = this.#metrics;
		const screen = this.#screen;
		if (!metrics || !screen || !this.#core) return;
		const padding = this.#options.padding;
		screen.style.left = `${padding}px`;
		screen.style.top = `${padding}px`;
		const width = this.#core.cols * metrics.deviceWidth;
		const height = this.#core.rows * metrics.deviceHeight;
		this.#images?.setSize(width, height, width / metrics.dpr, height / metrics.dpr);
		screen.style.width = `${width / metrics.dpr}px`;
		screen.style.height = `${height / metrics.dpr}px`;
	}
	#afterOutput() {
		const core = this.#core;
		const replies = core.takeReplies();
		if (replies.length > 0) this.#emit(replies, false);
		for (const event of core.takeEvents()) this.#handleEvent(event);
		clearTimeout(this.#syncTimer);
		const deadline = core.syncDeadline();
		if (deadline >= 0) this.#syncTimer = setTimeout(() => {
			if (this.#core?.flushSync()) this.#afterOutput();
		}, deadline + 1);
		clearTimeout(this.#compactTimer);
		this.#compactTimer = setTimeout(() => this.#core?.compactHistory(), COMPACT_DELAY);
		this.#scheduleRender();
	}
	#afterScroll() {
		this.#renderer?.invalidate();
		this.#scheduleRender();
	}
	#handleEvent(event) {
		switch (event.type) {
			case "title":
				this.#title = event.title;
				this.#onTitleChange.fire(event.title);
				break;
			case "resetTitle":
				this.#title = "";
				this.#onTitleChange.fire("");
				break;
			case "bell":
				this.#onBell.fire();
				if (this.#options.bellStyle === "visual") this.#flash();
				break;
			case "cwd":
				this.#onCwdChange.fire(event.cwd);
				break;
			case "progress":
				this.#onProgress.fire({
					state: event.state,
					value: event.value
				});
				break;
			case "shellIntegration":
				this.#onShellIntegration.fire(event.payload);
				break;
			case "clipboard": {
				if (event.data === "?") break;
				const text = decodeBase64(event.data);
				if (text === void 0) break;
				this.#onClipboard.fire({
					selection: event.selection,
					text
				});
				if (this.#options.allowClipboardWrite) navigator.clipboard?.writeText(text).catch(() => {});
				break;
			}
		}
	}
	#emit(bytes, user) {
		if (bytes.length === 0) return;
		if (user) {
			if (this.#options.disableStdin) return;
			if (this.#options.scrollOnUserInput && this.#core?.scrollToBottom()) this.#afterScroll();
		}
		this.#onBytes.fire(bytes);
		let text;
		try {
			text = utf8.decode(bytes);
		} catch {
			this.#onBinary.fire(String.fromCharCode(...bytes));
			return;
		}
		this.#onData.fire(text);
	}
	#scheduleRender() {
		if (this.#frame || !this.#renderer || this.#disposed) return;
		const raf = typeof requestAnimationFrame === "function" ? requestAnimationFrame : (fn) => setTimeout(() => fn(0), 16);
		this.#frame = raf(() => {
			this.#frame = 0;
			this.#render();
		});
	}
	#render() {
		const core = this.#core;
		const renderer = this.#renderer;
		const metrics = this.#metrics;
		if (!core || !renderer || !metrics) return;
		const images = this.#images;
		const hadUnder = images.hasUnderImages;
		images.update(core, metrics, this.#options.images);
		if (images.hasUnderImages !== hadUnder) this.#configureRenderer();
		clearTimeout(this.#imageTimer);
		const imageDeadline = images.deadline(core);
		if (imageDeadline >= 0) this.#imageTimer = setTimeout(() => this.#scheduleRender(), Math.max(8, imageDeadline));
		const cursor = core.cursor();
		const blinking = cursor.blinking || this.#options.cursorBlink;
		const range = this.#selection.active ? this.#selection.range(core) : void 0;
		const hover = this.#hoverLink;
		const link = hover ? {
			row: hover.row,
			start: hover.start,
			end: hover.end
		} : void 0;
		if (renderer.render({
			cursor: {
				...cursor,
				blinking
			},
			focused: this.#focused,
			blinkOn: this.#blinkOn
		}, range ? (row, col) => this.#selection.contains(core, range, row, col) : void 0, link) > 0) this.#onRender.fire({
			start: 0,
			end: core.rows - 1
		});
		const last = this.#lastCursor;
		if (!last || last.row !== cursor.row || last.col !== cursor.col) {
			this.#lastCursor = cursor;
			this.#positionTextarea(cursor);
			if (last) this.#onCursorMove.fire();
		}
		const viewport = core.historySize - core.displayOffset;
		if (viewport !== this.#lastViewport) {
			this.#lastViewport = viewport;
			this.#onScroll.fire(viewport);
		}
		this.#updateScrollbar();
	}
	#positionTextarea(cursor) {
		const metrics = this.#metrics;
		const textarea = this.#textarea;
		if (!metrics || !textarea) return;
		const padding = this.#options.padding;
		textarea.style.left = `${padding + cursor.col * metrics.width}px`;
		textarea.style.top = `${padding + cursor.row * metrics.height}px`;
		textarea.style.height = `${metrics.height}px`;
		textarea.style.lineHeight = `${metrics.height}px`;
		textarea.style.fontSize = `${this.#options.fontSize}px`;
	}
	#restartBlink() {
		clearInterval(this.#blinkTimer);
		this.#blinkOn = true;
		this.#blinkTimer = setInterval(() => {
			const core = this.#core;
			if (!core || !this.#focused) return;
			if (!(this.#options.cursorBlink || core.cursor().blinking)) return;
			this.#blinkOn = !this.#blinkOn;
			this.#renderer?.invalidateRow(core.cursor().row + core.displayOffset);
			this.#scheduleRender();
		}, BLINK_INTERVAL);
	}
	#updateScrollbar() {
		const scrollbar = this.#scrollbar;
		const thumb = this.#thumb;
		const core = this.#core;
		if (!scrollbar || !thumb || !core) return;
		const history = core.historySize;
		if (!this.#options.scrollbar || history === 0 || core.modes().alternateScreen) {
			scrollbar.style.display = "none";
			return;
		}
		scrollbar.style.display = "block";
		const height = scrollbar.clientHeight;
		const total = history + core.rows;
		const thumbHeight = Math.max(20, core.rows / total * height);
		const top = (history - core.displayOffset) / history * (height - thumbHeight);
		thumb.style.height = `${thumbHeight}px`;
		thumb.style.top = `${top}px`;
	}
	#bindScrollbar(scrollbar, thumb) {
		const root = this.#root;
		const show = () => {
			scrollbar.style.opacity = "1";
		};
		const hide = () => {
			scrollbar.style.opacity = "0";
		};
		this.#listen(root, "mouseenter", show);
		this.#listen(root, "mouseleave", hide);
		this.#listen(thumb, "pointerdown", (event) => {
			event.preventDefault();
			event.stopPropagation();
			const core = this.#core;
			if (!core) return;
			thumb.setPointerCapture(event.pointerId);
			const startY = event.clientY;
			const startOffset = core.displayOffset;
			const move = (moveEvent) => {
				const history = core.historySize;
				const track = scrollbar.clientHeight - thumb.clientHeight;
				if (track <= 0) return;
				const target = Math.round(startOffset - (moveEvent.clientY - startY) / track * history);
				const clamped = Math.max(0, Math.min(history, target));
				if (core.scrollDisplay(clamped - core.displayOffset)) this.#afterScroll();
			};
			const up = () => {
				thumb.removeEventListener("pointermove", move);
				thumb.removeEventListener("pointerup", up);
			};
			thumb.addEventListener("pointermove", move);
			thumb.addEventListener("pointerup", up);
		});
	}
	#listen(target, type, listener, options) {
		target.addEventListener(type, listener, options);
		this.#disposables.push(() => target.removeEventListener(type, listener, options));
	}
	#bindInput(root, screen, textarea) {
		this.#listen(textarea, "focus", () => {
			this.#focused = true;
			this.#blinkOn = true;
			const focusBytes = this.#core?.encodeFocus(true);
			if (focusBytes) this.#emit(focusBytes, false);
			this.#renderer?.invalidate();
			this.#scheduleRender();
			this.#onFocus.fire();
		});
		this.#listen(textarea, "blur", () => {
			this.#focused = false;
			const focusBytes = this.#core?.encodeFocus(false);
			if (focusBytes) this.#emit(focusBytes, false);
			this.#renderer?.invalidate();
			this.#scheduleRender();
			this.#onBlur.fire();
		});
		this.#listen(textarea, "keydown", (event) => this.#keyDown(event));
		this.#listen(textarea, "keyup", (event) => this.#keyUp(event));
		this.#listen(textarea, "compositionstart", () => {
			this.#composing = true;
		});
		this.#listen(textarea, "compositionend", (event) => {
			this.#composing = false;
			if (event.data) this.input(event.data);
			textarea.value = "";
		});
		this.#listen(textarea, "input", (event) => {
			const input = event;
			if (this.#composing || input.isComposing) return;
			if ((input.inputType === "insertText" || input.inputType === "insertReplacementText") && input.data) this.input(input.data);
			textarea.value = "";
		});
		this.#listen(textarea, "paste", (event) => {
			event.preventDefault();
			const text = event.clipboardData?.getData("text/plain");
			if (text) this.paste(text);
		});
		this.#listen(textarea, "copy", (event) => {
			if (!this.hasSelection()) return;
			event.preventDefault();
			event.clipboardData?.setData("text/plain", this.getSelection());
		});
		this.#listen(root, "mousedown", (event) => this.#mouseDown(event, screen));
		this.#listen(root, "mousemove", (event) => this.#hover(event, screen));
		this.#listen(root, "mouseleave", (event) => this.#setHoverLink(void 0, event));
		this.#listen(root, "wheel", (event) => this.#wheel(event, screen), { passive: false });
		this.#listen(root, "contextmenu", (event) => {
			if (this.#options.rightClickSelectsWord && this.#core) {
				const [col, row] = this.#cellAt(event, screen);
				this.#selection.start(this.#core, this.#bufferPoint(col, row), "word");
				this.#afterSelectionChange();
			}
		});
	}
	#keyDown(event) {
		const core = this.#core;
		if (!core || this.#options.disableStdin) return;
		if (this.#customKeyHandler && this.#customKeyHandler(event) === false) return;
		if (event.isComposing || event.keyCode === 229 || this.#composing) return;
		const mac = isMac();
		const lower = event.key.toLowerCase();
		const clipboardShortcut = mac ? event.metaKey && !event.ctrlKey && !event.altKey : event.ctrlKey && event.shiftKey && !event.altKey;
		if (clipboardShortcut && (lower === "c" || lower === "v" || lower === "x")) return;
		if (clipboardShortcut && lower === "a") {
			event.preventDefault();
			this.selectAll();
			return;
		}
		if (mac && event.metaKey) return;
		const translated = translateKey(event, this.#options.macOptionIsMeta);
		if (!translated) return;
		let modifiers = translated.modifiers;
		if (mac && event.altKey && !translated.optionAsAlt && translated.text) modifiers &= -3;
		const bytes = core.encodeKey(translated.key, translated.text, modifiers, event.repeat ? "repeat" : "press", translated.optionAsAlt);
		if (!bytes) return;
		event.preventDefault();
		event.stopPropagation();
		this.#onKey.fire({
			key: new TextDecoder().decode(bytes),
			domEvent: event
		});
		if (this.#selection.active) this.clearSelection();
		this.#emit(bytes, true);
	}
	#keyUp(event) {
		const core = this.#core;
		if (!core || this.#options.disableStdin) return;
		if ((core.modes().kittyKeyboardFlags & 2) === 0) return;
		if (this.#customKeyHandler && this.#customKeyHandler(event) === false) return;
		const translated = translateKey(event, this.#options.macOptionIsMeta);
		if (!translated) return;
		const bytes = core.encodeKey(translated.key, translated.text, translated.modifiers, "release", translated.optionAsAlt);
		if (bytes) this.#emit(bytes, true);
	}
	#cellAt(event, screen) {
		const metrics = this.#metrics;
		const core = this.#core;
		if (!metrics || !core) return [0, 0];
		const rect = screen.getBoundingClientRect();
		const col = Math.floor((event.clientX - rect.left) / metrics.width);
		const row = Math.floor((event.clientY - rect.top) / metrics.height);
		return [Math.max(0, Math.min(core.cols - 1, col)), Math.max(0, Math.min(core.rows - 1, row))];
	}
	#bufferPoint(col, row) {
		const core = this.#core;
		return {
			line: core.historySize - core.displayOffset + row,
			col
		};
	}
	#mouseTracking(event) {
		const core = this.#core;
		if (!core || !core.modes().mouseTracking || event.shiftKey) return false;
		return !(this.#options.macOptionClickForcesSelection && event.altKey && isMac());
	}
	#mouseDown(event, screen) {
		const core = this.#core;
		if (!core) return;
		if (event.target === this.#thumb) return;
		event.preventDefault();
		this.focus();
		const [col, row] = this.#cellAt(event, screen);
		const button = event.button === 1 ? "middle" : event.button === 2 ? "right" : "left";
		if (this.#mouseTracking(event)) {
			this.#mouseButton = button;
			this.#sendMouse("press", button, col, row, event);
			this.#trackDrag(screen, (move) => {
				const [c, r] = this.#cellAt(move, screen);
				this.#sendMouse("drag", button, c, r, move);
			}, (up) => {
				const [c, r] = this.#cellAt(up, screen);
				this.#sendMouse("release", button, c, r, up);
				this.#mouseButton = void 0;
			});
			return;
		}
		if (button !== "left") return;
		if (this.#hoverLink && event.detail === 1 && this.#options.linkHandler) {
			const link = this.#hoverLink;
			let moved = false;
			this.#trackDrag(screen, () => {
				moved = true;
			}, (up) => {
				if (!moved) this.#activateLink(link, up);
			});
			return;
		}
		if (event.altKey && this.#options.altClickMovesCursor && !core.modes().alternateScreen) {
			const cursor = core.cursor();
			if (row - core.displayOffset === cursor.row && col !== cursor.col) {
				const key = col > cursor.col ? "right" : "left";
				const bytes = core.encodeKey(key, void 0, 0);
				if (bytes) for (let i = 0; i < Math.abs(col - cursor.col); i++) this.#emit(bytes, true);
				return;
			}
		}
		const point = this.#bufferPoint(col, row);
		if (event.shiftKey && this.#selection.active) this.#selection.extend(core, point);
		else this.#selection.start(core, point, event.detail >= 3 ? "line" : event.detail === 2 ? "word" : "char");
		this.#selecting = true;
		this.#afterSelectionChange(false);
		this.#trackDrag(screen, (move) => {
			const metrics = this.#metrics;
			const rect = screen.getBoundingClientRect();
			if (metrics && move.clientY < rect.top) this.scrollLines(-1);
			else if (metrics && move.clientY > rect.bottom) this.scrollLines(1);
			const [c, r] = this.#cellAt(move, screen);
			const inside = move.clientX - rect.left;
			const edge = metrics && inside > (c + .5) * metrics.width ? c + 1 : c;
			this.#selection.extend(core, this.#bufferPoint(edge, r));
			this.#afterSelectionChange(false);
		}, () => {
			this.#selecting = false;
			this.#afterSelectionChange();
		});
	}
	#afterSelectionChange(final = true) {
		this.#renderer?.invalidate();
		this.#scheduleRender();
		if (!final) return;
		this.#onSelectionChange.fire();
		if (this.#options.copyOnSelect && this.hasSelection()) navigator.clipboard?.writeText(this.getSelection()).catch(() => {});
	}
	#trackDrag(_screen, move, up) {
		const onMove = (event) => move(event);
		const onUp = (event) => {
			window.removeEventListener("mousemove", onMove);
			window.removeEventListener("mouseup", onUp);
			up(event);
		};
		window.addEventListener("mousemove", onMove);
		window.addEventListener("mouseup", onUp);
	}
	#sendMouse(kind, button, col, row, event) {
		const bytes = this.#core?.encodeMouse(kind, button, col, row, modifiersOf(event));
		if (bytes) this.#emit(bytes, true);
	}
	#hover(event, screen) {
		const core = this.#core;
		if (!core || this.#selecting || event.buttons !== 0) return;
		const [col, row] = this.#cellAt(event, screen);
		const last = this.#lastMouseCell;
		if (last && last[0] === col && last[1] === row) return;
		this.#lastMouseCell = [col, row];
		if (this.#mouseTracking(event) && this.#mouseButton === void 0) this.#sendMouse("move", "left", col, row, event);
		const handler = this.#options.linkHandler;
		const link = handler ? linkAt(core, row, col, this.#options.linkDetection) : void 0;
		this.#setHoverLink(link && isSafeLink(link.uri, handler?.allowNonHttpProtocols ?? false) ? link : void 0, event);
	}
	#setHoverLink(link, event) {
		const previous = this.#hoverLink;
		if (previous?.uri === link?.uri && previous?.row === link?.row && previous?.start === link?.start) return;
		if (event.type === "mouseleave") this.#lastMouseCell = void 0;
		this.#hoverLink = link;
		if (this.#root) this.#root.style.cursor = link ? "pointer" : "text";
		if (previous) {
			this.#options.linkHandler?.leave?.(event, previous.uri);
			this.#renderer?.invalidateRow(previous.row);
		}
		if (link) {
			this.#options.linkHandler?.hover?.(event, link.uri);
			this.#renderer?.invalidateRow(link.row);
		}
		this.#scheduleRender();
	}
	#activateLink(link, event) {
		const handler = this.#options.linkHandler;
		if (handler && isSafeLink(link.uri, handler.allowNonHttpProtocols ?? false)) handler.activate(event, link.uri);
	}
	#wheel(event, screen) {
		const core = this.#core;
		const metrics = this.#metrics;
		if (!core || !metrics || event.deltaY === 0) return;
		event.preventDefault();
		const fast = this.#options.fastScrollModifier;
		const sensitivity = fast === "alt" && event.altKey || fast === "ctrl" && event.ctrlKey || fast === "shift" && event.shiftKey ? this.#options.fastScrollSensitivity : this.#options.scrollSensitivity;
		const pixels = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * metrics.height : event.deltaMode === WheelEvent.DOM_DELTA_PAGE ? event.deltaY * metrics.height * core.rows : event.deltaY;
		this.#wheelRemainder += pixels / metrics.height * sensitivity;
		const lines = Math.trunc(this.#wheelRemainder);
		if (lines === 0) return;
		this.#wheelRemainder -= lines;
		if (this.#mouseTracking(event)) {
			const [col, row] = this.#cellAt(event, screen);
			const kind = lines < 0 ? "wheelUp" : "wheelDown";
			for (let i = 0; i < Math.min(Math.abs(lines), 10); i++) this.#sendMouse(kind, "left", col, row, event);
			return;
		}
		if (core.modes().alternateScreen && this.#options.alternateScroll) {
			const bytes = core.encodeKey(lines < 0 ? "up" : "down", void 0, 0);
			if (bytes) for (let i = 0; i < Math.abs(lines); i++) this.#emit(bytes, true);
			return;
		}
		this.scrollLines(lines);
	}
	#flash() {
		const root = this.#root;
		if (!root) return;
		const overlay = document.createElement("div");
		Object.assign(overlay.style, {
			position: "absolute",
			inset: "0",
			background: this.#theme.foreground.css,
			opacity: "0.15",
			pointerEvents: "none",
			transition: "opacity 150ms"
		});
		root.appendChild(overlay);
		requestAnimationFrame(() => {
			overlay.style.opacity = "0";
			setTimeout(() => overlay.remove(), 200);
		});
	}
	#announce(data) {
		const live = this.#liveRegion;
		if (!live) return;
		const text = (typeof data === "string" ? data : new TextDecoder().decode(data)).replace(/\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-_]/g, "").replace(/\r/g, "");
		if (!text.trim()) return;
		const line = document.createElement("div");
		line.textContent = text;
		live.appendChild(line);
		while (live.childElementCount > 20) live.firstElementChild?.remove();
	}
};
function convertEol(data) {
	if (typeof data === "string") return data.replace(/\r?\n/g, "\r\n");
	const out = [];
	for (let i = 0; i < data.length; i++) {
		const byte = data[i];
		if (byte === 10 && data[i - 1] !== 13) out.push(13);
		out.push(byte);
	}
	return new Uint8Array(out);
}
function decodeBase64(value) {
	try {
		const binary = atob(value);
		return new TextDecoder().decode(Uint8Array.from(binary, (ch) => ch.charCodeAt(0)));
	} catch {
		return;
	}
}
//#endregion
export { DEFAULT_FONT_FAMILY, DEFAULT_OPTIONS, DEFAULT_THEME, Emitter, Terminal, builtinTheme, builtinThemeIds, init, initSync, isInitialized, parseColor, resolveTheme, toDisposable, translateKey };
