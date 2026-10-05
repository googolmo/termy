//! Compact, engine-owned terminal values. No renderer or PTY dependencies.

use std::sync::Arc;

pub(super) const MAX_COMBINING_BYTES: usize = 256;

/// A color occupies four bytes, including its default/indexed/RGB tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color(u32);

impl Color {
    pub const DEFAULT: Self = Self(0);
    pub const fn indexed(index: u8) -> Self {
        Self(0x0100_0000 | index as u32)
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0x0200_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }
    pub const fn as_indexed(self) -> Option<u8> {
        if self.0 >> 24 == 1 {
            Some(self.0 as u8)
        } else {
            None
        }
    }
    pub const fn as_rgb(self) -> Option<(u8, u8, u8)> {
        if self.0 >> 24 == 2 {
            Some(((self.0 >> 16) as u8, (self.0 >> 8) as u8, self.0 as u8))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub foreground: Color,
    pub background: Color,
    pub underline_color: Color,
    pub attributes: u16,
    pub underline: UnderlineStyle,
}

impl Style {
    pub const BOLD: u16 = 1;
    pub const DIM: u16 = 1 << 1;
    pub const ITALIC: u16 = 1 << 2;
    pub const INVERSE: u16 = 1 << 3;
    pub const HIDDEN: u16 = 1 << 4;
    pub const STRIKE: u16 = 1 << 5;
    pub const BLINK: u16 = 1 << 6;
    pub const PROTECTED: u16 = 1 << 7;

    pub fn set(&mut self, flags: u16, enabled: bool) {
        if enabled {
            self.attributes |= flags;
        } else {
            self.attributes &= !flags;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hyperlink {
    pub id: String,
    pub uri: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellExtra {
    pub combining: String,
    pub hyperlink: Option<Arc<Hyperlink>>,
}

/// Plain cells allocate nothing. Uncommon combining text and OSC 8 metadata
/// live behind a shared pointer so row copies do not copy strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub character: char,
    pub style: Style,
    pub flags: u16,
    pub extra: Option<Arc<CellExtra>>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            style: Style::default(),
            flags: 0,
            extra: None,
        }
    }
}

impl Cell {
    pub const WIDE: u16 = 1;
    pub const WIDE_SPACER: u16 = 1 << 1;
    pub const LEADING_WIDE_SPACER: u16 = 1 << 2;
    pub fn combining(&self) -> &str {
        self.extra
            .as_ref()
            .map_or("", |extra| extra.combining.as_str())
    }
    pub fn hyperlink(&self) -> Option<&Arc<Hyperlink>> {
        self.extra
            .as_ref()
            .and_then(|extra| extra.hyperlink.as_ref())
    }
    pub(super) fn push_combining(&mut self, character: char) {
        // Bound a hostile stream of combining marks attached to a single cell.
        if self.combining().len().saturating_add(character.len_utf8()) > MAX_COMBINING_BYTES {
            return;
        }
        let extra = self
            .extra
            .get_or_insert_with(|| Arc::new(CellExtra::default()));
        Arc::make_mut(extra).combining.push(character);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    #[default]
    Block,
    Beam,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub visible: bool,
    pub shape: CursorShape,
    pub blinking: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            shape: CursorShape::Block,
            blinking: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtySpan {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// Column ranges are half open, matching Rust slice indexing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Damage {
    Full,
    Partial(Vec<DirtySpan>),
}

/// A viewport row rotation to replay before applying the accompanying damage.
/// `bottom` is exclusive; positive `lines` move rows up, negative values down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportScroll {
    pub top: usize,
    pub bottom: usize,
    pub lines: i32,
}

/// Ordered text-grid operations consumed by the graphics placement bridge.
/// Scroll regions and column spans both use exclusive upper bounds. Positive
/// line counts move cells up; negative counts move them down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridEffect {
    Scroll {
        alternate: bool,
        top: usize,
        bottom: usize,
        lines: i64,
        retains_history: bool,
        history_before: usize,
        history_after: usize,
    },
    Clear {
        alternate: bool,
        history_size: usize,
    },
    ClearHistory {
        removed: usize,
    },
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub cols: usize,
    pub rows: usize,
}

impl Default for Size {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

impl Size {
    pub const MAX_COLS: usize = 4096;
    pub const MAX_ROWS: usize = 4096;
    pub const MAX_CELLS: usize = 1_048_576;
    pub(super) fn clamped(self) -> Self {
        let cols = self.cols.clamp(1, Self::MAX_COLS);
        let rows = self
            .rows
            .clamp(1, Self::MAX_ROWS)
            .min(Self::MAX_CELLS / cols);
        Self { cols, rows }
    }
}
