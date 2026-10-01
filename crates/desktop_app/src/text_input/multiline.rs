use super::{INLINE_INPUT_LINE_HEIGHT_MULTIPLIER, TextInputAlignment, TextInputProvider};
use gpui_kit::{
    Bounds, ContentMask, ElementInputHandler, Entity, EntityInputHandler, Font, Hsla, IntoElement,
    PaintQuad, Pixels, ShapedLine, Styled, TextRun, UnderlineStyle, canvas, fill, point, px, size,
};

pub struct MultilineTextInputElement<V: TextInputProvider> {
    view: Entity<V>,
    focus_handle: gpui_kit::FocusHandle,
    font: Font,
    font_size: Pixels,
    text_color: Hsla,
    selection_color: Hsla,
    alignment: TextInputAlignment,
}

impl<V: TextInputProvider> MultilineTextInputElement<V> {
    pub fn new(
        view: Entity<V>,
        focus_handle: gpui_kit::FocusHandle,
        font: Font,
        font_size: Pixels,
        text_color: Hsla,
        selection_color: Hsla,
        alignment: TextInputAlignment,
    ) -> Self {
        Self {
            view,
            focus_handle,
            font,
            font_size,
            text_color,
            selection_color,
            alignment,
        }
    }
}

pub struct MultilinePrepaintState {
    lines: Vec<Option<ShapedLine>>,
    line_starts: Vec<usize>,
    line_bounds: Vec<Bounds<Pixels>>,
    line_offsets: Vec<Pixels>,
    content_bounds: Bounds<Pixels>,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

fn line_starts(lines: &[&str]) -> Vec<usize> {
    let mut offset = 0;
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let start = offset;
            offset += line.len();
            if index + 1 < lines.len() {
                offset += 1;
            }
            start
        })
        .collect()
}

fn text_runs(
    line: &str,
    line_start: usize,
    marked_range: Option<&std::ops::Range<usize>>,
    font: &Font,
    color: Hsla,
) -> Vec<TextRun> {
    let base = TextRun {
        len: line.len(),
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let Some(marked) = marked_range else {
        return vec![base];
    };
    let line_end = line_start + line.len();
    if marked.end <= line_start || marked.start >= line_end {
        return vec![base];
    }
    let start = marked.start.saturating_sub(line_start).min(line.len());
    let end = marked.end.saturating_sub(line_start).min(line.len());
    let mut runs = Vec::with_capacity(3);
    if start > 0 {
        runs.push(TextRun {
            len: start,
            ..base.clone()
        });
    }
    if end > start {
        runs.push(TextRun {
            len: end - start,
            underline: Some(UnderlineStyle {
                color: Some(color),
                thickness: px(1.0),
                wavy: false,
            }),
            ..base.clone()
        });
    }
    if end < line.len() {
        runs.push(TextRun {
            len: line.len() - end,
            ..base
        });
    }
    runs
}

impl<V: TextInputProvider + gpui_kit::Render + EntityInputHandler> IntoElement
    for MultilineTextInputElement<V>
{
    type Element = gpui_kit::Canvas<MultilinePrepaintState>;

    fn into_element(self) -> Self::Element {
        let focus_handle = self.focus_handle;
        let prepaint_focus_handle = focus_handle.clone();
        let view = self.view;
        let prepaint_view = view.clone();
        let font = self.font;
        let font_size = self.font_size;
        let text_color = self.text_color;
        let selection_color = self.selection_color;
        let alignment = self.alignment;

        canvas(
            move |bounds, window, cx| {
                let font_size_value: f32 = font_size.into();
                let line_height = px((font_size_value * INLINE_INPUT_LINE_HEIGHT_MULTIPLIER)
                    .round()
                    .max(1.0));
                let (
                    text,
                    selected_range,
                    cursor_offset,
                    marked_range,
                    focused,
                    cursor_visible,
                    cursor_width,
                    previous_offsets,
                ) = {
                    let view = prepaint_view.read(cx);
                    let focused = prepaint_focus_handle.is_focused(window);
                    let cursor_visible = view.cursor_visible();
                    let cursor_width = view.cursor_width(font_size_value);
                    view.text_input_state().map_or_else(
                        || {
                            (
                                String::new(),
                                0..0,
                                0,
                                None,
                                focused,
                                false,
                                cursor_width,
                                Vec::new(),
                            )
                        },
                        |state| {
                            (
                                state.text().to_string(),
                                state.selected_range(),
                                state.cursor_offset(),
                                state.marked_range(),
                                focused,
                                cursor_visible,
                                cursor_width,
                                state
                                    .last_line_metas
                                    .iter()
                                    .map(|(start, _, offset)| (*start, *offset))
                                    .collect(),
                            )
                        },
                    )
                };

                let raw_lines = text.split('\n').collect::<Vec<_>>();
                let starts = line_starts(&raw_lines);
                let cursor_offset = cursor_offset.min(text.len());
                let cursor_row = starts
                    .iter()
                    .rposition(|start| *start <= cursor_offset)
                    .unwrap_or(0);
                let available_height: f32 = bounds.size.height.into();
                let line_height_value: f32 = line_height.into();
                let visible_capacity =
                    (available_height / line_height_value).floor().max(1.0) as usize;
                let first_visible = cursor_row.saturating_sub(visible_capacity - 1);
                let last_visible = (first_visible + visible_capacity).min(raw_lines.len());

                let mut lines = Vec::with_capacity(last_visible - first_visible);
                let mut visible_starts = Vec::with_capacity(last_visible - first_visible);
                let mut line_bounds = Vec::with_capacity(last_visible - first_visible);
                let mut line_offsets = Vec::with_capacity(last_visible - first_visible);

                for (visible_index, row) in (first_visible..last_visible).enumerate() {
                    let line = raw_lines[row];
                    let start = starts[row];
                    let shaped = (!line.is_empty()).then(|| {
                        let runs = text_runs(line, start, marked_range.as_ref(), &font, text_color);
                        window.text_system().shape_line(
                            line.to_string().into(),
                            font_size,
                            &runs,
                            None,
                        )
                    });
                    let row_bounds = Bounds::new(
                        point(
                            bounds.left(),
                            bounds.top() + line_height * visible_index as f32,
                        ),
                        size(bounds.size.width, line_height),
                    );
                    let offset = match alignment {
                        TextInputAlignment::Left if row == cursor_row => {
                            let previous = previous_offsets
                                .iter()
                                .find_map(|(cached_start, offset)| {
                                    (*cached_start == start).then_some(*offset)
                                })
                                .unwrap_or(px(0.0));
                            let cursor_column = cursor_offset.saturating_sub(start).min(line.len());
                            let cursor_x = shaped
                                .as_ref()
                                .map_or(px(0.0), |line| line.x_for_index(cursor_column));
                            let available_width: f32 = row_bounds.size.width.into();
                            let visible_x: f32 = (cursor_x + previous).into();
                            let cursor_x_value: f32 = cursor_x.into();
                            if visible_x < 0.0 {
                                px(-(cursor_x_value - 4.0).max(0.0))
                            } else if visible_x > available_width - 4.0 {
                                px(-(cursor_x_value - available_width + 4.0))
                            } else {
                                previous
                            }
                        }
                        TextInputAlignment::Left => px(0.0),
                    };
                    lines.push(shaped);
                    visible_starts.push(start);
                    line_bounds.push(row_bounds);
                    line_offsets.push(offset);
                }

                let selection_start = selected_range.start.min(text.len());
                let selection_end = selected_range.end.min(text.len());
                let mut selection = Vec::new();
                if selection_start < selection_end {
                    for (((line, start), row_bounds), offset) in lines
                        .iter()
                        .zip(&visible_starts)
                        .zip(&line_bounds)
                        .zip(&line_offsets)
                    {
                        let line_len = line.as_ref().map_or(0, |line| line.len);
                        let local_start = selection_start.saturating_sub(*start).min(line_len);
                        let local_end = selection_end.saturating_sub(*start).min(line_len);
                        if local_start >= local_end {
                            continue;
                        }
                        let start_x = line
                            .as_ref()
                            .map_or(px(0.0), |line| line.x_for_index(local_start));
                        let end_x = line
                            .as_ref()
                            .map_or(px(0.0), |line| line.x_for_index(local_end));
                        selection.push(fill(
                            Bounds::from_corners(
                                point(row_bounds.left() + *offset + start_x, row_bounds.top()),
                                point(row_bounds.left() + *offset + end_x, row_bounds.bottom()),
                            ),
                            selection_color,
                        ));
                    }
                }

                let cursor = if focused && cursor_visible && selection_start == selection_end {
                    let visible_row = cursor_row - first_visible;
                    let row_bounds = line_bounds[visible_row];
                    let line = lines[visible_row].as_ref();
                    let column = cursor_offset
                        .saturating_sub(visible_starts[visible_row])
                        .min(line.map_or(0, |line| line.len));
                    let cursor_x = line.map_or(px(0.0), |line| line.x_for_index(column));
                    Some(fill(
                        Bounds::new(
                            point(
                                row_bounds.left() + line_offsets[visible_row] + cursor_x,
                                row_bounds.top(),
                            ),
                            size(px(cursor_width), row_bounds.size.height),
                        ),
                        text_color,
                    ))
                } else {
                    None
                };

                MultilinePrepaintState {
                    lines,
                    line_starts: visible_starts,
                    line_bounds,
                    line_offsets,
                    content_bounds: bounds,
                    selection,
                    cursor,
                }
            },
            move |bounds, mut prepaint, window, cx| {
                window.handle_input(
                    &focus_handle,
                    ElementInputHandler::new(bounds, view.clone()),
                    cx,
                );
                let painted_lines = window.with_content_mask(
                    Some(ContentMask {
                        bounds: prepaint.content_bounds,
                    }),
                    |window| {
                        for selection in prepaint.selection.drain(..) {
                            window.paint_quad(selection);
                        }
                        let mut painted = Vec::with_capacity(prepaint.lines.len());
                        for index in 0..prepaint.lines.len() {
                            let line = prepaint.lines[index].take();
                            if let Some(line) = line.as_ref() {
                                line.paint(
                                    point(
                                        prepaint.line_bounds[index].left()
                                            + prepaint.line_offsets[index],
                                        prepaint.line_bounds[index].top(),
                                    ),
                                    prepaint.line_bounds[index].size.height,
                                    gpui_kit::TextAlign::Left,
                                    None,
                                    window,
                                    cx,
                                )
                                .expect("failed to paint multiline text input text");
                            }
                            painted.push(line);
                        }
                        if let Some(cursor) = prepaint.cursor.take() {
                            window.paint_quad(cursor);
                        }
                        painted
                    },
                );
                let line_metas = prepaint
                    .line_starts
                    .iter()
                    .copied()
                    .zip(prepaint.line_bounds.iter().copied())
                    .zip(prepaint.line_offsets.iter().copied())
                    .map(|((start, bounds), offset)| (start, bounds, offset))
                    .collect();
                view.update(cx, |this, _cx| {
                    if let Some(state) = this.text_input_state_mut() {
                        state.update_multiline_layout_cache(
                            prepaint.content_bounds,
                            line_metas,
                            painted_lines,
                        );
                    }
                });
            },
        )
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_starts_include_newline_separators_and_trailing_lines() {
        let lines = "one\ntwo\n".split('\n').collect::<Vec<_>>();
        assert_eq!(line_starts(&lines), vec![0, 4, 8]);
    }
}
