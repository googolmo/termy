//! Cold rows retain scalar/flag arrays, style runs and sparse shared metadata.
//! The live screen keeps dense cells. Borrowed public reads materialize a row
//! until the next mutation; streaming reads use caller-owned scratch instead.
use super::super::types::CellExtra;
use super::{Cell, Color, Row, Style};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug)]
struct Scalar {
    character: char,
    flags: u16,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PackedCells {
    decoded: OnceLock<Vec<Cell>>,
    cols: usize,
    background: Color,
    scalars: Vec<Scalar>,
    styles: Vec<(usize, Style)>,
    extras: Vec<(usize, Arc<CellExtra>)>,
}

impl PackedCells {
    fn encode(&mut self, cells: &[Cell], cols: usize, background: Color) -> bool {
        self.decoded.take();
        self.cols = cols;
        self.background = background;
        self.scalars.clear();
        self.styles.clear();
        self.extras.clear();
        if self.scalars.capacity() < cells.len() {
            self.scalars.reserve_exact(cells.len() - self.scalars.len());
        }
        self.scalars.extend(cells.iter().map(|cell| Scalar {
            character: cell.character,
            flags: cell.flags,
        }));
        let fixed_bytes = self.scalars.capacity() * size_of::<Scalar>();
        let budget = cols * size_of::<Cell>() / 2;
        if fixed_bytes >= budget {
            return false;
        }
        for (col, cell) in cells.iter().enumerate() {
            if self
                .styles
                .last()
                .is_none_or(|(_, style)| *style != cell.style)
            {
                if let Some((end, _)) = self.styles.last_mut() {
                    *end = col;
                }
                self.styles.push((cells.len(), cell.style));
                if fixed_bytes
                    + self.styles.capacity() * size_of::<(usize, Style)>()
                    + self.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
                    >= budget
                {
                    return false;
                }
            }
            if let Some(extra) = &cell.extra {
                self.extras.push((col, Arc::clone(extra)));
                if fixed_bytes
                    + self.styles.capacity() * size_of::<(usize, Style)>()
                    + self.extras.capacity() * size_of::<(usize, Arc<CellExtra>)>()
                    >= budget
                {
                    return false;
                }
            }
        }
        true
    }

    fn decode(&self, cells: &mut Vec<Cell>) {
        let blank = Cell {
            style: Style {
                background: self.background,
                ..Style::default()
            },
            ..Cell::default()
        };
        cells.clear();
        cells.resize(self.cols, blank);
        for (cell, scalar) in cells.iter_mut().zip(&self.scalars) {
            cell.character = scalar.character;
            cell.flags = scalar.flags;
        }
        let mut start = 0;
        for &(end, style) in &self.styles {
            for cell in &mut cells[start..end] {
                cell.style = style;
            }
            start = end;
        }
        for (col, extra) in &self.extras {
            cells[*col].extra = Some(Arc::clone(extra));
        }
    }
}

impl Row {
    pub(in crate::terminal_engine) fn with_cells<T>(
        &self,
        scratch: &mut Vec<Cell>,
        read: impl FnOnce(&[Cell]) -> T,
    ) -> T {
        if let Some(packed) = &self.packed {
            if let Some(cells) = packed.decoded.get() {
                return read(cells);
            }
            packed.decode(scratch);
            read(scratch)
        } else {
            read(&self.cells)
        }
    }

    pub(in crate::terminal_engine) fn cells(&self) -> &[Cell] {
        if let Some(packed) = &self.packed {
            packed.decoded.get_or_init(|| {
                let mut cells = Vec::new();
                packed.decode(&mut cells);
                cells
            })
        } else {
            &self.cells
        }
    }

    pub(super) fn make_dense(&mut self) {
        if let Some(mut packed) = self.packed.take() {
            if let Some(cells) = packed.decoded.take() {
                self.cells = cells;
            } else {
                packed.decode(&mut self.cells);
            }
        }
    }

    pub(super) fn release_read_cache(&mut self) {
        if let Some(packed) = &mut self.packed {
            packed.decoded.take();
        }
    }

    pub(super) fn compact(&mut self) {
        if self.packed.is_some() {
            return;
        }
        let mut packed = Box::<PackedCells>::default();
        if !packed.encode(
            &self.cells[..self.occupied],
            self.cells.len(),
            self.clear_background,
        ) {
            return;
        }
        self.cells = Vec::new();
        self.packed = Some(packed);
    }

    pub(super) fn retain(
        mut self,
        recycled: Option<Self>,
        cols: usize,
        blank: &Cell,
    ) -> (Self, Self) {
        if cols < 16 {
            return (self, recycled.unwrap_or_else(|| Self::new(cols, blank)));
        }
        let mut recycled = recycled;
        let mut packed = recycled
            .as_mut()
            .and_then(|row| row.packed.take())
            .unwrap_or_default();
        if !packed.encode(&self.cells[..self.occupied], cols, self.clear_background) {
            let mut recycled = recycled.unwrap_or_else(|| Self::new(cols, blank));
            if recycled.cells.is_empty() {
                recycled.cells.resize(cols, blank.clone());
                recycled.occupied = 0;
                recycled.clear_background = blank.style.background;
            }
            return (self, recycled);
        }
        let live = Self {
            cells: std::mem::take(&mut self.cells),
            packed: None,
            occupied: self.occupied,
            clear_background: self.clear_background,
            wrapped: false,
        };
        self.packed = Some(packed);
        (self, live)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_engine::Hyperlink;

    #[test]
    fn packed_rows_roundtrip_wide_flags_styles_and_shared_metadata() {
        let blank = Cell {
            style: Style {
                background: Color::indexed(4),
                ..Style::default()
            },
            ..Cell::default()
        };
        let mut row = Row::new(120, &blank);
        row.cells[0].character = '👩';
        row.cells[0].flags = Cell::WIDE;
        row.cells[0].extra = Some(Arc::new(CellExtra {
            combining: "🏽‍💻".into(),
            hyperlink: Some(Arc::new(Hyperlink {
                id: "id".into(),
                uri: "https://example.test".into(),
            })),
        }));
        row.cells[1].flags = Cell::WIDE_SPACER;
        row.cells[2].character = 'x';
        row.cells[2].style.foreground = Color::rgb(1, 2, 3);
        row.occupied = 3;
        row.wrapped = true;
        let expected = row.cells.clone();
        row.compact();
        assert_eq!(row.cells.capacity(), 0);
        assert!(row.packed.is_some());
        let mut scratch = Vec::new();
        row.with_cells(&mut scratch, |cells| assert_eq!(cells, expected));
        assert!(row.packed.as_ref().unwrap().decoded.get().is_none());
        assert_eq!(row.cells(), expected);
        row.release_read_cache();
        assert!(row.packed.as_ref().unwrap().decoded.get().is_none());
        row.make_dense();
        assert_eq!(row.cells, expected);
        assert!(row.wrapped);
        assert!(Arc::ptr_eq(
            row.cells[0].extra.as_ref().unwrap(),
            expected[0].extra.as_ref().unwrap()
        ));
    }

    #[test]
    fn compact_history_reuses_allocations_and_keeps_live_buffers_dense() {
        let mut grid = super::super::Grid::new(super::super::Size { cols: 120, rows: 4 }, 32);
        for _ in 0..100 {
            grid.write_ascii(b"a short log line");
            grid.carriage_return();
            grid.linefeed();
        }
        grid.compact_history(usize::MAX);
        assert!(grid.primary.rows.iter().all(|row| row.packed.is_none()));
        let mut before: Vec<_> = grid
            .history
            .iter()
            .map(|row| {
                assert!(row.cells.capacity() == 0);
                let packed = row.packed.as_ref().unwrap();
                assert!(packed.decoded.get().is_none());
                packed.scalars.as_ptr()
            })
            .collect();
        for _ in 0..100 {
            grid.write_ascii(b"a short log line");
            grid.carriage_return();
            grid.linefeed();
        }
        let mut after: Vec<_> = grid
            .history
            .iter()
            .map(|row| row.packed.as_ref().unwrap().scalars.as_ptr())
            .collect();
        before.sort();
        after.sort();
        assert_eq!(before, after);
    }

    #[test]
    fn pathological_style_changes_keep_the_bounded_dense_representation() {
        let mut row = Row::new(120, &Cell::default());
        for (col, cell) in row.cells.iter_mut().enumerate() {
            cell.style.foreground = Color::indexed(col as u8);
        }
        row.occupied = 120;
        row.compact();
        assert!(row.packed.is_none());
        assert_eq!(row.cells.len(), 120);
    }
}
