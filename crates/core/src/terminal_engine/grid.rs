//! Row-oriented terminal storage. Scrolling moves row handles, not cells.

use std::collections::VecDeque;

use unicode_width::UnicodeWidthChar;

use super::types::{Cell, Cursor, Damage, DirtySpan, GridEffect, Size, Style};

mod combining;
use combining::CombiningCache;

const MAX_HISTORY_ROWS: usize = 20_000;

#[derive(Clone, Debug)]
pub(super) struct Row {
    cells: Vec<Cell>,
    pub(super) wrapped: bool,
}

impl Row {
    fn new(cols: usize, blank: &Cell) -> Self {
        Self {
            cells: vec![blank.clone(); cols],
            wrapped: false,
        }
    }

    pub(super) fn cells(&self) -> &[Cell] {
        &self.cells
    }

    fn clear(&mut self, cols: usize, blank: &Cell) {
        self.cells.resize(cols, blank.clone());
        self.cells.fill(blank.clone());
        self.wrapped = false;
    }

    fn content_len(&self) -> usize {
        if self.wrapped {
            return self.cells.len();
        }
        self.cells
            .iter()
            .rposition(|cell| cell != &Cell::default())
            .map_or(0, |index| index + 1)
    }
}

#[derive(Clone, Default)]
struct SavedCursor {
    cursor: Cursor,
    pen: Cell,
    pending_wrap: bool,
    origin_mode: bool,
    autowrap: bool,
}

struct Screen {
    rows: VecDeque<Row>,
    saved: SavedCursor,
    cursor: Cursor,
    pending_wrap: bool,
}

struct ReflowedLine {
    rows: VecDeque<Row>,
    cursor: Option<(usize, usize, bool)>,
    boundary: Option<usize>,
    viewport: Option<usize>,
}

impl Screen {
    fn new(size: Size) -> Self {
        Self {
            rows: (0..size.rows)
                .map(|_| Row::new(size.cols, &Cell::default()))
                .collect(),
            saved: SavedCursor {
                autowrap: true,
                ..SavedCursor::default()
            },
            cursor: Cursor::default(),
            pending_wrap: false,
        }
    }
}

pub(super) struct Grid {
    size: Size,
    primary: Screen,
    alternate: Option<Screen>,
    alternate_active: bool,
    history: VecDeque<Row>,
    requested_history_limit: usize,
    history_limit: usize,
    display_offset: usize,
    tabs: Vec<bool>,
    scroll_top: usize,
    scroll_bottom: usize,
    pending_wrap: bool,
    // Erasing the display establishes a boundary that width reflow must not
    // cross when choosing the new live viewport.
    clear_anchor: bool,
    full_damage: bool,
    dirty: Vec<Option<(usize, usize)>>,
    combining_cache: CombiningCache,
    track_effects: bool,
    effects: Vec<GridEffect>,
    pub(super) cursor: Cursor,
    pub(super) pen: Cell,
    pub(super) autowrap: bool,
    pub(super) origin_mode: bool,
    pub(super) insert_mode: bool,
    pub(super) newline_mode: bool,
}

impl Grid {
    pub(super) fn new(size: Size, history_limit: usize) -> Self {
        let size = size.clamped();
        Self {
            size,
            primary: Screen::new(size),
            alternate: None,
            alternate_active: false,
            history: VecDeque::new(),
            requested_history_limit: history_limit.min(MAX_HISTORY_ROWS),
            history_limit: Self::bounded_history(size, history_limit),
            display_offset: 0,
            tabs: Self::default_tabs(size.cols),
            scroll_top: 0,
            scroll_bottom: size.rows,
            pending_wrap: false,
            clear_anchor: false,
            full_damage: true,
            dirty: vec![None; size.rows],
            combining_cache: CombiningCache::default(),
            track_effects: false,
            effects: Vec::new(),
            cursor: Cursor::default(),
            pen: Cell::default(),
            autowrap: true,
            origin_mode: false,
            insert_mode: false,
            newline_mode: false,
        }
    }

    fn bounded_history(size: Size, requested: usize) -> usize {
        requested
            .min(MAX_HISTORY_ROWS)
            .min(Size::MAX_CELLS / size.cols)
    }

    fn default_tabs(cols: usize) -> Vec<bool> {
        (0..cols).map(|col| col != 0 && col % 8 == 0).collect()
    }
    fn screen(&self) -> &Screen {
        if self.alternate_active {
            self.alternate.as_ref().expect("active alternate screen")
        } else {
            &self.primary
        }
    }
    fn screen_mut(&mut self) -> &mut Screen {
        if self.alternate_active {
            self.alternate.as_mut().expect("active alternate screen")
        } else {
            &mut self.primary
        }
    }
    pub(super) fn size(&self) -> Size {
        self.size
    }
    pub(super) fn history_size(&self) -> usize {
        if self.alternate_active {
            0
        } else {
            self.history.len()
        }
    }
    pub(super) fn display_offset(&self) -> usize {
        if self.alternate_active {
            0
        } else {
            self.display_offset
        }
    }
    pub(super) fn alternate_screen(&self) -> bool {
        self.alternate_active
    }
    pub(super) fn scroll_region(&self) -> (usize, usize) {
        (self.scroll_top, self.scroll_bottom)
    }

    /// History has negative line numbers; the live screen starts at zero.
    pub(super) fn row(&self, line: i32) -> Option<&Row> {
        if line >= 0 {
            return self.screen().rows.get(line as usize);
        }
        if self.alternate_active {
            return None;
        }
        self.history
            .len()
            .checked_sub(line.unsigned_abs() as usize)
            .and_then(|index| self.history.get(index))
    }

    pub(super) fn visible_row(&self, row: usize) -> Option<&Row> {
        if row >= self.size.rows {
            return None;
        }
        self.row(row as i32 - self.display_offset() as i32)
    }

    pub(super) fn mark_full_damage(&mut self) {
        self.full_damage = true;
    }

    pub(super) fn set_effect_tracking(&mut self, enabled: bool) {
        self.track_effects = enabled;
        if !enabled {
            self.effects.clear();
        }
    }

    pub(super) fn drain_effects(&mut self, output: &mut Vec<GridEffect>) {
        output.append(&mut self.effects);
    }

    fn effect(&mut self, effect: GridEffect) {
        if !self.track_effects {
            return;
        }
        if let GridEffect::Scroll {
            alternate,
            top,
            bottom,
            lines,
            retains_history,
            history_before,
            history_after,
        } = effect
            && let Some(GridEffect::Scroll {
                alternate: previous_alternate,
                top: previous_top,
                bottom: previous_bottom,
                lines: previous_lines,
                retains_history: previous_retains_history,
                history_after: previous_history_after,
                ..
            }) = self.effects.last_mut()
            && *previous_alternate == alternate
            && *previous_top == top
            && *previous_bottom == bottom
            && previous_lines.signum() == lines.signum()
            && *previous_retains_history == retains_history
            && *previous_history_after == history_before
        {
            *previous_lines = previous_lines.saturating_add(lines);
            *previous_history_after = history_after;
            return;
        }
        self.effects.push(effect);
    }

    fn mark(&mut self, row: usize, start: usize, end: usize) {
        if self.full_damage || row >= self.size.rows || start >= end {
            return;
        }
        let row = row.saturating_add(self.display_offset());
        if let Some(dirty) = self.dirty.get_mut(row) {
            let end = end.min(self.size.cols);
            *dirty = Some(match *dirty {
                Some((old_start, old_end)) => (old_start.min(start), old_end.max(end)),
                None => (start, end),
            });
        }
    }

    fn mark_cursor(&mut self, cursor: Cursor) {
        if cursor.visible {
            self.mark(cursor.row, cursor.col, cursor.col.saturating_add(1));
        }
    }

    fn motion_done(&mut self, old: Cursor) {
        self.pending_wrap = false;
        self.mark_cursor(old);
        self.mark_cursor(self.cursor);
    }

    pub(super) fn cursor_changed(&mut self, old: Cursor) {
        self.mark_cursor(old);
        self.mark_cursor(self.cursor);
    }

    pub(super) fn take_damage(&mut self) -> Damage {
        if std::mem::take(&mut self.full_damage) {
            self.dirty.fill(None);
            return Damage::Full;
        }
        let spans = self
            .dirty
            .iter_mut()
            .enumerate()
            .filter_map(|(row, dirty)| {
                dirty
                    .take()
                    .map(|(start, end)| DirtySpan { row, start, end })
            })
            .collect();
        Damage::Partial(spans)
    }

    fn blank(&self) -> Cell {
        Cell {
            style: Style {
                background: self.pen.style.background,
                ..Style::default()
            },
            ..Cell::default()
        }
    }

    fn clear_wide_at(row: &mut Row, col: usize, blank: &Cell) -> (usize, usize) {
        let mut start = col;
        let mut end = col + 1;
        if row.cells[col].flags & Cell::WIDE_SPACER != 0 && col > 0 {
            start -= 1;
            row.cells[start] = blank.clone();
        }
        if row.cells[col].flags & Cell::WIDE != 0 && end < row.cells.len() {
            row.cells[end] = blank.clone();
            end += 1;
        }
        row.cells[col] = blank.clone();
        (start, end)
    }

    pub(super) fn put_char(&mut self, character: char) {
        self.observe_output();
        let width = character.width().unwrap_or(0);
        if width == 0 {
            let mut row = self.cursor.row;
            let mut col = self.cursor.col;
            if !self.pending_wrap {
                if col > 0 {
                    col -= 1;
                } else if row > 0 && self.screen().rows[row - 1].wrapped {
                    row -= 1;
                    col = self.size.cols - 1;
                } else {
                    return;
                }
            }
            if self.screen().rows[row].cells[col].flags & Cell::WIDE_SPACER != 0 && col > 0 {
                col -= 1;
            }
            let cell = if self.alternate_active {
                &mut self
                    .alternate
                    .as_mut()
                    .expect("active alternate screen")
                    .rows[row]
                    .cells[col]
            } else {
                &mut self.primary.rows[row].cells[col]
            };
            self.combining_cache.append(cell, character);
            self.mark(row, col, col + 1);
            return;
        }

        let old = self.cursor;
        if self.pending_wrap {
            if self.autowrap {
                self.screen_mut().rows[old.row].wrapped = true;
                self.cursor.col = 0;
                self.linefeed();
            }
            self.pending_wrap = false;
        }
        let width = width.min(self.size.cols).min(2);
        if width == 2 && self.cursor.col + 1 == self.size.cols {
            if !self.autowrap {
                return;
            }
            let row = self.cursor.row;
            let col = self.cursor.col;
            let blank = self.blank();
            let active = &mut self.screen_mut().rows[row];
            Self::clear_wide_at(active, col, &blank);
            active.cells[col].flags = Cell::LEADING_WIDE_SPACER;
            active.wrapped = true;
            self.mark(row, col.saturating_sub(1), col + 1);
            self.cursor.col = 0;
            self.linefeed();
        }
        if self.insert_mode {
            self.insert_chars(width);
        }

        let row = self.cursor.row;
        let col = self.cursor.col;
        let blank = self.blank();
        let mut cell = self.pen.clone();
        cell.character = character;
        cell.flags = if width == 2 { Cell::WIDE } else { 0 };
        let active = &mut self.screen_mut().rows[row];
        let (mut start, mut end) = Self::clear_wide_at(active, col, &blank);
        if width == 2 {
            let (next_start, next_end) = Self::clear_wide_at(active, col + 1, &blank);
            start = start.min(next_start);
            end = end.max(next_end);
            let mut spacer = cell.clone();
            spacer.character = ' ';
            spacer.flags = Cell::WIDE_SPACER;
            spacer.extra = None;
            active.cells[col + 1] = spacer;
        }
        active.cells[col] = cell;
        if col + width >= self.size.cols {
            self.cursor.col = self.size.cols - 1;
            self.pending_wrap = self.autowrap;
        } else {
            self.cursor.col += width;
        }
        self.mark(row, start, end);
        self.mark_cursor(old);
        self.mark_cursor(self.cursor);
    }

    /// Ordinary ASCII uses one bounds/damage update per row-local run.
    pub(super) fn write_ascii(&mut self, mut text: &[u8]) {
        debug_assert!(text.iter().all(|byte| (0x20..=0x7e).contains(byte)));
        while !text.is_empty() {
            self.observe_output();
            if self.pending_wrap || self.insert_mode || !self.autowrap {
                self.put_char(char::from(text[0]));
                text = &text[1..];
                continue;
            }
            let row = self.cursor.row;
            let col = self.cursor.col;
            let count = text.len().min(self.size.cols - col);
            // Wide-cell repair is uncommon, and the scalar path handles both
            // ends of an overwritten glyph without a second general scan.
            if self.screen().rows[row].cells[col..col + count]
                .iter()
                .any(|cell| cell.flags != 0)
            {
                for &byte in &text[..count] {
                    self.put_char(char::from(byte));
                }
                text = &text[count..];
                continue;
            }
            let pen = self.pen.clone();
            let active = &mut self.screen_mut().rows[row];
            for (cell, &byte) in active.cells[col..col + count].iter_mut().zip(text) {
                cell.clone_from(&pen);
                cell.character = char::from(byte);
                cell.flags = 0;
            }
            self.cursor.col += count;
            if self.cursor.col == self.size.cols {
                self.cursor.col -= 1;
                self.pending_wrap = true;
            }
            self.mark(row, col, (col + count + 1).min(self.size.cols));
            text = &text[count..];
        }
    }

    pub(super) fn carriage_return(&mut self) {
        let old = self.cursor;
        self.cursor.col = 0;
        self.motion_done(old);
    }
    pub(super) fn backspace(&mut self) {
        let old = self.cursor;
        self.cursor.col = self.cursor.col.saturating_sub(1);
        self.motion_done(old);
    }

    pub(super) fn linefeed(&mut self) {
        let old = self.cursor;
        if self.cursor.row + 1 == self.scroll_bottom {
            self.scroll_up(1);
        } else {
            self.cursor.row = (self.cursor.row + 1).min(self.size.rows - 1);
        }
        if self.newline_mode {
            self.cursor.col = 0;
        }
        self.motion_done(old);
        self.observe_output();
    }

    fn observe_output(&mut self) {
        if !self.alternate_active && self.cursor.row + 1 == self.size.rows {
            self.clear_anchor = false;
        }
    }

    pub(super) fn reverse_index(&mut self) {
        let old = self.cursor;
        if self.cursor.row == self.scroll_top {
            self.scroll_down(1);
        } else {
            self.cursor.row = self.cursor.row.saturating_sub(1);
        }
        self.motion_done(old);
    }

    pub(super) fn goto(&mut self, row: usize, col: usize) {
        let old = self.cursor;
        self.cursor.row = if self.origin_mode {
            row.saturating_add(self.scroll_top)
                .min(self.scroll_bottom - 1)
        } else {
            row.min(self.size.rows - 1)
        };
        self.cursor.col = col.min(self.size.cols - 1);
        self.motion_done(old);
    }

    pub(super) fn move_cursor(&mut self, row_delta: isize, col_delta: isize) {
        let old = self.cursor;
        let (top, bottom) = if self.origin_mode
            || (self.cursor.row >= self.scroll_top && self.cursor.row < self.scroll_bottom)
        {
            (self.scroll_top, self.scroll_bottom)
        } else {
            (0, self.size.rows)
        };
        self.cursor.row = self
            .cursor
            .row
            .saturating_add_signed(row_delta)
            .clamp(top, bottom - 1);
        self.cursor.col = self
            .cursor
            .col
            .saturating_add_signed(col_delta)
            .min(self.size.cols - 1);
        self.motion_done(old);
    }

    pub(super) fn tab(&mut self) {
        let old = self.cursor;
        self.cursor.col = ((self.cursor.col + 1)..self.size.cols)
            .find(|&col| self.tabs[col])
            .unwrap_or(self.size.cols - 1);
        self.motion_done(old);
    }
    pub(super) fn backtab(&mut self) {
        let old = self.cursor;
        self.cursor.col = (0..self.cursor.col)
            .rev()
            .find(|&col| self.tabs[col])
            .unwrap_or(0);
        self.motion_done(old);
    }
    pub(super) fn set_tab(&mut self) {
        self.tabs[self.cursor.col] = true;
    }
    pub(super) fn clear_tab(&mut self, all: bool) {
        if all {
            self.tabs.fill(false);
        } else {
            self.tabs[self.cursor.col] = false;
        }
    }

    fn erase_range(&mut self, row: usize, start: usize, end: usize, selective: bool) {
        if start >= end {
            return;
        }
        let blank = self.blank();
        let active = &mut self.screen_mut().rows[row];
        if !selective {
            let (first_left, first_right) = Self::clear_wide_at(active, start, &blank);
            let (last_left, last_right) = Self::clear_wide_at(active, end - 1, &blank);
            active.cells[start..end].fill(blank);
            if end == active.cells.len() {
                active.wrapped = false;
            }
            self.mark(row, first_left.min(last_left), first_right.max(last_right));
            return;
        }
        let mut changed_start = end;
        let mut changed_end = start;
        let mut col = start;
        while col < end {
            let protected = active.cells[col].style.attributes & Style::PROTECTED != 0;
            if !selective || !protected {
                let (left, right) = Self::clear_wide_at(active, col, &blank);
                changed_start = changed_start.min(left);
                changed_end = changed_end.max(right);
            }
            col += 1;
        }
        if end == active.cells.len() && (!selective || changed_end == end) {
            active.wrapped = false;
        }
        self.mark(row, changed_start, changed_end);
    }

    pub(super) fn erase_display(&mut self, mode: u16, selective: bool) {
        self.pending_wrap = false;
        let full_clear = mode == 2
            || (mode == 0 && self.cursor.row == 0 && self.cursor.col == 0)
            || (mode == 1
                && self.cursor.row + 1 == self.size.rows
                && self.cursor.col + 1 == self.size.cols);
        match mode {
            0 => {
                self.erase_range(self.cursor.row, self.cursor.col, self.size.cols, selective);
                for row in self.cursor.row + 1..self.size.rows {
                    self.erase_range(row, 0, self.size.cols, selective);
                }
            }
            1 => {
                for row in 0..self.cursor.row {
                    self.erase_range(row, 0, self.size.cols, selective);
                }
                self.erase_range(self.cursor.row, 0, self.cursor.col + 1, selective);
            }
            2 => {
                for row in 0..self.size.rows {
                    self.erase_range(row, 0, self.size.cols, selective);
                }
                if !self.alternate_active && !selective {
                    self.clear_anchor = true;
                }
            }
            3 => {
                self.clear_scrollback();
            }
            _ => {}
        }
        if full_clear && !selective {
            self.effect(GridEffect::Clear {
                alternate: self.alternate_active,
                history_size: self.history_size(),
            });
        }
    }

    pub(super) fn erase_line(&mut self, mode: u16, selective: bool) {
        self.pending_wrap = false;
        let (start, end) = match mode {
            0 => (self.cursor.col, self.size.cols),
            1 => (0, self.cursor.col + 1),
            2 => (0, self.size.cols),
            _ => return,
        };
        self.erase_range(self.cursor.row, start, end, selective);
    }

    pub(super) fn erase_chars(&mut self, count: usize) {
        self.pending_wrap = false;
        self.erase_range(
            self.cursor.row,
            self.cursor.col,
            self.cursor.col.saturating_add(count).min(self.size.cols),
            false,
        );
    }

    fn repair_wide(row: &mut Row, blank: &Cell) {
        for col in 0..row.cells.len() {
            let flags = row.cells[col].flags;
            let invalid = (flags & Cell::WIDE != 0
                && (col + 1 == row.cells.len()
                    || row.cells[col + 1].flags & Cell::WIDE_SPACER == 0))
                || (flags & Cell::WIDE_SPACER != 0
                    && (col == 0 || row.cells[col - 1].flags & Cell::WIDE == 0))
                || (flags & Cell::LEADING_WIDE_SPACER != 0
                    && (!row.wrapped || col + 1 != row.cells.len()));
            if invalid {
                row.cells[col] = blank.clone();
            }
        }
    }

    pub(super) fn insert_chars(&mut self, count: usize) {
        self.pending_wrap = false;
        let col = self.cursor.col;
        let row = self.cursor.row;
        let count = count.min(self.size.cols - col);
        if count == 0 {
            return;
        }
        let blank = self.blank();
        let cols = self.size.cols;
        let active = &mut self.screen_mut().rows[row];
        if active.cells[col].flags & Cell::WIDE_SPACER != 0 {
            Self::clear_wide_at(active, col, &blank);
        }
        active.cells[col..].rotate_right(count);
        active.cells[col..col + count].fill(blank.clone());
        active.wrapped = false;
        Self::repair_wide(active, &blank);
        self.mark(row, col.saturating_sub(1), cols);
    }

    pub(super) fn delete_chars(&mut self, count: usize) {
        self.pending_wrap = false;
        let col = self.cursor.col;
        let row = self.cursor.row;
        let count = count.min(self.size.cols - col);
        if count == 0 {
            return;
        }
        let blank = self.blank();
        let cols = self.size.cols;
        let active = &mut self.screen_mut().rows[row];
        if active.cells[col].flags & Cell::WIDE_SPACER != 0 {
            Self::clear_wide_at(active, col, &blank);
        }
        active.cells[col..].rotate_left(count);
        active.cells[cols - count..].fill(blank.clone());
        active.wrapped = false;
        Self::repair_wide(active, &blank);
        self.mark(row, col.saturating_sub(1), cols);
    }

    fn scroll_region_up(&mut self, top: usize, bottom: usize, count: usize, retain_history: bool) {
        let blank = self.blank();
        let cols = self.size.cols;
        let count = count.min(bottom - top);
        let history_before = self.history_size();
        for _ in 0..count {
            let removed = self
                .screen_mut()
                .rows
                .remove(top)
                .expect("scroll region row");
            let mut recycled = if retain_history && self.history_limit != 0 {
                let recycled = if self.history.len() == self.history_limit {
                    self.history.pop_front()
                } else {
                    None
                };
                self.history.push_back(removed);
                if self.display_offset != 0 {
                    self.display_offset = (self.display_offset + 1).min(self.history.len());
                }
                recycled.unwrap_or_else(|| Row::new(cols, &blank))
            } else {
                removed
            };
            recycled.clear(cols, &blank);
            self.screen_mut().rows.insert(bottom - 1, recycled);
        }
        if count != 0 {
            self.mark_full_damage();
            self.effect(GridEffect::Scroll {
                alternate: self.alternate_active,
                top,
                bottom,
                lines: count as i64,
                retains_history: retain_history && self.history_limit != 0,
                history_before,
                history_after: self.history_size(),
            });
        }
    }

    fn scroll_region_down(&mut self, top: usize, bottom: usize, count: usize) {
        let blank = self.blank();
        let cols = self.size.cols;
        let count = count.min(bottom - top);
        let history_before = self.history_size();
        for _ in 0..count {
            let mut row = self
                .screen_mut()
                .rows
                .remove(bottom - 1)
                .expect("scroll region row");
            row.clear(cols, &blank);
            self.screen_mut().rows.insert(top, row);
        }
        if count != 0 {
            self.mark_full_damage();
            self.effect(GridEffect::Scroll {
                alternate: self.alternate_active,
                top,
                bottom,
                lines: -(count as i64),
                retains_history: false,
                history_before,
                history_after: self.history_size(),
            });
        }
    }

    pub(super) fn scroll_up(&mut self, count: usize) {
        let retain = !self.alternate_active && self.scroll_top == 0;
        self.scroll_region_up(self.scroll_top, self.scroll_bottom, count, retain);
    }
    pub(super) fn scroll_down(&mut self, count: usize) {
        self.scroll_region_down(self.scroll_top, self.scroll_bottom, count);
    }
    pub(super) fn insert_lines(&mut self, count: usize) {
        self.pending_wrap = false;
        if self.cursor.row >= self.scroll_top && self.cursor.row < self.scroll_bottom {
            self.scroll_region_down(self.cursor.row, self.scroll_bottom, count);
        }
    }
    pub(super) fn delete_lines(&mut self, count: usize) {
        self.pending_wrap = false;
        if self.cursor.row >= self.scroll_top && self.cursor.row < self.scroll_bottom {
            self.scroll_region_up(self.cursor.row, self.scroll_bottom, count, false);
        }
    }
    pub(super) fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let bottom = bottom.min(self.size.rows);
        if top >= bottom || bottom - top < 2 {
            return;
        }
        self.scroll_top = top;
        self.scroll_bottom = bottom;
        self.goto(0, 0);
    }

    pub(super) fn save_cursor(&mut self) {
        let saved = SavedCursor {
            cursor: self.cursor,
            pen: self.pen.clone(),
            pending_wrap: self.pending_wrap,
            origin_mode: self.origin_mode,
            autowrap: self.autowrap,
        };
        self.screen_mut().saved = saved;
    }

    pub(super) fn restore_cursor(&mut self) {
        let old = self.cursor;
        let saved = self.screen().saved.clone();
        self.cursor = saved.cursor;
        self.cursor.row = self.cursor.row.min(self.size.rows - 1);
        self.cursor.col = self.cursor.col.min(self.size.cols - 1);
        self.pen = saved.pen;
        self.origin_mode = saved.origin_mode;
        self.autowrap = saved.autowrap;
        self.motion_done(old);
        self.pending_wrap = saved.pending_wrap && self.cursor.col + 1 == self.size.cols;
    }

    pub(super) fn set_alternate(&mut self, enabled: bool, clear: bool, save_cursor: bool) {
        if enabled == self.alternate_active {
            return;
        }
        if enabled && save_cursor {
            self.save_cursor();
        }
        let cursor = self.cursor;
        let pending_wrap = self.pending_wrap;
        let previous = self.screen_mut();
        previous.cursor = cursor;
        previous.pending_wrap = pending_wrap;
        if enabled {
            if self.alternate.is_none() {
                self.alternate = Some(Screen::new(self.size));
            }
            if clear {
                let blank = self.blank();
                let alternate = self.alternate.as_mut().expect("allocated alternate screen");
                for row in &mut alternate.rows {
                    row.clear(self.size.cols, &blank);
                }
                alternate.cursor = Cursor {
                    shape: self.cursor.shape,
                    blinking: self.cursor.blinking,
                    visible: self.cursor.visible,
                    ..Cursor::default()
                };
                alternate.pending_wrap = false;
                self.effect(GridEffect::Clear {
                    alternate: true,
                    history_size: 0,
                });
            }
        }
        self.alternate_active = enabled;
        self.cursor = self.screen().cursor;
        self.pending_wrap = self.screen().pending_wrap;
        if !enabled && save_cursor {
            self.restore_cursor();
        }
        self.scroll_top = 0;
        self.scroll_bottom = self.size.rows;
        self.mark_full_damage();
    }

    pub(super) fn scroll_display(&mut self, delta: i32) -> bool {
        if self.alternate_active {
            return false;
        }
        let previous = self.display_offset;
        self.display_offset = if delta >= 0 {
            previous
                .saturating_add(delta as usize)
                .min(self.history.len())
        } else {
            previous.saturating_sub(delta.unsigned_abs() as usize)
        };
        if previous != self.display_offset {
            self.mark_full_damage();
            true
        } else {
            false
        }
    }

    pub(super) fn clear_scrollback(&mut self) -> bool {
        if self.alternate_active {
            return false;
        }
        self.clear_anchor = false;
        let removed = self.history.len();
        let changed = removed != 0 || self.display_offset != 0;
        self.history.clear();
        self.display_offset = 0;
        if changed {
            self.mark_full_damage();
            if removed != 0 {
                self.effect(GridEffect::ClearHistory { removed });
            }
        }
        changed
    }

    pub(super) fn set_history_limit(&mut self, limit: usize) {
        let previous = self.history.len();
        self.requested_history_limit = limit.min(MAX_HISTORY_ROWS);
        self.history_limit = Self::bounded_history(self.size, limit);
        self.trim_history();
        let removed = previous.saturating_sub(self.history.len());
        if removed != 0 {
            self.effect(GridEffect::ClearHistory { removed });
        }
        self.mark_full_damage();
    }

    fn trim_history(&mut self) {
        while self.history.len() > self.history_limit {
            self.history.pop_front();
        }
        self.display_offset = self.display_offset.min(self.history.len());
    }

    pub(super) fn reset(&mut self) {
        self.alternate_active = false;
        self.alternate = None;
        self.history.clear();
        self.display_offset = 0;
        self.cursor = Cursor::default();
        self.pen = Cell::default();
        self.primary.saved = SavedCursor {
            autowrap: true,
            ..SavedCursor::default()
        };
        self.primary.cursor = self.cursor;
        for row in &mut self.primary.rows {
            row.clear(self.size.cols, &self.pen);
        }
        self.tabs = Self::default_tabs(self.size.cols);
        self.scroll_top = 0;
        self.scroll_bottom = self.size.rows;
        self.autowrap = true;
        self.origin_mode = false;
        self.insert_mode = false;
        self.newline_mode = false;
        self.pending_wrap = false;
        self.clear_anchor = false;
        self.combining_cache.clear();
        self.mark_full_damage();
        self.effect(GridEffect::Reset);
    }

    /// Resize reconstructs logical lines only when their width changes. Normal
    /// feed, history reads, and height-only resizes never flatten the buffer.
    pub(super) fn resize(&mut self, size: Size) {
        let size = size.clamped();
        if size == self.size {
            return;
        }
        let current_cursor = self.cursor;
        let current_wrap = self.pending_wrap;
        self.screen_mut().cursor = current_cursor;
        self.screen_mut().pending_wrap = current_wrap;
        if size.cols != self.size.cols {
            self.reflow_primary(size);
            if let Some(alternate) = &mut self.alternate {
                // Alternate-screen applications repaint after SIGWINCH. Keep
                // absolute cell positions rather than manufacturing history.
                for row in &mut alternate.rows {
                    row.cells.resize(size.cols, Cell::default());
                    row.wrapped = false;
                    Self::repair_wide(row, &Cell::default());
                }
                alternate.cursor.col = alternate.cursor.col.min(size.cols - 1);
                alternate.pending_wrap = false;
            }
        }
        self.resize_height(size);
        self.size = size;
        self.history_limit = Self::bounded_history(size, self.requested_history_limit);
        self.trim_history();
        self.scroll_top = 0;
        self.scroll_bottom = size.rows;
        let old_cols = self.tabs.len();
        self.tabs.resize(size.cols, false);
        for col in old_cols..size.cols {
            self.tabs[col] = col != 0 && col % 8 == 0;
        }
        self.cursor = self.screen().cursor;
        self.cursor.row = self.cursor.row.min(size.rows - 1);
        self.cursor.col = self.cursor.col.min(size.cols - 1);
        self.pending_wrap = self.screen().pending_wrap && self.cursor.col + 1 == size.cols;
        self.dirty = vec![None; size.rows];
        self.mark_full_damage();
    }

    fn resize_height(&mut self, size: Size) {
        let history_before = self.history.len();
        // Shrink by removing bottom space first, then shift only enough rows
        // into history to keep the cursor visible.
        if self.primary.rows.len() > size.rows {
            let remove_top = self.primary.cursor.row.saturating_sub(size.rows - 1);
            for _ in 0..remove_top {
                if let Some(row) = self.primary.rows.pop_front() {
                    self.history.push_back(row);
                }
            }
            self.primary.cursor.row = self.primary.cursor.row.saturating_sub(remove_top);
            self.primary.rows.truncate(size.rows);
        } else if self.primary.rows.len() < size.rows {
            if !self.clear_anchor {
                while self.primary.rows.len() < size.rows {
                    let Some(row) = self.history.pop_back() else {
                        break;
                    };
                    self.primary.rows.push_front(row);
                    self.primary.cursor.row += 1;
                }
            }
            while self.primary.rows.len() < size.rows {
                self.primary
                    .rows
                    .push_back(Row::new(size.cols, &Cell::default()));
            }
        }
        if let Some(alternate) = &mut self.alternate {
            alternate.rows.truncate(size.rows);
            while alternate.rows.len() < size.rows {
                alternate
                    .rows
                    .push_back(Row::new(size.cols, &Cell::default()));
            }
            alternate.cursor.row = alternate.cursor.row.min(size.rows - 1);
        }
        if self.display_offset != 0 {
            self.display_offset = if self.history.len() >= history_before {
                self.display_offset
                    .saturating_add(self.history.len() - history_before)
            } else {
                self.display_offset
                    .saturating_sub(history_before - self.history.len())
            };
        }
    }

    fn reflow_primary(&mut self, size: Size) {
        let old_history = self.history.len();
        let cursor_source = old_history + self.primary.cursor.row;
        let cursor_col = self.primary.cursor.col;
        let cursor_pending = self.primary.pending_wrap;
        let viewport_source =
            (self.display_offset != 0).then_some(old_history - self.display_offset);
        // Unused bottom rows do not become logical history during reflow.
        while self.primary.rows.len() > self.primary.cursor.row + 1
            && self
                .primary
                .rows
                .back()
                .is_some_and(|row| row.content_len() == 0)
        {
            self.primary.rows.pop_back();
        }
        let source = self.history.drain(..).chain(self.primary.rows.drain(..));
        let mut output = VecDeque::new();
        let mut logical = Vec::new();
        let mut logical_cursor = None;
        let mut logical_boundary = None;
        let mut logical_viewport = None;
        let mut cursor_output = (0, 0, false);
        let mut boundary_output = 0;
        let mut viewport_output = None;
        for (index, row) in source.enumerate() {
            if index == old_history {
                logical_boundary = Some(logical.len());
            }
            if Some(index) == viewport_source {
                logical_viewport = Some(logical.len());
            }
            if index == cursor_source {
                let removed = row.cells[..cursor_col]
                    .iter()
                    .filter(|cell| cell.flags & Cell::LEADING_WIDE_SPACER != 0)
                    .count();
                logical_cursor =
                    Some(logical.len() + cursor_col - removed + usize::from(cursor_pending));
            }
            let required = if index == cursor_source {
                cursor_col + usize::from(cursor_pending)
            } else {
                0
            };
            let len = row.content_len().max(required).min(row.cells.len());
            logical.extend(
                row.cells
                    .into_iter()
                    .take(len)
                    .filter(|cell| cell.flags & Cell::LEADING_WIDE_SPACER == 0),
            );
            if !row.wrapped {
                let start = output.len();
                let result = Self::wrap_logical(
                    std::mem::take(&mut logical),
                    size.cols,
                    logical_cursor.take(),
                    logical_boundary.take(),
                    logical_viewport.take(),
                    cursor_pending,
                );
                if let Some((row, col, wrap)) = result.cursor {
                    cursor_output = (start + row, col, wrap);
                }
                if let Some(row) = result.boundary {
                    boundary_output = start + row;
                }
                if let Some(row) = result.viewport {
                    viewport_output = Some(start + row);
                }
                output.extend(result.rows);
            }
        }
        if !logical.is_empty()
            || logical_cursor.is_some()
            || logical_boundary.is_some()
            || logical_viewport.is_some()
        {
            let start = output.len();
            let result = Self::wrap_logical(
                logical,
                size.cols,
                logical_cursor,
                logical_boundary,
                logical_viewport,
                cursor_pending,
            );
            if let Some((row, col, wrap)) = result.cursor {
                cursor_output = (start + row, col, wrap);
            }
            if let Some(row) = result.boundary {
                boundary_output = start + row;
            }
            if let Some(row) = result.viewport {
                viewport_output = Some(start + row);
            }
            output.extend(result.rows);
        }
        let mut split = output.len().saturating_sub(size.rows).min(cursor_output.0);
        if self.clear_anchor {
            split = split.max(boundary_output.min(cursor_output.0));
        }
        self.primary.rows = output.split_off(split);
        self.history = output;
        self.primary.cursor.row = cursor_output.0.saturating_sub(split);
        self.primary.cursor.col = cursor_output.1;
        self.primary.pending_wrap = cursor_output.2;
        if let Some(viewport) = viewport_output {
            self.display_offset = split.saturating_sub(viewport);
        }
    }

    fn wrap_logical(
        cells: Vec<Cell>,
        cols: usize,
        cursor: Option<usize>,
        boundary: Option<usize>,
        viewport: Option<usize>,
        cursor_pending: bool,
    ) -> ReflowedLine {
        let mut rows = VecDeque::new();
        let mut row = Row::new(cols, &Cell::default());
        let mut col = 0;
        let mut cursor_result = None;
        let mut boundary_result = None;
        let mut viewport_result = None;
        let mut input = cells.into_iter().enumerate().peekable();
        while let Some((index, mut cell)) = input.next() {
            // A one-column screen must keep making progress, so wide glyphs
            // temporarily occupy one cell there. Recover their second cell
            // when the viewport grows again rather than permanently narrowing
            // the saved text.
            let expanded_wide =
                cols > 1 && cell.flags & Cell::WIDE == 0 && cell.character.width() == Some(2);
            let wide = cell.flags & Cell::WIDE != 0 || expanded_wide;
            if col == cols || (wide && cols > 1 && col + 1 == cols) {
                if col < cols {
                    row.cells[col].flags = Cell::LEADING_WIDE_SPACER;
                }
                row.wrapped = true;
                rows.push_back(row);
                row = Row::new(cols, &Cell::default());
                col = 0;
            }
            if cursor == Some(index) {
                cursor_result = Some((rows.len(), col, false));
            }
            if boundary == Some(index) {
                boundary_result = Some(rows.len());
            }
            if viewport == Some(index) {
                viewport_result = Some(rows.len());
            }
            if wide && cols == 1 {
                cell.flags &= !Cell::WIDE;
                if input
                    .peek()
                    .is_some_and(|(_, spacer)| spacer.flags & Cell::WIDE_SPACER != 0)
                {
                    let spacer_index = input.next().expect("peeked wide spacer").0;
                    if cursor == Some(spacer_index) {
                        cursor_result = Some((rows.len(), col, false));
                    }
                    if boundary == Some(spacer_index) {
                        boundary_result = Some(rows.len());
                    }
                    if viewport == Some(spacer_index) {
                        viewport_result = Some(rows.len());
                    }
                }
            }
            if expanded_wide {
                cell.flags |= Cell::WIDE;
                row.cells[col + 1] = Cell {
                    character: ' ',
                    style: cell.style,
                    flags: Cell::WIDE_SPACER,
                    extra: None,
                };
            }
            row.cells[col] = cell;
            col += if expanded_wide { 2 } else { 1 };
        }
        if cursor_result.is_none() && cursor.is_some() {
            if col == cols && cursor_pending {
                cursor_result = Some((rows.len(), cols - 1, true));
            } else if col == cols {
                row.wrapped = true;
                rows.push_back(row);
                row = Row::new(cols, &Cell::default());
                cursor_result = Some((rows.len(), 0, false));
            } else {
                cursor_result = Some((rows.len(), col, false));
            }
        }
        if boundary_result.is_none() && boundary.is_some() {
            boundary_result = Some(rows.len());
        }
        if viewport_result.is_none() && viewport.is_some() {
            viewport_result = Some(rows.len());
        }
        rows.push_back(row);
        ReflowedLine {
            rows,
            cursor: cursor_result,
            boundary: boundary_result,
            viewport: viewport_result,
        }
    }
}

#[cfg(test)]
mod tests;
