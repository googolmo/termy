/// Physical columns for text extracted from terminal cells. Ordinary ASCII
/// needs no entries; only cells whose bytes differ from their column width do.
#[derive(Clone, Debug, Default)]
pub struct SearchLineMapping {
    cells: Vec<CellMapping>,
}

#[derive(Clone, Debug)]
struct CellMapping {
    byte_start: usize,
    byte_end: usize,
    col_start: usize,
    col_end: usize,
}

impl SearchLineMapping {
    /// Record the last cell appended to `text`, in ascending column order.
    /// Repeating a byte start replaces that cell's width, allowing a following
    /// wide spacer to supply its physical width without Unicode inference.
    pub fn record_cell(&mut self, text: &str, byte_start: usize, column: usize, width: usize) {
        if self
            .cells
            .last()
            .is_some_and(|cell| cell.byte_start == byte_start)
        {
            self.cells.pop();
        }
        let suffix = &text[byte_start..];
        if suffix.is_ascii() && suffix.len() == width {
            return;
        }
        self.cells.push(CellMapping {
            byte_start,
            byte_end: text.len(),
            col_start: column,
            col_end: column + width,
        });
    }

    pub(super) fn column(&self, byte: usize, round_up: bool) -> usize {
        let index = self.cells.partition_point(|cell| cell.byte_start <= byte);
        let Some(cell) = index.checked_sub(1).map(|index| &self.cells[index]) else {
            return byte;
        };
        if byte >= cell.byte_end {
            cell.col_end + byte - cell.byte_end
        } else if round_up && byte > cell.byte_start {
            cell.col_end
        } else {
            cell.col_start
        }
    }
}
