//! Shape a styled run once, then anchor glyph clusters to terminal columns.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct TextBatchKey {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) strikethrough: bool,
    pub(super) fg: Hsla,
}

/// Temporary mutable builder for a text batch. Collects chars into a String,
/// then converts to the immutable `TextBatch` (with `SharedString`) on finalize.
pub(super) struct TextBatchBuilder {
    cell_offsets: Vec<(usize, usize)>,
    positioned: bool,
    start_col: usize,
    row: usize,
    text: String,
    bold: bool,
    italic: bool,
    strikethrough: bool,
    fg: Hsla,
    underline: Option<TerminalUnderline>,
    cell_len: usize,
}

impl TextBatchBuilder {
    pub(super) fn new(
        start_col: usize,
        row: usize,
        initial_char: char,
        initial_combining: Option<&str>,
        key: TextBatchKey,
        underline: Option<TerminalUnderline>,
    ) -> Self {
        let mut text = String::with_capacity(16);
        text.push(initial_char);
        if let Some(combining) = initial_combining {
            text.push_str(combining);
        }
        Self {
            cell_offsets: if initial_char.is_ascii() && initial_combining.is_none() {
                Vec::new()
            } else {
                vec![(0, 0)]
            },
            positioned: initial_combining.is_some(),
            start_col,
            row,
            text,
            bold: key.bold,
            italic: key.italic,
            strikethrough: key.strikethrough,
            fg: key.fg,
            underline,
            cell_len: 1,
        }
    }

    pub(super) fn can_append(
        &self,
        col: usize,
        row: usize,
        key: TextBatchKey,
        underline: &Option<TerminalUnderline>,
    ) -> bool {
        self.row == row
            && self.start_col + self.cell_len == col
            && self.bold == key.bold
            && self.italic == key.italic
            && self.strikethrough == key.strikethrough
            && self.fg == key.fg
            && self.underline == *underline
    }

    pub(super) fn append_cell(&mut self, c: char, combining: Option<&str>) {
        if self.cell_offsets.is_empty() && (!c.is_ascii() || combining.is_some()) {
            self.capture_ascii_offsets();
        }
        if !self.cell_offsets.is_empty() {
            self.cell_offsets.push((self.text.len(), self.cell_len));
        }
        self.positioned |= combining.is_some();
        self.text.push(c);
        if let Some(combining) = combining {
            self.text.push_str(combining);
        }
        self.cell_len += 1;
    }

    pub(super) fn ends_at(&self, col: usize) -> bool {
        self.start_col + self.cell_len == col
    }

    fn capture_ascii_offsets(&mut self) {
        self.cell_offsets.extend(
            self.text
                .char_indices()
                .enumerate()
                .map(|(col, (byte, _))| (byte, col)),
        );
    }

    pub(super) fn add_wide_spacer(&mut self) {
        if self.cell_offsets.is_empty() {
            self.capture_ascii_offsets();
        }
        self.cell_len += 1;
        self.positioned = true;
    }

    pub(super) fn finalize(self) -> TextBatch {
        TextBatch {
            cell_offsets: self.cell_offsets,
            positioned: self.positioned,
            start_col: self.start_col,
            row: self.row,
            text: SharedString::from(self.text),
            bold: self.bold,
            italic: self.italic,
            strikethrough: self.strikethrough,
            fg: self.fg,
            underline: self.underline,
            cell_len: self.cell_len,
        }
    }
}

pub(super) fn position_terminal_cells(
    mut line: ShapedLine,
    batch: &TextBatch,
    cell_width: Pixels,
) -> ShapedLine {
    // GPUI's forced width advances once per glyph, which collapses CJK and
    // emoji into one column. Copy only this cached layout before adjusting it;
    // the text system's shared layout must remain immutable.
    let len = line.len();
    line = line.with_len(len);
    let layout = Arc::get_mut(&mut line).expect("fresh terminal layout");
    position_layout(layout, batch, cell_width);
    line
}

fn position_layout(layout: &mut gpui_kit::LineLayout, batch: &TextBatch, cell_width: Pixels) {
    let mut anchors = vec![None; batch.cell_offsets.len()];
    for run in &layout.runs {
        for glyph in &run.glyphs {
            let index = batch
                .cell_offsets
                .partition_point(|(byte, _)| *byte <= glyph.index)
                .saturating_sub(1);
            let anchor = &mut anchors[index];
            anchor.get_or_insert(glyph.position.x);
        }
    }
    for run in &mut layout.runs {
        for glyph in &mut run.glyphs {
            let index = batch
                .cell_offsets
                .partition_point(|(byte, _)| *byte <= glyph.index)
                .saturating_sub(1);
            glyph.position.x += cell_width * batch.cell_offsets[index].1 as f32
                - anchors[index].expect("cluster anchor");
        }
    }
    layout.width = cell_width * batch.cell_len as f32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_and_combining_glyphs_follow_cell_columns_across_font_runs() {
        let key = TextBatchKey {
            bold: false,
            italic: false,
            strikethrough: false,
            fg: Hsla::transparent_black(),
        };
        let mut builder = TextBatchBuilder::new(0, 0, '你', None, key, None);
        builder.add_wide_spacer();
        builder.append_cell('e', Some("\u{301}"));
        builder.append_cell('x', None);
        let batch = builder.finalize();
        let glyph = |index, x| gpui_kit::ShapedGlyph {
            id: gpui_kit::GlyphId(0),
            index,
            position: point(px(x), px(0.0)),
            is_emoji: false,
        };
        let mut layout = gpui_kit::LineLayout {
            runs: vec![
                gpui_kit::ShapedRun {
                    font_id: gpui_kit::FontId(0),
                    glyphs: vec![glyph(0, 0.0)],
                },
                gpui_kit::ShapedRun {
                    font_id: gpui_kit::FontId(1),
                    glyphs: vec![glyph(3, 14.0), glyph(4, 12.0), glyph(6, 22.0)],
                },
            ],
            ..Default::default()
        };
        position_layout(&mut layout, &batch, px(10.0));
        let positions: Vec<_> = layout
            .runs
            .iter()
            .flat_map(|run| run.glyphs.iter().map(|glyph| f32::from(glyph.position.x)))
            .collect();
        assert_eq!(positions, vec![0.0, 20.0, 18.0, 30.0]);
        assert_eq!(layout.width, px(40.0));
    }

    #[test]
    fn consecutive_cjk_cells_form_one_batch_with_two_column_advances() {
        let key = TextBatchKey {
            bold: false,
            italic: false,
            strikethrough: false,
            fg: Hsla::transparent_black(),
        };
        let mut builder = TextBatchBuilder::new(0, 0, '你', None, key, None);
        builder.add_wide_spacer();
        for (index, character) in "好世界日本語".chars().enumerate() {
            assert!(builder.can_append((index + 1) * 2, 0, key, &None));
            builder.append_cell(character, None);
            builder.add_wide_spacer();
        }
        let batch = builder.finalize();
        assert_eq!(batch.text, "你好世界日本語");
        assert_eq!(batch.cell_len, 14);
    }
}
