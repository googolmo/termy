//! Termy's independent terminal engine.
//!
//! The parser, screen storage and protocol state are owned here. Parsing and
//! borrowed screen reads need neither a renderer nor a PTY, and do not depend on
//! Alacritty or the legacy experimental engine. See `README.md` for migration
//! status and the remaining integration gates.

mod dispatch;
mod grid;
mod parser;
mod types;

use std::collections::VecDeque;

pub use types::{
    Cell, CellExtra, Color, Cursor, CursorShape, Damage, DirtySpan, Hyperlink, Size, Style,
    UnderlineStyle,
};

use dispatch::State;
use parser::Parser;

const MAX_EVENTS: usize = 1024;
const MAX_REPLY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub scrollback_history: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scrollback_history: 1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Bell,
    Title(String),
    WorkingDirectory(String),
    ShellIntegration(String),
    Clipboard { selection: String, data: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseTracking {
    #[default]
    None,
    Press,
    Click,
    Drag,
    Motion,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseEncoding {
    #[default]
    Default,
    Utf8,
    Sgr,
    Urxvt,
    SgrPixels,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    pub application_cursor: bool,
    pub application_keypad: bool,
    pub bracketed_paste: bool,
    pub focus_events: bool,
    pub mouse_tracking: MouseTracking,
    pub mouse_encoding: MouseEncoding,
    pub kitty_keyboard: u8,
    pub synchronized_update: bool,
}

/// Reusable, single-owner parser and grid. The runtime controls synchronization;
/// the hot parsing path performs no locking and ordinary screen reads borrow.
pub struct Engine {
    parser: Parser,
    state: State,
    generation: u64,
}

impl Engine {
    pub fn new(size: Size, options: Options) -> Self {
        Self {
            parser: Parser::default(),
            state: State::new(size, options),
            generation: 0,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.parser.advance(&mut self.state, bytes);
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn size(&self) -> Size {
        self.state.grid.size()
    }
    pub fn cursor(&self) -> Cursor {
        self.state.grid.cursor
    }
    pub fn modes(&self) -> Modes {
        self.state.modes
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn history_size(&self) -> usize {
        self.state.grid.history_size()
    }
    pub fn display_offset(&self) -> usize {
        self.state.grid.display_offset()
    }
    pub fn alternate_screen(&self) -> bool {
        self.state.grid.alternate_screen()
    }

    pub fn set_options(&mut self, options: Options) {
        self.state
            .grid
            .set_history_limit(options.scrollback_history);
        self.generation = self.generation.wrapping_add(1);
    }

    /// The lifetime of the slice prevents mutation while the renderer reads it.
    pub fn viewport_row(&self, row: usize) -> Option<&[Cell]> {
        self.state.grid.visible_row(row).map(|row| row.cells())
    }

    pub fn viewport_row_wrapped(&self, row: usize) -> bool {
        self.state
            .grid
            .visible_row(row)
            .is_some_and(|row| row.wrapped)
    }

    /// Lines before the live screen are negative, with -1 the newest history row.
    pub fn line(&self, line: i32) -> Option<&[Cell]> {
        self.state.grid.row(line).map(|row| row.cells())
    }

    pub fn take_damage(&mut self) -> Damage {
        self.state.grid.take_damage()
    }

    pub fn resize(&mut self, size: Size) {
        self.state.grid.resize(size);
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn scroll_display(&mut self, delta: i32) -> bool {
        let changed = self.state.grid.scroll_display(delta);
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
        changed
    }

    pub fn clear_scrollback(&mut self) {
        self.state.grid.clear_scrollback();
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn pop_event(&mut self) -> Option<Event> {
        self.state.events.pop_front()
    }

    /// Draining into a caller-owned buffer allows the PTY writer to reuse it.
    pub fn drain_replies(&mut self, output: &mut Vec<u8>) {
        output.append(&mut self.state.replies);
    }

    pub fn palette(&self) -> &[Option<Color>; 256] {
        &self.state.palette
    }
    pub fn foreground(&self) -> Option<Color> {
        self.state.foreground
    }
    pub fn background(&self) -> Option<Color> {
        self.state.background
    }
    pub fn cursor_color(&self) -> Option<Color> {
        self.state.cursor_color
    }
    pub fn palette_revision(&self) -> u64 {
        self.state.palette_revision
    }

    /// Reports bounded output queues reaching capacity. The runtime can drain
    /// them between reads; hostile output cannot grow these queues forever.
    pub fn dropped_events(&self) -> u64 {
        self.state.dropped_events
    }
    pub fn dropped_reply_bytes(&self) -> u64 {
        self.state.dropped_reply_bytes
    }
}

fn enqueue(events: &mut VecDeque<Event>, dropped: &mut u64, event: Event) {
    if events.len() == MAX_EVENTS {
        *dropped = dropped.saturating_add(1);
    } else {
        events.push_back(event);
    }
}

#[cfg(test)]
mod tests;
