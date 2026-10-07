//! Flat, renderer-neutral cell encoding shared with `@termysh/web`.
//!
//! Each cell occupies [`CELL_STRIDE`] `u32` slots:
//!
//! | slot | meaning |
//! | ---- | ------- |
//! | 0 | Unicode scalar, or `STRING_FLAG \| index` into the read's string table |
//! | 1 | foreground color (raw, see below) |
//! | 2 | background color |
//! | 3 | underline color |
//! | 4 | style bits (`0..16`), cell width flags (`16..19`), underline style (`20..23`) |
//! | 5 | hyperlink: `0`, or string index + 1 of its URI |
//!
//! Raw colors keep the engine's tag in the top byte: `0` default, `1` indexed
//! (low byte is the palette index), `2` RGB (`0x02RRGGBB`).

use termy_core::terminal_engine::{Cell, Color, Engine, UnderlineStyle};

pub const CELL_STRIDE: usize = 6;
pub const STRING_FLAG: u32 = 0x8000_0000;

#[derive(Default)]
pub struct CellBuffer {
    strings: Vec<String>,
}

impl CellBuffer {
    pub fn read_viewport(&mut self, engine: &Engine, start: usize, end: usize) -> Vec<u32> {
        self.strings.clear();
        let size = engine.size();
        let end = end.min(size.rows);
        let start = start.min(end);
        let mut out = vec![0; (end - start) * size.cols * CELL_STRIDE];
        for (offset, row) in (start..end).enumerate() {
            let Some(cells) = engine.viewport_row(row) else {
                continue;
            };
            let base = offset * size.cols * CELL_STRIDE;
            for (col, cell) in cells.iter().take(size.cols).enumerate() {
                let slot = base + col * CELL_STRIDE;
                self.encode(cell, &mut out[slot..slot + CELL_STRIDE]);
            }
        }
        out
    }

    /// Flat cells for one buffer line (negative lines are scrollback).
    pub fn read_line(&mut self, engine: &Engine, line: i32) -> Vec<u32> {
        self.strings.clear();
        let cols = engine.size().cols;
        let mut out = vec![0; cols * CELL_STRIDE];
        if let Some(cells) = engine.line(line) {
            for (col, cell) in cells.iter().take(cols).enumerate() {
                let slot = col * CELL_STRIDE;
                self.encode(cell, &mut out[slot..slot + CELL_STRIDE]);
            }
        }
        out
    }

    pub fn strings(&self) -> &[String] {
        &self.strings
    }

    pub fn string(&self, index: usize) -> Option<&str> {
        self.strings.get(index).map(String::as_str)
    }

    fn intern(&mut self, value: String) -> u32 {
        self.strings.push(value);
        (self.strings.len() - 1) as u32
    }

    fn encode(&mut self, cell: &Cell, out: &mut [u32]) {
        let combining = cell.combining();
        out[0] = if combining.is_empty() {
            u32::from(cell.character)
        } else {
            let mut text = String::with_capacity(4 + combining.len());
            text.push(cell.character);
            text.push_str(combining);
            STRING_FLAG | self.intern(text)
        };
        out[1] = raw_color(cell.style.foreground);
        out[2] = raw_color(cell.style.background);
        out[3] = raw_color(cell.style.underline_color);
        out[4] = u32::from(cell.style.attributes)
            | (u32::from(cell.flags & 0x7) << 16)
            | (underline_code(cell.style.underline) << 20);
        out[5] = cell
            .hyperlink()
            .map_or(0, |link| self.intern(link.uri.clone()) + 1);
    }
}

pub fn raw_color(color: Color) -> u32 {
    if let Some(index) = color.as_indexed() {
        0x0100_0000 | u32::from(index)
    } else if let Some((r, g, b)) = color.as_rgb() {
        0x0200_0000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    } else {
        0
    }
}

fn underline_code(style: UnderlineStyle) -> u32 {
    match style {
        UnderlineStyle::None => 0,
        UnderlineStyle::Single => 1,
        UnderlineStyle::Double => 2,
        UnderlineStyle::Curly => 3,
        UnderlineStyle::Dotted => 4,
        UnderlineStyle::Dashed => 5,
    }
}

/// Plain text of a row, skipping wide-character spacer cells.
pub fn line_text(cells: &[Cell], trim_end: bool) -> String {
    let mut text = String::with_capacity(cells.len());
    for cell in cells {
        if cell.flags & (Cell::WIDE_SPACER | Cell::LEADING_WIDE_SPACER) != 0 {
            continue;
        }
        text.push(cell.character);
        text.push_str(cell.combining());
    }
    if trim_end {
        text.truncate(text.trim_end_matches(' ').len());
    }
    text
}
