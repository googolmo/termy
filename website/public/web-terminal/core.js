//#region src/wasm/termy_wasm.js
/**
* One headless terminal: parser, grid, scrollback and protocol state.
*/
var TermyEngine = class {
	__destroy_into_raw() {
		const ptr = this.__wbg_ptr;
		this.__wbg_ptr = 0;
		TermyEngineFinalization.unregister(this);
		return ptr;
	}
	free() {
		const ptr = this.__destroy_into_raw();
		wasm.__wbg_termyengine_free(ptr, 0);
	}
	clear_scrollback() {
		wasm.termyengine_clear_scrollback(this.__wbg_ptr);
	}
	/**
	* @returns {number}
	*/
	cols() {
		return wasm.termyengine_cols(this.__wbg_ptr) >>> 0;
	}
	/**
	* Pack cold scrollback; call after output goes quiet.
	*/
	compact_history() {
		wasm.termyengine_compact_history(this.__wbg_ptr);
	}
	/**
	* `[row, col, visible, shape, blinking]`; shape 0 block, 1 bar, 2 underline.
	* @returns {Uint32Array}
	*/
	cursor() {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_cursor(retptr, this.__wbg_ptr);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* @returns {number}
	*/
	display_offset() {
		return wasm.termyengine_display_offset(this.__wbg_ptr) >>> 0;
	}
	/**
	* OSC 10/11/12 overrides as raw colors: `[foreground, background, cursor]`.
	* @returns {Uint32Array}
	*/
	dynamic_color_overrides() {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_dynamic_color_overrides(retptr, this.__wbg_ptr);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* @param {boolean} focused
	* @returns {Uint8Array | undefined}
	*/
	encode_focus(focused) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_encode_focus(retptr, this.__wbg_ptr, focused);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v1;
			if (r0 !== 0) {
				v1 = getArrayU8FromWasm0(r0, r1).slice();
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Encode a key event. `modifiers`: 1 ctrl, 2 alt, 4 shift, 8 meta.
	* `kind`: 0 press, 1 repeat, 2 release. `key` uses Termy key names
	* (`enter`, `up`, `f1`, `a`, ...); `text` is the produced character.
	* @param {string} key
	* @param {string | null | undefined} text
	* @param {number} modifiers
	* @param {number} kind
	* @param {boolean} option_as_alt
	* @returns {Uint8Array | undefined}
	*/
	encode_key(key, text, modifiers, kind, option_as_alt) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			const ptr0 = passStringToWasm0(key, wasm.__wbindgen_export2, wasm.__wbindgen_export4);
			const len0 = WASM_VECTOR_LEN;
			var ptr1 = isLikeNone(text) ? 0 : passStringToWasm0(text, wasm.__wbindgen_export2, wasm.__wbindgen_export4);
			var len1 = WASM_VECTOR_LEN;
			wasm.termyengine_encode_key(retptr, this.__wbg_ptr, ptr0, len0, ptr1, len1, modifiers, kind, option_as_alt);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v3;
			if (r0 !== 0) {
				v3 = getArrayU8FromWasm0(r0, r1).slice();
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v3;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Encode a mouse report, or nothing when the application has not enabled
	* the matching tracking mode. `kind`: 0 press, 1 release, 2 drag, 3 move,
	* 4-7 wheel up/down/left/right. `button`: 0 left, 1 middle, 2 right.
	* @param {number} kind
	* @param {number} button
	* @param {number} col
	* @param {number} row
	* @param {number} modifiers
	* @returns {Uint8Array | undefined}
	*/
	encode_mouse(kind, button, col, row, modifiers) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_encode_mouse(retptr, this.__wbg_ptr, kind, button, col, row, modifiers);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v1;
			if (r0 !== 0) {
				v1 = getArrayU8FromWasm0(r0, r1).slice();
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Normalize newlines and apply bracketed paste when the application asked for it.
	* @param {string} text
	* @returns {Uint8Array}
	*/
	encode_paste(text) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export2, wasm.__wbindgen_export4);
			const len0 = WASM_VECTOR_LEN;
			wasm.termyengine_encode_paste(retptr, this.__wbg_ptr, ptr0, len0);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v2 = getArrayU8FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 1, 1);
			return v2;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Feed child/host output into the parser.
	* @param {Uint8Array} bytes
	*/
	feed(bytes) {
		const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_export2);
		const len0 = WASM_VECTOR_LEN;
		wasm.termyengine_feed(this.__wbg_ptr, ptr0, len0);
	}
	/**
	* @param {string} text
	*/
	feed_str(text) {
		const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export2, wasm.__wbindgen_export4);
		const len0 = WASM_VECTOR_LEN;
		wasm.termyengine_feed_str(this.__wbg_ptr, ptr0, len0);
	}
	/**
	* @returns {boolean}
	*/
	flush_sync() {
		return wasm.termyengine_flush_sync(this.__wbg_ptr) !== 0;
	}
	/**
	* Increments whenever parsed output or a viewport change may alter a read.
	* @returns {number}
	*/
	generation() {
		return wasm.termyengine_generation(this.__wbg_ptr);
	}
	/**
	* Milliseconds until the next animation frame of a visible image, or -1.
	* @returns {number}
	*/
	graphics_deadline_ms() {
		return wasm.termyengine_graphics_deadline_ms(this.__wbg_ptr);
	}
	/**
	* Pixels of placement `index` from the last [`Self::read_graphics`]:
	* RGBA, or a PNG stream when [`Self::graphics_image_is_png`] is true.
	* @param {number} index
	* @returns {Uint8Array | undefined}
	*/
	graphics_image(index) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_graphics_image(retptr, this.__wbg_ptr, index);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v1;
			if (r0 !== 0) {
				v1 = getArrayU8FromWasm0(r0, r1).slice();
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* @param {number} index
	* @returns {boolean}
	*/
	graphics_image_is_png(index) {
		return wasm.termyengine_graphics_image_is_png(this.__wbg_ptr, index) !== 0;
	}
	/**
	* Changes when kitty graphics placements or animation frames change.
	* @returns {number}
	*/
	graphics_revision() {
		return wasm.termyengine_graphics_revision(this.__wbg_ptr);
	}
	/**
	* @returns {number}
	*/
	history_size() {
		return wasm.termyengine_history_size(this.__wbg_ptr) >>> 0;
	}
	/**
	* Active kitty keyboard protocol flags (CSI > u), 0 when legacy encoding.
	* @returns {number}
	*/
	keyboard_flags() {
		return wasm.termyengine_keyboard_flags(this.__wbg_ptr) >>> 0;
	}
	/**
	* Text of a buffer line. Negative lines are scrollback, -1 the newest.
	* @param {number} line
	* @param {boolean} trim_end
	* @returns {string | undefined}
	*/
	line_text(line, trim_end) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_line_text(retptr, this.__wbg_ptr, line, trim_end);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v1;
			if (r0 !== 0) {
				v1 = getStringFromWasm0(r0, r1);
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Text of columns `[start, end)` of a buffer line; wide characters are
	* included when their leading cell is in range.
	* @param {number} line
	* @param {number} start
	* @param {number} end
	* @param {boolean} trim_end
	* @returns {string | undefined}
	*/
	line_text_range(line, start, end, trim_end) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_line_text_range(retptr, this.__wbg_ptr, line, start, end, trim_end);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v1;
			if (r0 !== 0) {
				v1 = getStringFromWasm0(r0, r1);
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* @param {number} line
	* @returns {boolean}
	*/
	line_wrapped(line) {
		return wasm.termyengine_line_wrapped(this.__wbg_ptr, line) !== 0;
	}
	/**
	* @returns {number}
	*/
	mode_bits() {
		return wasm.termyengine_mode_bits(this.__wbg_ptr) >>> 0;
	}
	/**
	* @param {number} cols
	* @param {number} rows
	* @param {number} scrollback
	*/
	constructor(cols, rows, scrollback) {
		const ret = wasm.termyengine_new(cols, rows, scrollback);
		this.__wbg_ptr = ret;
		TermyEngineFinalization.register(this, this.__wbg_ptr, this);
		return this;
	}
	/**
	* OSC 4 overrides: 256 raw colors where 0 means "use the theme".
	* @returns {Uint32Array}
	*/
	palette_overrides() {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_palette_overrides(retptr, this.__wbg_ptr);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* @returns {number}
	*/
	palette_revision() {
		return wasm.termyengine_palette_revision(this.__wbg_ptr);
	}
	/**
	* Visible kitty graphics placements laid out for a cell size in CSS
	* pixels; see [`PLACEMENT_STRIDE`].
	* @param {number} cell_width
	* @param {number} cell_height
	* @returns {Float64Array}
	*/
	read_graphics(cell_width, cell_height) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_read_graphics(retptr, this.__wbg_ptr, cell_width, cell_height);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayF64FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 8, 8);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Flat cells for one buffer line; strings via [`Self::read_strings`].
	* @param {number} line
	* @returns {Uint32Array}
	*/
	read_line(line) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_read_line(retptr, this.__wbg_ptr, line);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Flat cells for viewport rows `[start, end)`; see [`CELL_STRIDE`].
	* @param {number} start
	* @param {number} end
	* @returns {Uint32Array}
	*/
	read_rows(start, end) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_read_rows(retptr, this.__wbg_ptr, start, end);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Strings referenced by the last [`Self::read_rows`] call.
	* @param {number} index
	* @returns {string | undefined}
	*/
	read_string(index) {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_read_string(retptr, this.__wbg_ptr, index);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			let v1;
			if (r0 !== 0) {
				v1 = getStringFromWasm0(r0, r1);
				wasm.__wbindgen_export3(r0, r1 * 1, 1);
			}
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* The whole string table of the last [`Self::read_rows`] call.
	* @returns {string[]}
	*/
	read_strings() {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_read_strings(retptr, this.__wbg_ptr);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayJsValueFromWasm0(r0, r1);
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* @param {number} cols
	* @param {number} rows
	*/
	resize(cols, rows) {
		wasm.termyengine_resize(this.__wbg_ptr, cols, rows);
	}
	/**
	* @returns {number}
	*/
	rows() {
		return wasm.termyengine_rows(this.__wbg_ptr) >>> 0;
	}
	/**
	* Positive deltas scroll back into history.
	* @param {number} delta
	* @returns {boolean}
	*/
	scroll_display(delta) {
		return wasm.termyengine_scroll_display(this.__wbg_ptr, delta) !== 0;
	}
	/**
	* @returns {boolean}
	*/
	scroll_to_bottom() {
		return wasm.termyengine_scroll_to_bottom(this.__wbg_ptr) !== 0;
	}
	/**
	* Pixel cell size, used for XTWINOPS size reports and image placement.
	* @param {number} width
	* @param {number} height
	*/
	set_cell_pixels(width, height) {
		wasm.termyengine_set_cell_pixels(this.__wbg_ptr, width, height);
	}
	/**
	* Default cursor shape until an application overrides it with DECSCUSR.
	* @param {number} shape
	*/
	set_default_cursor_shape(shape) {
		wasm.termyengine_set_default_cursor_shape(this.__wbg_ptr, shape);
	}
	/**
	* Colors answered to OSC 4/10/11/12 queries: `[fg, bg, cursor, ansi0..15]`
	* as `0xRRGGBB`.
	* @param {Uint32Array} colors
	*/
	set_query_colors(colors) {
		const ptr0 = passArray32ToWasm0(colors, wasm.__wbindgen_export2);
		const len0 = WASM_VECTOR_LEN;
		wasm.termyengine_set_query_colors(this.__wbg_ptr, ptr0, len0);
	}
	/**
	* @param {number} lines
	*/
	set_scrollback(lines) {
		wasm.termyengine_set_scrollback(this.__wbg_ptr, lines);
	}
	/**
	* Milliseconds until a pending synchronized update must be committed, or -1.
	* @returns {number}
	*/
	sync_deadline_ms() {
		return wasm.termyengine_sync_deadline_ms(this.__wbg_ptr);
	}
	/**
	* `[full, scrollCount, (top, bottom, lines)*, (row, start, end)*]`.
	* Scroll entries rotate retained rows before the spans are repainted;
	* `lines` is an i32 stored in a u32 slot.
	* @returns {Uint32Array}
	*/
	take_damage() {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_take_damage(retptr, this.__wbg_ptr);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
	/**
	* Drained events as plain objects: `{ type, ... }`.
	* @returns {Array<any>}
	*/
	take_events() {
		return takeObject(wasm.termyengine_take_events(this.__wbg_ptr));
	}
	/**
	* Protocol replies (DA, DSR, color queries, ...) for the host transport.
	* @returns {Uint8Array}
	*/
	take_replies() {
		try {
			const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
			wasm.termyengine_take_replies(retptr, this.__wbg_ptr);
			var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
			var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
			var v1 = getArrayU8FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 1, 1);
			return v1;
		} finally {
			wasm.__wbindgen_add_to_stack_pointer(16);
		}
	}
};
if (Symbol.dispose) TermyEngine.prototype[Symbol.dispose] = TermyEngine.prototype.free;
/**
* @returns {number}
*/
function cellStride() {
	return wasm.cellStride() >>> 0;
}
/**
* Geometry for a special glyph, or undefined when it should be shaped as text.
* `neighbors` are the code points two before, one before, one after and two
* after in the row (0 when absent). See `glyphs.rs` for the layout.
* @param {number} code_point
* @param {Uint32Array} neighbors
* @param {number} cell_width
* @param {number} cell_height
* @param {number} font_size
* @returns {Float32Array | undefined}
*/
function glyphPlan$1(code_point, neighbors, cell_width, cell_height, font_size) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passArray32ToWasm0(neighbors, wasm.__wbindgen_export2);
		const len0 = WASM_VECTOR_LEN;
		wasm.glyphPlan(retptr, code_point, ptr0, len0, cell_width, cell_height, font_size);
		var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
		let v2;
		if (r0 !== 0) {
			v2 = getArrayF32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
		}
		return v2;
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}
/**
* @returns {number}
*/
function placementStride() {
	return wasm.placementStride() >>> 0;
}
/**
* `[fg, bg, cursor, ansi0..15]` as `0xRRGGBB`, or undefined for unknown ids.
* @param {string} id
* @returns {Uint32Array | undefined}
*/
function themeColors(id) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(id, wasm.__wbindgen_export2, wasm.__wbindgen_export4);
		const len0 = WASM_VECTOR_LEN;
		wasm.themeColors(retptr, ptr0, len0);
		var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
		let v2;
		if (r0 !== 0) {
			v2 = getArrayU32FromWasm0(r0, r1).slice();
			wasm.__wbindgen_export3(r0, r1 * 4, 4);
		}
		return v2;
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}
/**
* Built-in theme ids, e.g. `termy`, `tokyo-night`, `dracula`.
* @returns {string[]}
*/
function themeIds() {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.themeIds(retptr);
		var r0 = getDataViewMemory0().getInt32(retptr + 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4, true);
		var v1 = getArrayJsValueFromWasm0(r0, r1);
		wasm.__wbindgen_export3(r0, r1 * 4, 4);
		return v1;
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}
function __wbg_get_imports() {
	return {
		__proto__: null,
		"./termy_wasm_bg.js": {
			__proto__: null,
			__wbg___wbindgen_is_undefined_8c687d0b90d5b524: function(arg0) {
				return getObject(arg0) === void 0;
			},
			__wbg___wbindgen_throw_5d9e815e6fdf150f: function(arg0, arg1) {
				throw new Error(getStringFromWasm0(arg0, arg1));
			},
			__wbg_new_bebc3f4757acf305: function() {
				return addHeapObject(/* @__PURE__ */ new Object());
			},
			__wbg_new_ffa92086ea89f79c: function() {
				return addHeapObject(new Array());
			},
			__wbg_now_e7c6795a7f81e10f: function(arg0) {
				return getObject(arg0).now();
			},
			__wbg_performance_3fcf6e32a7e1ed0a: function(arg0) {
				const ret = getObject(arg0).performance;
				return addHeapObject(ret);
			},
			__wbg_push_bfdf956ba476f65b: function(arg0, arg1) {
				return getObject(arg0).push(getObject(arg1));
			},
			__wbg_set_a377297433dfea63: function() {
				return handleError(function(arg0, arg1, arg2) {
					return Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
				}, arguments);
			},
			__wbg_static_accessor_GLOBAL_8eb4cd83130a11a0: function() {
				const ret = typeof global === "undefined" ? null : global;
				return isLikeNone(ret) ? 0 : addHeapObject(ret);
			},
			__wbg_static_accessor_GLOBAL_THIS_1e7044f654e934db: function() {
				const ret = typeof globalThis === "undefined" ? null : globalThis;
				return isLikeNone(ret) ? 0 : addHeapObject(ret);
			},
			__wbg_static_accessor_SELF_d8b50611246a6d92: function() {
				const ret = typeof self === "undefined" ? null : self;
				return isLikeNone(ret) ? 0 : addHeapObject(ret);
			},
			__wbg_static_accessor_WINDOW_fd0bc376bf0f8b42: function() {
				const ret = typeof window === "undefined" ? null : window;
				return isLikeNone(ret) ? 0 : addHeapObject(ret);
			},
			__wbindgen_generic_0000000000000001: function(arg0) {
				return addHeapObject(arg0);
			},
			__wbindgen_generic_0000000000000002: function(arg0, arg1) {
				return addHeapObject(getStringFromWasm0(arg0, arg1));
			},
			__wbindgen_object_clone_ref: function(arg0) {
				return addHeapObject(getObject(arg0));
			},
			__wbindgen_object_drop_ref: function(arg0) {
				takeObject(arg0);
			}
		}
	};
}
const TermyEngineFinalization = typeof FinalizationRegistry === "undefined" ? {
	register: () => {},
	unregister: () => {}
} : new FinalizationRegistry((ptr) => wasm.__wbg_termyengine_free(ptr, 1));
function addHeapObject(obj) {
	if (heap_next === heap.length) heap.push(heap.length + 1);
	const idx = heap_next;
	heap_next = heap[idx];
	heap[idx] = obj;
	return idx;
}
function dropObject(idx) {
	if (idx < 1028) return;
	heap[idx] = heap_next;
	heap_next = idx;
}
function getArrayF32FromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}
function getArrayF64FromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}
function getArrayJsValueFromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	const mem = getDataViewMemory0();
	const result = [];
	for (let i = ptr; i < ptr + 4 * len; i += 4) result.push(takeObject(mem.getUint32(i, true)));
	return result;
}
function getArrayU32FromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}
function getArrayU8FromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}
let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
	if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || cachedDataViewMemory0.buffer.detached === void 0 && cachedDataViewMemory0.buffer !== wasm.memory.buffer) cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
	return cachedDataViewMemory0;
}
let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
	if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
	return cachedFloat32ArrayMemory0;
}
let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
	if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
	return cachedFloat64ArrayMemory0;
}
function getStringFromWasm0(ptr, len) {
	return decodeText(ptr >>> 0, len);
}
let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
	if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
	return cachedUint32ArrayMemory0;
}
let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
	if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
	return cachedUint8ArrayMemory0;
}
function getObject(idx) {
	return heap[idx];
}
function handleError(f, args) {
	try {
		return f.apply(this, args);
	} catch (e) {
		wasm.__wbindgen_export(addHeapObject(e));
	}
}
let heap = new Array(1024).fill(void 0);
heap.push(void 0, null, true, false);
let heap_next = heap.length;
function isLikeNone(x) {
	return x === void 0 || x === null;
}
function passArray32ToWasm0(arg, malloc) {
	const ptr = malloc(arg.length * 4, 4) >>> 0;
	getUint32ArrayMemory0().set(arg, ptr / 4);
	WASM_VECTOR_LEN = arg.length;
	return ptr;
}
function passArray8ToWasm0(arg, malloc) {
	const ptr = malloc(arg.length * 1, 1) >>> 0;
	getUint8ArrayMemory0().set(arg, ptr / 1);
	WASM_VECTOR_LEN = arg.length;
	return ptr;
}
function passStringToWasm0(arg, malloc, realloc) {
	if (realloc === void 0) {
		const buf = cachedTextEncoder.encode(arg);
		const ptr = malloc(buf.length, 1) >>> 0;
		getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
		WASM_VECTOR_LEN = buf.length;
		return ptr;
	}
	let len = arg.length;
	let ptr = malloc(len, 1) >>> 0;
	const mem = getUint8ArrayMemory0();
	let offset = 0;
	for (; offset < len; offset++) {
		const code = arg.charCodeAt(offset);
		if (code > 127) break;
		mem[ptr + offset] = code;
	}
	if (offset !== len) {
		if (offset !== 0) arg = arg.slice(offset);
		ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
		const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
		const ret = cachedTextEncoder.encodeInto(arg, view);
		offset += ret.written;
		ptr = realloc(ptr, len, offset, 1) >>> 0;
	}
	WASM_VECTOR_LEN = offset;
	return ptr;
}
function takeObject(idx) {
	const ret = getObject(idx);
	dropObject(idx);
	return ret;
}
let cachedTextDecoder = new TextDecoder("utf-8", {
	ignoreBOM: true,
	fatal: true
});
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
	numBytesDecoded += len;
	if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
		cachedTextDecoder = new TextDecoder("utf-8", {
			ignoreBOM: true,
			fatal: true
		});
		cachedTextDecoder.decode();
		numBytesDecoded = len;
	}
	return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}
const cachedTextEncoder = new TextEncoder();
if (!("encodeInto" in cachedTextEncoder)) cachedTextEncoder.encodeInto = function(arg, view) {
	const buf = cachedTextEncoder.encode(arg);
	view.set(buf);
	return {
		read: arg.length,
		written: buf.length
	};
};
let WASM_VECTOR_LEN = 0;
let wasm;
function __wbg_finalize_init(instance, module) {
	wasm = instance.exports;
	cachedDataViewMemory0 = null;
	cachedFloat32ArrayMemory0 = null;
	cachedFloat64ArrayMemory0 = null;
	cachedUint32ArrayMemory0 = null;
	cachedUint8ArrayMemory0 = null;
	return wasm;
}
async function __wbg_load(module, imports) {
	if (typeof Response === "function" && module instanceof Response) {
		if (!module.ok) throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
		if (typeof WebAssembly.instantiateStreaming === "function") try {
			return await WebAssembly.instantiateStreaming(module, imports);
		} catch (e) {
			if (expectedResponseType(module.type) && module.headers.get("Content-Type") !== "application/wasm") console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
			else throw e;
		}
		const bytes = await module.arrayBuffer();
		return await WebAssembly.instantiate(bytes, imports);
	} else {
		const instance = await WebAssembly.instantiate(module, imports);
		if (instance instanceof WebAssembly.Instance) return {
			instance,
			module
		};
		else return instance;
	}
	function expectedResponseType(type) {
		switch (type) {
			case "basic":
			case "cors":
			case "default": return true;
		}
		return false;
	}
}
function initSync$1(module) {
	if (wasm !== void 0) return wasm;
	if (module !== void 0) {
		if (Object.getPrototypeOf(module) === Object.prototype) ({module} = module);
		else console.warn("using deprecated parameters for `initSync()`; pass a single object instead");
	}
	const imports = __wbg_get_imports();
	if (!(module instanceof WebAssembly.Module)) module = new WebAssembly.Module(module);
	return __wbg_finalize_init(new WebAssembly.Instance(module, imports), module);
}
async function __wbg_init(module_or_path) {
	if (wasm !== void 0) return wasm;
	if (module_or_path !== void 0) {
		if (Object.getPrototypeOf(module_or_path) === Object.prototype) ({module_or_path} = module_or_path);
		else console.warn("using deprecated parameters for the initialization function; pass a single object instead");
	}
	const imports = __wbg_get_imports();
	if (typeof module_or_path === "string" || typeof Request === "function" && module_or_path instanceof Request || typeof URL === "function" && module_or_path instanceof URL) module_or_path = fetch(module_or_path);
	const { instance, module } = await __wbg_load(await module_or_path, imports);
	return __wbg_finalize_init(instance, module);
}
//#endregion
//#region src/init.ts
let ready;
let initialized = false;
/**
* Load the Termy WebAssembly module. Safe to call repeatedly; later calls
* share the first load. Without `source`, `termy.wasm` is resolved next to
* this module, which bundlers (Vite, webpack 5, Rspack, esbuild) and Node
* understand.
*/
function init(source) {
	ready ??= load(source).then(() => {
		initialized = true;
	}, (error) => {
		ready = void 0;
		throw error;
	});
	return ready;
}
/** Synchronous init from bytes or a compiled module, e.g. in a worker. */
function initSync(source) {
	if (initialized) return;
	initSync$1({ module: source });
	initialized = true;
	ready = Promise.resolve();
}
function isInitialized() {
	return initialized;
}
/** Throws a helpful error when the module is used before `init()`. */
function assertInitialized() {
	if (!initialized) throw new Error("@termysh/core: call `await init()` before creating a terminal");
}
async function load(source) {
	const resolved = source ?? new URL("./termy.wasm", import.meta.url);
	if (resolved instanceof URL && resolved.protocol === "file:") {
		const { readFile } = await import(
			/* @vite-ignore */
			"node:fs/promises"
);
		await __wbg_init({ module_or_path: await readFile(resolved) });
		return;
	}
	await __wbg_init({ module_or_path: resolved });
}
//#endregion
//#region src/cells.ts
/**
* Flat cell layout produced by `TermyCore.readRows`. Mirrors
* `crates/wasm/src/cells.rs`; each cell is `CELL_STRIDE` u32 slots.
*/
const CELL_STRIDE = 6;
const Slot = {
	Text: 0,
	Foreground: 1,
	Background: 2,
	UnderlineColor: 3,
	Style: 4,
	Link: 5
};
/** Slot 0 flag: the low bits index the read's string table (grapheme clusters). */
const STRING_FLAG = 2147483648;
/** Style bits in slot 4 (low 16 bits). */
const Attr = {
	Bold: 1,
	Dim: 2,
	Italic: 4,
	Inverse: 8,
	Hidden: 16,
	Strike: 32,
	Blink: 64
};
/** Cell width flags in slot 4, bits 16..19. */
const Width = {
	Wide: 1,
	WideSpacer: 2,
	LeadingWideSpacer: 4
};
const UNDERLINES = [
	"none",
	"single",
	"double",
	"curly",
	"dotted",
	"dashed"
];
function attributes(style) {
	return style & 65535;
}
function widthFlags(style) {
	return style >>> 16 & 7;
}
function underlineStyle(style) {
	return UNDERLINES[style >>> 20 & 7] ?? "none";
}
const ColorTag = {
	Default: 0,
	Indexed: 1,
	Rgb: 2
};
function colorTag(raw) {
	return raw >>> 24;
}
function decodeColor(raw) {
	switch (raw >>> 24) {
		case ColorTag.Indexed: return {
			kind: "indexed",
			index: raw & 255
		};
		case ColorTag.Rgb: return {
			kind: "rgb",
			rgb: raw & 16777215
		};
		default: return { kind: "default" };
	}
}
/** A block of viewport rows with accessors over the flat cell data. */
var CellRows = class {
	data;
	cols;
	startRow;
	strings;
	constructor(data, cols, startRow, strings) {
		this.data = data;
		this.cols = cols;
		this.startRow = startRow;
		this.strings = strings;
	}
	get rowCount() {
		return this.cols === 0 ? 0 : this.data.length / (this.cols * 6);
	}
	/** Slot offset of a cell; `row` is relative to `startRow`. */
	offset(row, col) {
		return (row * this.cols + col) * 6;
	}
	text(row, col) {
		const value = this.data[this.offset(row, col)] ?? 32;
		if (value & 2147483648) return this.strings[value & 2147483647] ?? "";
		return value === 0 ? " " : String.fromCodePoint(value);
	}
	link(row, col) {
		const value = this.data[this.offset(row, col) + Slot.Link] ?? 0;
		return value === 0 ? void 0 : this.strings[value - 1];
	}
};
//#endregion
//#region src/engine.ts
const Modifier = {
	Ctrl: 1,
	Alt: 2,
	Shift: 4,
	Meta: 8
};
const KEY_KINDS = {
	press: 0,
	repeat: 1,
	release: 2
};
const MOUSE_KINDS = {
	press: 0,
	release: 1,
	drag: 2,
	move: 3,
	wheelUp: 4,
	wheelDown: 5,
	wheelLeft: 6,
	wheelRight: 7
};
const MOUSE_BUTTONS = {
	left: 0,
	middle: 1,
	right: 2
};
const CURSOR_SHAPES = [
	"block",
	"bar",
	"underline"
];
const encoder = new TextEncoder();
const EMPTY = /* @__PURE__ */ new Uint8Array(0);
/**
* A headless terminal backed by Termy's Rust engine. Feed it output with
* `write`, read cells and damage for rendering, and encode user input into
* bytes for the host. It performs no I/O and has no DOM dependency.
*/
var TermyCore = class {
	#engine;
	#disposed = false;
	constructor(options = {}) {
		assertInitialized();
		this.#engine = new TermyEngine(options.cols ?? 80, options.rows ?? 24, options.scrollback ?? 1e3);
	}
	get cols() {
		return this.#engine.cols();
	}
	get rows() {
		return this.#engine.rows();
	}
	/** Increments whenever output or a viewport change may alter a read. */
	get generation() {
		return this.#engine.generation();
	}
	write(data) {
		if (typeof data === "string") this.#engine.feed_str(data);
		else this.#engine.feed(data);
	}
	resize(cols, rows) {
		this.#engine.resize(cols, rows);
	}
	/** Cell size in CSS pixels, used for pixel size reports and image layout. */
	setCellPixels(width, height) {
		this.#engine.set_cell_pixels(width, height);
	}
	setScrollback(lines) {
		this.#engine.set_scrollback(lines);
	}
	/** Protocol replies (device attributes, cursor reports, color queries). */
	takeReplies() {
		return this.#engine.take_replies();
	}
	takeEvents() {
		return this.#engine.take_events();
	}
	modes() {
		const bits = this.#engine.mode_bits();
		return {
			applicationCursor: (bits & 1) !== 0,
			applicationKeypad: (bits & 2) !== 0,
			bracketedPaste: (bits & 4) !== 0,
			focusEvents: (bits & 8) !== 0,
			mouseTracking: (bits & 16) !== 0,
			synchronizedUpdate: (bits & 32) !== 0,
			alternateScreen: (bits & 64) !== 0,
			cursorVisible: (bits & 128) !== 0,
			kittyKeyboardFlags: this.#engine.keyboard_flags()
		};
	}
	cursor() {
		const [row = 0, col = 0, visible = 1, shape = 0, blinking = 0] = this.#engine.cursor();
		return {
			row,
			col,
			visible: visible !== 0,
			shape: CURSOR_SHAPES[shape] ?? "block",
			blinking: blinking !== 0
		};
	}
	/** Default cursor shape until an application sets one with DECSCUSR. */
	setDefaultCursorShape(shape) {
		this.#engine.set_default_cursor_shape(CURSOR_SHAPES.indexOf(shape));
	}
	takeDamage() {
		const raw = this.#engine.take_damage();
		const scrollCount = raw[1] ?? 0;
		const scrolls = [];
		let index = 2;
		for (let i = 0; i < scrollCount; i++, index += 3) scrolls.push({
			top: raw[index],
			bottom: raw[index + 1],
			lines: raw[index + 2] | 0
		});
		const spans = [];
		for (; index + 2 < raw.length; index += 3) spans.push({
			row: raw[index],
			start: raw[index + 1],
			end: raw[index + 2]
		});
		return {
			full: raw[0] === 1,
			scrolls,
			spans
		};
	}
	/** Cells for viewport rows `[start, end)`. */
	readRows(start = 0, end = this.rows) {
		const data = this.#engine.read_rows(start, end);
		const strings = this.#engine.read_strings();
		return new CellRows(data, this.cols, start, strings);
	}
	/** Cells of one buffer line: `0..rows` is the live screen, negative is scrollback. */
	readLine(line) {
		return new CellRows(this.#engine.read_line(line), this.cols, line, this.#engine.read_strings());
	}
	/** Text of a buffer line: `0..rows` is the live screen, negative is scrollback. */
	lineText(line, trimEnd = true) {
		return this.#engine.line_text(line, trimEnd);
	}
	lineTextRange(line, start, end, trimEnd = true) {
		return this.#engine.line_text_range(line, start, end, trimEnd);
	}
	/** Whether `line` soft-wraps into the next one. */
	lineWrapped(line) {
		return this.#engine.line_wrapped(line);
	}
	get historySize() {
		return this.#engine.history_size();
	}
	/** Lines scrolled back from the live screen. */
	get displayOffset() {
		return this.#engine.display_offset();
	}
	/** Positive deltas scroll back into history. Returns whether it moved. */
	scrollDisplay(delta) {
		return this.#engine.scroll_display(delta);
	}
	scrollToBottom() {
		return this.#engine.scroll_to_bottom();
	}
	clearScrollback() {
		this.#engine.clear_scrollback();
	}
	/** Pack cold scrollback; call after output goes quiet. */
	compactHistory() {
		this.#engine.compact_history();
	}
	/** Milliseconds until a pending synchronized update (mode 2026) must flush, or -1. */
	syncDeadline() {
		return this.#engine.sync_deadline_ms();
	}
	flushSync() {
		return this.#engine.flush_sync();
	}
	/** OSC 4 palette overrides: 256 raw colors, 0 where the theme applies. */
	paletteOverrides() {
		return this.#engine.palette_overrides();
	}
	/** OSC 10/11/12 overrides as raw colors: `[foreground, background, cursor]`. */
	dynamicColorOverrides() {
		return this.#engine.dynamic_color_overrides();
	}
	get paletteRevision() {
		return this.#engine.palette_revision();
	}
	/** Colors reported to OSC 4/10/11/12 queries: `[fg, bg, cursor, ansi0..15]` as 0xRRGGBB. */
	setQueryColors(colors) {
		this.#engine.set_query_colors(Uint32Array.from(colors));
	}
	/**
	* Encode a key with Termy's native encoder (legacy, application cursor and
	* the kitty keyboard protocol). `key` is a Termy key name such as `enter`,
	* `up`, `f5` or `a`; `text` is the character it produced.
	*/
	encodeKey(key, text, modifiers, kind = "press", optionAsAlt = false) {
		const bytes = this.#engine.encode_key(key, text, modifiers, KEY_KINDS[kind], optionAsAlt);
		return bytes ? new Uint8Array(bytes) : void 0;
	}
	/** Encode a mouse report; undefined when the application is not tracking it. */
	encodeMouse(kind, button, col, row, modifiers) {
		const bytes = this.#engine.encode_mouse(MOUSE_KINDS[kind], MOUSE_BUTTONS[button], col, row, modifiers);
		return bytes ? new Uint8Array(bytes) : void 0;
	}
	encodePaste(text, bracketed = true) {
		if (!bracketed) return encoder.encode(text.replace(/\r?\n/g, "\r"));
		return this.#engine.encode_paste(text);
	}
	encodeFocus(focused) {
		return this.#engine.encode_focus(focused) ?? EMPTY;
	}
	/** Changes when kitty graphics placements or animation frames change. */
	get graphicsRevision() {
		return this.#engine.graphics_revision();
	}
	/** Visible kitty graphics placements, laid out for a cell size in CSS pixels. */
	readGraphics(cellWidth, cellHeight) {
		const raw = this.#engine.read_graphics(cellWidth, cellHeight);
		const stride = placementStride();
		const placements = [];
		for (let offset = 0, index = 0; offset + stride <= raw.length; offset += stride, index++) {
			const at = (slot) => raw[offset + slot];
			const optional = (slot) => at(slot) < 0 ? void 0 : at(slot);
			placements.push({
				index,
				imageId: at(0),
				placementId: at(1),
				imageGeneration: at(2),
				imageWidth: at(3),
				imageHeight: at(4),
				viewportRow: at(5),
				col: at(6),
				colOffset: at(7),
				sourceX: at(8),
				sourceY: at(9),
				sourceWidth: at(10),
				sourceHeight: at(11),
				displayCols: optional(12),
				displayRows: optional(13),
				occupiedCols: at(14),
				occupiedRows: at(15),
				clipTopRows: at(16),
				clipBottomRows: at(17),
				xOffset: at(18),
				yOffset: at(19),
				zIndex: at(20),
				placementSerial: at(21),
				clip: {
					left: at(22),
					top: at(23),
					width: at(24),
					height: at(25)
				},
				draw: {
					left: at(26),
					top: at(27),
					width: at(28),
					height: at(29)
				}
			});
		}
		return placements;
	}
	/** Pixels for a placement from the most recent `readGraphics()`. */
	graphicsImage(index) {
		const data = this.#engine.graphics_image(index);
		if (!data) return void 0;
		return {
			format: this.#engine.graphics_image_is_png(index) ? "png" : "rgba",
			data
		};
	}
	/** Milliseconds until the next visible animation frame, or -1. */
	graphicsDeadline() {
		return this.#engine.graphics_deadline_ms();
	}
	dispose() {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#engine.free();
	}
};
/** Number of u32 slots per cell in `CellRows.data`. */
function cellSlots() {
	return cellStride();
}
//#endregion
//#region src/themes.ts
/** Ids of the themes bundled with Termy, e.g. `termy`, `tokyo-night`, `dracula`. */
function builtinThemeIds() {
	assertInitialized();
	return themeIds();
}
/** A bundled Termy theme by id (case and separator insensitive). */
function builtinTheme(id) {
	assertInitialized();
	const colors = themeColors(id);
	if (!colors) return void 0;
	const hex = (value) => `#${(value ?? 0).toString(16).padStart(6, "0")}`;
	return {
		foreground: hex(colors[0]),
		background: hex(colors[1]),
		cursor: hex(colors[2]),
		ansi: Array.from(colors.subarray(3, 19), hex)
	};
}
//#endregion
//#region src/glyphs.ts
const KINDS = [
	"block",
	"box",
	"sextant",
	"braille",
	"roundedCorner",
	"diagonal"
];
const RECT_STRIDE = 6;
const STROKE_STRIDE = 15;
/**
* Termy's pixel-exact geometry for block elements, box drawing, sextants,
* Braille runs, rounded corners and diagonals. Returns undefined for
* characters that should be shaped as normal text. `neighbors` are the code
* points two before, before, after and two after in the row (0 when absent).
*/
function glyphPlan(codePoint, neighbors, cellWidth, cellHeight, fontSize) {
	const raw = glyphPlan$1(codePoint, Uint32Array.from(neighbors), cellWidth, cellHeight, fontSize);
	if (!raw) return void 0;
	const rectCount = raw[1] ?? 0;
	const strokeCount = raw[2] ?? 0;
	const rects = [];
	let offset = 3;
	for (let i = 0; i < rectCount; i++, offset += RECT_STRIDE) rects.push({
		left: raw[offset],
		top: raw[offset + 1],
		right: raw[offset + 2],
		bottom: raw[offset + 3],
		alpha: raw[offset + 4],
		snap: raw[offset + 5] === 1 ? "outward" : "nearest"
	});
	const strokes = [];
	for (let i = 0; i < strokeCount; i++, offset += STROKE_STRIDE) {
		const pointCount = raw[offset + 2];
		const points = [];
		for (let p = 0; p < pointCount; p++) points.push({
			x: raw[offset + 3 + p * 2],
			y: raw[offset + 4 + p * 2]
		});
		strokes.push({
			kind: raw[offset] === 1 ? "roundedCorner" : "line",
			width: raw[offset + 1],
			points
		});
	}
	return {
		kind: KINDS[raw[0] ?? 0] ?? "block",
		rects,
		strokes
	};
}
//#endregion
export { Attr, CELL_STRIDE, CellRows, ColorTag, Modifier, STRING_FLAG, Slot, TermyCore, Width, attributes, builtinTheme, builtinThemeIds, cellSlots, colorTag, decodeColor, glyphPlan, init, initSync, isInitialized, underlineStyle, widthFlags };
