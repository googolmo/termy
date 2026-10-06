use super::super::types::{Color, CursorShape};
use super::*;

fn grid(cols: usize, rows: usize, history: usize) -> Grid {
    Grid::new(Size { cols, rows }, history)
}

fn text(grid: &Grid, row: usize) -> String {
    grid.visible_row(row)
        .unwrap()
        .cells()
        .iter()
        .map(|cell| cell.character)
        .collect()
}

fn print(grid: &mut Grid, text: &str) {
    for character in text.chars() {
        match character {
            '\r' => grid.carriage_return(),
            '\n' => grid.linefeed(),
            character => grid.put_char(character),
        }
    }
}

#[test]
fn wrapping_is_deferred_until_the_next_printable_character() {
    let mut grid = grid(4, 2, 4);
    grid.write_ascii(b"abcd");
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 3));
    assert!(!grid.row(0).unwrap().wrapped);
    grid.put_char('e');
    assert_eq!((grid.cursor.row, grid.cursor.col), (1, 1));
    assert_eq!(text(&grid, 0), "abcd");
    assert_eq!(text(&grid, 1), "e   ");
    assert!(grid.row(0).unwrap().wrapped);
}

#[test]
fn scrolling_recycles_a_bounded_set_of_cell_buffers() {
    let mut grid = grid(12, 3, 4);
    for _ in 0..16 {
        print(&mut grid, "row\r\n");
    }
    let mut pointers = grid
        .history
        .iter()
        .chain(&grid.primary.rows)
        .map(|row| row.cells().as_ptr())
        .collect::<Vec<_>>();
    pointers.sort();
    assert_eq!(pointers.len(), 7);
    for _ in 0..1000 {
        print(&mut grid, "next\r\n");
    }
    let mut after = grid
        .history
        .iter()
        .chain(&grid.primary.rows)
        .map(|row| row.cells().as_ptr())
        .collect::<Vec<_>>();
    after.sort();
    assert_eq!(
        after, pointers,
        "steady scrolling must reuse evicted row buffers"
    );
    assert_eq!(grid.history_size(), 4);
}

#[test]
fn short_composed_lines_recycle_only_the_occupied_prefix() {
    let mut grid = grid(120, 2, 2);
    for _ in 0..32 {
        grid.write_ascii(b"e");
        grid.put_char('\u{301}');
        grid.write_ascii(b" a");
        grid.put_char('\u{308}');
        grid.carriage_return();
        grid.linefeed();
    }
    for row in grid.history.iter().chain(&grid.primary.rows) {
        assert!(row.occupied <= 3);
        assert!(row.cells()[3..].iter().all(|cell| cell == &Cell::default()));
        if row.occupied != 0 {
            assert_eq!(row.cells()[0].combining(), "\u{301}");
            assert_eq!(row.cells()[2].combining(), "\u{308}");
        }
    }
    assert_eq!(grid.primary.rows[1].occupied, 0);
    assert_eq!(grid.history_size(), 2);
}

#[test]
fn recycled_rows_clear_metadata_and_repaint_the_entire_background() {
    use std::sync::Arc;

    use crate::terminal_engine::{CellExtra, Hyperlink};

    let mut grid = grid(8, 1, 0);
    grid.pen.extra = Some(Arc::new(CellExtra {
        combining: String::new(),
        hyperlink: Some(Arc::new(Hyperlink {
            id: "retained snapshot".into(),
            uri: "https://example.test".into(),
        })),
    }));
    grid.write_ascii(b"e");
    grid.put_char('\u{301}');
    let snapshot = grid.primary.rows[0].cells[0].clone();
    grid.pen.extra = None;

    // Changing the erase background invalidates even the untouched suffix.
    for background in [Color::indexed(4), Color::indexed(4), Color::DEFAULT] {
        grid.pen.style.background = background;
        grid.carriage_return();
        grid.linefeed();
        let row = &grid.primary.rows[0];
        let blank = Cell {
            style: Style {
                background,
                ..Style::default()
            },
            ..Cell::default()
        };
        assert!(row.cells().iter().all(|cell| cell == &blank));
        assert_eq!(row.occupied, 0);
        assert!(!row.wrapped);
    }
    assert_eq!(snapshot.character, 'e');
    assert_eq!(snapshot.combining(), "\u{301}");
    assert_eq!(snapshot.hyperlink().unwrap().id, "retained snapshot");
}

#[test]
fn combining_on_blank_cells_and_selective_erases_survive_until_recycling() {
    let mut grid = grid(12, 1, 0);
    grid.pen.style.set(Style::PROTECTED, true);
    grid.put_char('p');
    grid.pen.style.set(Style::PROTECTED, false);
    grid.goto(0, 7);
    grid.put_char('\u{301}');
    assert_eq!(grid.primary.rows[0].cells[6].combining(), "\u{301}");

    grid.pen.style.background = Color::indexed(5);
    grid.goto(0, 9);
    grid.erase_line(1, true);
    assert_eq!(grid.primary.rows[0].cells[0].character, 'p');
    assert_eq!(
        grid.primary.rows[0].cells[9].style.background,
        Color::indexed(5)
    );

    grid.pen.style.background = Color::DEFAULT;
    grid.linefeed();
    assert!(
        grid.primary.rows[0]
            .cells
            .iter()
            .all(|cell| cell == &Cell::default())
    );
}

#[test]
fn viewport_remains_anchored_while_new_output_enters_history() {
    let mut grid = grid(5, 2, 10);
    print(&mut grid, "one\r\ntwo\r\nthree\r\n");
    assert!(grid.scroll_display(2));
    let before = [text(&grid, 0), text(&grid, 1)];
    print(&mut grid, "four\r\n");
    assert_eq!([text(&grid, 0), text(&grid, 1)], before);
    assert_eq!(grid.display_offset(), 3);
    assert_eq!(grid.row(-1).unwrap().cells()[0].character, 't');
}

#[test]
fn partial_scroll_regions_never_enter_history() {
    let mut grid = grid(4, 4, 10);
    print(&mut grid, "AAAA\r\nBBBB\r\nCCCC\r\nDDDD");
    grid.set_scroll_region(1, 3);
    grid.goto(2, 0);
    grid.linefeed();
    assert_eq!(text(&grid, 0), "AAAA");
    assert_eq!(text(&grid, 1), "CCCC");
    assert_eq!(text(&grid, 2), "    ");
    assert_eq!(text(&grid, 3), "DDDD");
    assert_eq!(grid.history_size(), 0);
    grid.goto(1, 0);
    grid.reverse_index();
    assert_eq!(text(&grid, 1), "    ");
    assert_eq!(text(&grid, 2), "CCCC");
}

#[test]
fn wide_character_overwrite_cleans_both_halves() {
    let mut grid = grid(6, 2, 0);
    print(&mut grid, "a界b");
    assert_eq!(grid.row(0).unwrap().cells()[1].flags, Cell::WIDE);
    assert_eq!(grid.row(0).unwrap().cells()[2].flags, Cell::WIDE_SPACER);
    grid.goto(0, 2);
    grid.put_char('x');
    assert_eq!(text(&grid, 0), "a xb  ");
    assert!(
        grid.row(0)
            .unwrap()
            .cells()
            .iter()
            .all(|cell| cell.flags == 0)
    );
    grid.goto(0, 3);
    grid.put_char('界');
    grid.goto(0, 3);
    grid.erase_chars(1);
    assert_eq!(text(&grid, 0), "a x   ");
}

#[test]
fn scalar_overwrites_repair_wide_boundaries_and_damage() {
    for (column, character, expected, start, end) in [
        (1, 'é', "aé 界 z  ", 1, 3),
        (2, 'é', "a é界 z  ", 1, 4),
        (0, '語', "語  界 z  ", 0, 3),
        (1, '語', "a語 界 z  ", 1, 4),
        (2, '語', "a 語  z  ", 1, 5),
    ] {
        let mut grid = grid(8, 2, 0);
        print(&mut grid, "a界界z");
        grid.goto(0, column);
        grid.take_damage();
        grid.put_char(character);
        assert_eq!(text(&grid, 0), expected, "{character} at {column}");
        assert_eq!(
            grid.take_damage(),
            Damage::Partial(vec![DirtySpan { row: 0, start, end }]),
            "{character} at {column}"
        );
        let cells = grid.row(0).unwrap().cells();
        for (column, cell) in cells.iter().enumerate() {
            if cell.flags & Cell::WIDE != 0 {
                assert_eq!(cells[column + 1].flags, Cell::WIDE_SPACER);
            } else if cell.flags & Cell::WIDE_SPACER != 0 {
                assert!(column > 0);
                assert_eq!(cells[column - 1].flags, Cell::WIDE);
            }
        }
    }
}

#[test]
fn wide_overwrite_preserves_styles_and_owns_only_its_base_metadata() {
    use super::super::types::{CellExtra, Hyperlink};
    use std::sync::Arc;

    let mut grid = grid(8, 2, 0);
    grid.pen.style.foreground = Color::indexed(1);
    grid.pen.style.background = Color::indexed(2);
    grid.pen.style.attributes = Style::BOLD;
    grid.pen.extra = Some(Arc::new(CellExtra {
        hyperlink: Some(Arc::new(Hyperlink {
            id: "old".into(),
            uri: "https://old.test".into(),
        })),
        ..CellExtra::default()
    }));
    print(&mut grid, "a界\u{301}界\u{308}z");
    let before = grid.row(0).unwrap().cells().to_vec();
    let extra = Arc::new(CellExtra {
        hyperlink: Some(Arc::new(Hyperlink {
            id: "new".into(),
            uri: "https://new.test".into(),
        })),
        ..CellExtra::default()
    });
    grid.pen.style.foreground = Color::indexed(3);
    grid.pen.style.background = Color::indexed(4);
    grid.pen.style.attributes = Style::ITALIC;
    grid.pen.extra = Some(Arc::clone(&extra));
    grid.goto(0, 2);
    grid.put_char('語');

    let cells = grid.row(0).unwrap().cells();
    let blank = Cell {
        style: Style {
            background: Color::indexed(4),
            ..Style::default()
        },
        ..Cell::default()
    };
    assert_eq!(cells[1], blank);
    assert_eq!(cells[4], blank);
    assert_eq!(cells[2].style, grid.pen.style);
    assert_eq!(cells[3].style, grid.pen.style);
    assert!(Arc::ptr_eq(cells[2].extra.as_ref().unwrap(), &extra));
    assert!(cells[3].extra.is_none());
    assert_eq!(Arc::strong_count(&extra), 3);
    for column in [0, 5, 6, 7] {
        assert_eq!(cells[column], before[column]);
    }
    assert_eq!(before[1].combining(), "\u{301}");
    assert_eq!(before[3].combining(), "\u{308}");
    assert_eq!(before[1].hyperlink().unwrap().id, "old");
}

#[test]
fn wide_wrap_and_combining_marks_follow_the_base_cell() {
    let mut grid = grid(4, 2, 4);
    print(&mut grid, "abc界\u{301}");
    assert_eq!(
        grid.row(0).unwrap().cells()[3].flags,
        Cell::LEADING_WIDE_SPACER
    );
    assert!(grid.row(0).unwrap().wrapped);
    assert_eq!(grid.row(1).unwrap().cells()[0].character, '界');
    assert_eq!(grid.row(1).unwrap().cells()[0].combining(), "\u{301}");
    assert_eq!(grid.row(1).unwrap().cells()[1].combining(), "");
    grid.resize(Size { cols: 6, rows: 2 });
    assert_eq!(text(&grid, 0), "abc界  ");
    assert_eq!(grid.row(0).unwrap().cells()[3].combining(), "\u{301}");
}

#[test]
fn combining_sequences_are_bounded_and_owned_by_each_cell() {
    let mut grid = grid(4, 2, 0);
    grid.put_char('e');
    for _ in 0..1000 {
        grid.put_char('\u{301}');
    }
    let cell = &grid.row(0).unwrap().cells()[0];
    assert!(cell.combining().len() <= 256);
    grid.put_char('a');
    assert_eq!(grid.row(0).unwrap().cells()[1].combining(), "");
}

#[test]
fn single_column_wide_input_always_makes_progress() {
    let mut grid = grid(1, 2, 2);
    print(&mut grid, "界界界界");
    assert_eq!(grid.history_size(), 2);
    assert_eq!(text(&grid, 0), "界");
    assert_eq!(text(&grid, 1), "界");
    grid.resize(Size { cols: 2, rows: 2 });
    assert_eq!(grid.size().cols, 2);
}

#[test]
fn wide_glyphs_recover_their_spacers_after_single_column_resize() {
    let mut grid = grid(4, 3, 8);
    print(&mut grid, "界e\u{301}");
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 3));
    grid.resize(Size { cols: 1, rows: 3 });
    grid.resize(Size { cols: 4, rows: 3 });
    let row = grid.row(0).unwrap().cells();
    assert_eq!(row[0].character, '界');
    assert_eq!(row[0].flags, Cell::WIDE);
    assert_eq!(row[1].flags, Cell::WIDE_SPACER);
    assert_eq!(row[2].character, 'e');
    assert_eq!(row[2].combining(), "\u{301}");
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 3));
}

#[test]
fn squeezed_wide_glyph_preserves_pending_wrap_and_spacer_cursor_mapping() {
    let mut grid = grid(2, 2, 4);
    grid.put_char('界');
    grid.resize(Size { cols: 1, rows: 2 });
    grid.resize(Size { cols: 2, rows: 2 });
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 1));
    assert!(grid.pending_wrap);
    grid.goto(0, 1);
    grid.resize(Size { cols: 1, rows: 2 });
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 0));
    assert!(!grid.pending_wrap);
}

#[test]
fn editing_repairs_cut_wide_glyphs_and_preserves_other_cells() {
    let mut grid = grid(8, 2, 0);
    print(&mut grid, "ab界cd");
    grid.goto(0, 3);
    grid.insert_chars(1);
    assert_eq!(text(&grid, 0), "ab   cd ");
    grid.goto(0, 2);
    grid.delete_chars(3);
    assert_eq!(text(&grid, 0), "abcd    ");
    assert!(
        grid.row(0)
            .unwrap()
            .cells()
            .iter()
            .all(|cell| cell.flags == 0)
    );
}

#[test]
fn selective_erasure_respects_protection_and_background() {
    let mut grid = grid(6, 2, 0);
    grid.pen.style.attributes = Style::PROTECTED;
    print(&mut grid, "界");
    grid.pen.style.attributes = 0;
    print(&mut grid, "abc");
    grid.pen.style.background = Color::indexed(4);
    grid.erase_line(2, true);
    assert_eq!(text(&grid, 0), "界     ");
    assert_eq!(grid.row(0).unwrap().cells()[0].flags, Cell::WIDE);
    assert_eq!(
        grid.row(0).unwrap().cells()[2].style.background,
        Color::indexed(4)
    );
    grid.erase_line(2, false);
    assert!(
        grid.row(0)
            .unwrap()
            .cells()
            .iter()
            .all(|cell| cell.flags == 0)
    );
}

#[test]
fn damage_spans_cover_cursor_motion_and_changed_glyph_halves() {
    let mut grid = grid(8, 3, 0);
    assert_eq!(grid.take_damage(), Damage::Full);
    grid.write_ascii(b"abc");
    assert_eq!(
        grid.take_damage(),
        Damage::Partial(vec![DirtySpan {
            row: 0,
            start: 0,
            end: 4
        }])
    );
    grid.goto(1, 5);
    assert_eq!(
        grid.take_damage(),
        Damage::Partial(vec![
            DirtySpan {
                row: 0,
                start: 3,
                end: 4
            },
            DirtySpan {
                row: 1,
                start: 5,
                end: 6
            }
        ])
    );
    assert_eq!(grid.take_damage(), Damage::Partial(vec![]));
}

#[test]
fn scalar_damage_tracks_visible_cursors_and_wraps_between_rows() {
    for (character, width) in [('é', 1), ('界', 2)] {
        for visible in [false, true] {
            let mut grid = grid(6, 2, 0);
            grid.cursor.visible = visible;
            grid.goto(0, 2);
            grid.take_damage();
            grid.put_char(character);
            assert_eq!(
                grid.take_damage(),
                Damage::Partial(vec![DirtySpan {
                    row: 0,
                    start: 2,
                    end: 2 + width + usize::from(visible),
                }])
            );
        }
    }

    let mut grid = grid(4, 2, 0);
    print(&mut grid, "abcd");
    grid.take_damage();
    grid.put_char('é');
    assert_eq!(
        grid.take_damage(),
        Damage::Partial(vec![
            DirtySpan {
                row: 0,
                start: 0,
                end: 4,
            },
            DirtySpan {
                row: 1,
                start: 0,
                end: 2,
            },
        ])
    );
    assert!(grid.row(0).unwrap().wrapped);
}

#[test]
fn erasing_one_column_of_a_wide_glyph_damages_both_columns() {
    let mut grid = grid(8, 2, 0);
    grid.put_char('界');
    grid.goto(0, 0);
    grid.take_damage();
    grid.erase_chars(1);
    assert_eq!(
        grid.take_damage(),
        Damage::Partial(vec![DirtySpan {
            row: 0,
            start: 0,
            end: 2
        }])
    );
    assert_eq!(text(&grid, 0), "        ");
}

#[test]
fn alternate_screen_restores_primary_and_never_retains_history() {
    let mut grid = grid(6, 3, 5);
    print(&mut grid, "hello\r\nworld");
    let primary = (0..3).map(|row| text(&grid, row)).collect::<Vec<_>>();
    let cursor = grid.cursor;
    grid.set_alternate(true, true, true);
    for _ in 0..10 {
        print(&mut grid, "alt\r\n");
    }
    assert_eq!(grid.history_size(), 0);
    assert!(!grid.scroll_display(1));
    grid.set_alternate(false, false, true);
    assert_eq!(
        (0..3).map(|row| text(&grid, row)).collect::<Vec<_>>(),
        primary
    );
    assert_eq!(grid.cursor, cursor);
}

#[test]
fn saved_cursor_restores_style_wrap_and_origin() {
    let mut grid = grid(4, 3, 0);
    grid.pen.style.foreground = Color::indexed(1);
    grid.cursor.shape = CursorShape::Beam;
    grid.write_ascii(b"abcd");
    grid.save_cursor();
    grid.goto(2, 1);
    grid.pen.style.foreground = Color::DEFAULT;
    grid.restore_cursor();
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 3));
    assert_eq!(grid.cursor.shape, CursorShape::Beam);
    assert_eq!(grid.pen.style.foreground, Color::indexed(1));
    grid.put_char('e');
    assert_eq!((grid.cursor.row, grid.cursor.col), (1, 1));
}

#[test]
fn tabs_origin_and_relative_motion_use_the_configured_region() {
    let mut grid = grid(20, 6, 0);
    grid.tab();
    assert_eq!(grid.cursor.col, 8);
    grid.clear_tab(false);
    grid.carriage_return();
    grid.tab();
    assert_eq!(grid.cursor.col, 16);
    grid.clear_tab(true);
    grid.goto(0, 3);
    grid.set_tab();
    grid.goto(0, 10);
    grid.backtab();
    assert_eq!(grid.cursor.col, 3);
    grid.set_scroll_region(2, 5);
    grid.origin_mode = true;
    grid.goto(0, 0);
    assert_eq!(grid.cursor.row, 2);
    grid.move_cursor(999, 999);
    assert_eq!((grid.cursor.row, grid.cursor.col), (4, 19));
    grid.move_cursor(-999, -999);
    assert_eq!((grid.cursor.row, grid.cursor.col), (2, 0));
}

#[test]
fn width_reflow_preserves_hard_breaks_soft_wraps_cursor_and_style() {
    let mut grid = grid(10, 6, 10);
    grid.pen.style.attributes = Style::BOLD;
    print(&mut grid, "abcdefghijklm\r\n$ ");
    grid.resize(Size { cols: 6, rows: 6 });
    assert_eq!(text(&grid, 0), "abcdef");
    assert_eq!(text(&grid, 1), "ghijkl");
    assert_eq!(text(&grid, 2), "m     ");
    assert_eq!(text(&grid, 3), "$     ");
    assert_eq!((grid.cursor.row, grid.cursor.col), (3, 2));
    assert_eq!(
        grid.row(1).unwrap().cells()[0].style.attributes,
        Style::BOLD
    );
    assert!(grid.row(0).unwrap().wrapped);
    assert!(!grid.row(2).unwrap().wrapped);
    grid.resize(Size { cols: 10, rows: 6 });
    assert_eq!(text(&grid, 0), "abcdefghij");
    assert_eq!(text(&grid, 1), "klm       ");
    assert_eq!(text(&grid, 2), "$         ");
    assert_eq!((grid.cursor.row, grid.cursor.col), (2, 2));
}

#[test]
fn cleared_scrollback_remains_hidden_across_width_reflow() {
    let mut grid = grid(20, 6, 40);
    for index in 0..18 {
        print(&mut grid, &format!("HISTORY-{index:02}-abcdefghijk\r\n"));
    }
    grid.goto(0, 0);
    grid.erase_display(2, false);
    print(&mut grid, "$ ");
    for cols in [10, 20, 10, 20] {
        grid.resize(Size { cols, rows: 6 });
        assert!((0..6).all(|row| !text(&grid, row).contains("HISTORY")));
        assert!(grid.history_size() > 0);
    }
    grid.scroll_display(i32::MAX);
    assert!((0..6).any(|row| text(&grid, row).contains("HISTORY")));
}

#[test]
fn height_resize_keeps_cursor_visible_and_recovers_history_on_growth() {
    let mut grid = grid(6, 4, 10);
    print(&mut grid, "one\r\ntwo\r\nthree\r\nfour");
    grid.resize(Size { cols: 6, rows: 2 });
    assert_eq!(
        (text(&grid, 0), text(&grid, 1)),
        ("three ".into(), "four  ".into())
    );
    assert_eq!(grid.history_size(), 2);
    assert_eq!(grid.cursor.row, 1);
    grid.resize(Size { cols: 6, rows: 4 });
    assert_eq!(text(&grid, 0), "one   ");
    assert_eq!(text(&grid, 3), "four  ");
    assert_eq!(grid.cursor.row, 3);
    assert_eq!(grid.history_size(), 0);
}

#[test]
fn scrolled_viewport_tracks_its_logical_anchor_through_reflow() {
    let mut grid = grid(12, 3, 40);
    for index in 0..12 {
        print(&mut grid, &format!("row-{index:02}-abcdefgh\r\n"));
    }
    grid.scroll_display(i32::MAX);
    let prefix = text(&grid, 0)[..6].to_owned();
    grid.resize(Size { cols: 6, rows: 3 });
    assert_eq!(text(&grid, 0), prefix);
    grid.resize(Size { cols: 12, rows: 3 });
    assert!(text(&grid, 0).starts_with(&prefix));
    let before = text(&grid, 0);
    grid.resize(Size { cols: 12, rows: 2 });
    assert_eq!(text(&grid, 0), before);
}

#[test]
fn limits_bound_dimensions_and_scrollback_storage() {
    let grid = grid(usize::MAX, usize::MAX, usize::MAX);
    assert!(grid.size.cols * grid.size.rows <= Size::MAX_CELLS);
    assert!(grid.history_limit * grid.size.cols <= Size::MAX_CELLS);
    assert!(grid.history_limit <= MAX_HISTORY_ROWS);
}

#[test]
fn mixed_edit_resize_and_scroll_sequences_preserve_grid_invariants() {
    let mut grid = grid(13, 5, 11);
    let mut seed = 0xd9a3_4871_u32;
    for _ in 0..3000 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let count = (seed >> 8) as usize % 8 + 1;
        grid.pen.style.background = if seed & 0x80 != 0 {
            Color::indexed((seed >> 16) as u8)
        } else {
            Color::DEFAULT
        };
        match seed % 15 {
            0 => grid.put_char('界'),
            1 => grid.put_char('\u{301}'),
            2 => grid.write_ascii(b"abcdef"),
            3 => grid.insert_chars(count),
            4 => grid.delete_chars(count),
            5 => grid.erase_chars(count),
            6 => grid.linefeed(),
            7 => grid.reverse_index(),
            8 => grid.goto(count % grid.size.rows, count % grid.size.cols),
            9 => grid.insert_lines(count),
            10 => grid.delete_lines(count),
            11 => grid.resize(Size {
                cols: count + 1,
                rows: 3 + count % 4,
            }),
            12 => {
                grid.scroll_display(count as i32 - 4);
            }
            13 => grid.set_alternate(!grid.alternate_active, true, true),
            _ => grid.erase_display(2, false),
        }
        assert!(grid.cursor.col < grid.size.cols);
        assert!(grid.cursor.row < grid.size.rows);
        assert_eq!(grid.screen().rows.len(), grid.size.rows);
        assert!(grid.history.len() <= grid.history_limit);
        for row in grid
            .history
            .iter()
            .chain(&grid.primary.rows)
            .chain(grid.alternate.iter().flat_map(|screen| &screen.rows))
        {
            assert_eq!(row.cells().len(), grid.size.cols);
            assert!(row.occupied <= row.cells().len());
            let blank = Cell {
                style: Style {
                    background: row.clear_background,
                    ..Style::default()
                },
                ..Cell::default()
            };
            assert!(
                row.cells()[row.occupied..]
                    .iter()
                    .all(|cell| cell == &blank)
            );
            for (col, cell) in row.cells().iter().enumerate() {
                if cell.flags & Cell::WIDE != 0 {
                    assert!(col + 1 < row.cells().len());
                    assert_ne!(row.cells()[col + 1].flags & Cell::WIDE_SPACER, 0);
                }
                if cell.flags & Cell::WIDE_SPACER != 0 {
                    assert!(col > 0);
                    assert_ne!(row.cells()[col - 1].flags & Cell::WIDE, 0);
                }
                if cell.flags & Cell::LEADING_WIDE_SPACER != 0 {
                    assert_eq!(col + 1, row.cells().len());
                    assert!(row.wrapped);
                }
            }
        }
    }
}

#[test]
fn effects_are_dormant_without_graphics_and_coalesce_contiguous_scrolls() {
    let mut grid = grid(8, 3, 4);
    for _ in 0..20 {
        print(&mut grid, "row\r\n");
    }
    assert!(grid.effects.is_empty());
    assert_eq!(grid.effects.capacity(), 0);
    grid.set_effect_tracking(true);
    for _ in 0..1000 {
        print(&mut grid, "row\r\n");
    }
    assert_eq!(
        grid.effects,
        vec![GridEffect::Scroll {
            alternate: false,
            top: 0,
            bottom: 3,
            lines: 1000,
            retains_history: true,
            history_before: 4,
            history_after: 4
        }]
    );
    let capacity = grid.effects.capacity();
    let mut output = Vec::new();
    grid.drain_effects(&mut output);
    assert_eq!(output.len(), 1);
    assert_eq!(grid.effects.capacity(), capacity);
    assert!(grid.effects.is_empty());
}

#[test]
fn effect_order_preserves_opposing_scrolls_partial_regions_and_clears() {
    let mut grid = grid(8, 5, 10);
    grid.set_effect_tracking(true);
    grid.scroll_up(2);
    grid.scroll_up(1);
    grid.scroll_down(1);
    grid.set_scroll_region(1, 4);
    grid.goto(2, 0);
    grid.delete_lines(1);
    grid.insert_lines(2);
    grid.erase_display(2, false);
    grid.clear_scrollback();
    grid.set_alternate(true, true, true);
    grid.scroll_up(1);
    grid.reset();
    assert_eq!(
        grid.effects,
        vec![
            GridEffect::Scroll {
                alternate: false,
                top: 0,
                bottom: 5,
                lines: 3,
                retains_history: true,
                history_before: 0,
                history_after: 3
            },
            GridEffect::Scroll {
                alternate: false,
                top: 0,
                bottom: 5,
                lines: -1,
                retains_history: false,
                history_before: 3,
                history_after: 3
            },
            GridEffect::Scroll {
                alternate: false,
                top: 2,
                bottom: 4,
                lines: 1,
                retains_history: false,
                history_before: 3,
                history_after: 3
            },
            GridEffect::Scroll {
                alternate: false,
                top: 2,
                bottom: 4,
                lines: -2,
                retains_history: false,
                history_before: 3,
                history_after: 3
            },
            GridEffect::Clear {
                alternate: false,
                history_size: 3
            },
            GridEffect::ClearHistory { removed: 3 },
            GridEffect::Clear {
                alternate: true,
                history_size: 0
            },
            GridEffect::Scroll {
                alternate: true,
                top: 0,
                bottom: 5,
                lines: 1,
                retains_history: false,
                history_before: 0,
                history_after: 0
            },
            GridEffect::Reset,
        ]
    );
    assert!(
        grid.track_effects,
        "reset must still report future graphics changes"
    );
    grid.set_effect_tracking(false);
    grid.scroll_up(1);
    assert!(grid.effects.is_empty());
}

#[test]
fn saturated_history_scroll_effects_distinguish_line_deletion() {
    let mut grid = grid(8, 3, 2);
    grid.scroll_up(2);
    grid.set_effect_tracking(true);
    grid.scroll_up(1);
    grid.goto(0, 0);
    grid.delete_lines(1);
    assert_eq!(
        grid.effects,
        vec![
            GridEffect::Scroll {
                alternate: false,
                top: 0,
                bottom: 3,
                lines: 1,
                retains_history: true,
                history_before: 2,
                history_after: 2
            },
            GridEffect::Scroll {
                alternate: false,
                top: 0,
                bottom: 3,
                lines: 1,
                retains_history: false,
                history_before: 2,
                history_after: 2
            },
        ]
    );
}

#[test]
fn history_clear_and_reset_release_resize_suppression() {
    let mut grid = grid(8, 3, 8);
    grid.erase_display(2, false);
    assert!(grid.clear_anchor);
    grid.erase_display(3, false);
    assert!(!grid.clear_anchor);
    grid.erase_display(2, false);
    grid.reset();
    assert!(!grid.clear_anchor);
}

#[test]
fn invalid_scroll_margins_preserve_the_previous_region_and_cursor() {
    let mut grid = grid(8, 6, 8);
    grid.set_scroll_region(1, 5);
    grid.goto(3, 4);
    grid.set_scroll_region(5, 1);
    grid.set_scroll_region(3, 4);
    grid.set_scroll_region(100, 200);
    assert_eq!(grid.scroll_region(), (1, 5));
    assert_eq!((grid.cursor.row, grid.cursor.col), (3, 4));
}

#[test]
fn inserting_and_deleting_lines_preserve_the_cursor_column() {
    let mut grid = grid(8, 5, 8);
    print(&mut grid, "first\r\nsecond\r\nthird");
    grid.goto(1, 4);
    grid.insert_lines(1);
    assert_eq!((grid.cursor.row, grid.cursor.col), (1, 4));
    assert_eq!(text(&grid, 1), "        ");
    grid.delete_lines(1);
    assert_eq!((grid.cursor.row, grid.cursor.col), (1, 4));
    assert_eq!(text(&grid, 1), "second  ");
}

fn rendered_rows(grid: &Grid) -> Vec<Vec<(Cell, bool)>> {
    (0..grid.size.rows)
        .map(|row| {
            let row = grid.visible_row(row).unwrap();
            row.cells()
                .iter()
                .cloned()
                .enumerate()
                .map(|(col, cell)| (cell, row.wrapped && col + 1 == grid.size.cols))
                .collect()
        })
        .collect()
}

fn replay_damage(grid: &mut Grid, cached: &mut Vec<Vec<(Cell, bool)>>) -> usize {
    let (damage, scrolls) = grid.take_render_damage();
    let actual = rendered_rows(grid);
    if damage == Damage::Full {
        assert!(scrolls.is_empty());
        *cached = actual;
        return 0;
    }
    for scroll in &scrolls {
        let rows = &mut cached[scroll.top..scroll.bottom];
        if scroll.lines > 0 {
            rows.rotate_left(scroll.lines as usize);
        } else {
            rows.rotate_right(scroll.lines.unsigned_abs() as usize);
        }
    }
    let Damage::Partial(spans) = damage else {
        unreachable!();
    };
    for span in spans {
        cached[span.row][span.start..span.end]
            .clone_from_slice(&actual[span.row][span.start..span.end]);
    }
    assert_eq!(*cached, actual);
    scrolls.len()
}

#[test]
fn ordered_scroll_damage_replays_edits_before_between_and_after_scrolls() {
    let mut grid = grid(8, 5, 10);
    print(&mut grid, "one\r\ntwo\r\nthree\r\nfour\r\nfive");
    let mut cached = Vec::new();
    replay_damage(&mut grid, &mut cached);
    grid.goto(2, 1);
    print(&mut grid, "界");
    grid.scroll_up(1);
    grid.goto(4, 0);
    print(&mut grid, "new");
    grid.set_scroll_region(1, 4);
    grid.scroll_down(1);
    grid.goto(1, 3);
    print(&mut grid, "z\u{301}");
    grid.scroll_up(2);
    grid.goto(2, 2);
    grid.insert_lines(1);
    assert_eq!(replay_damage(&mut grid, &mut cached), 4);
    assert_eq!(grid.take_render_damage(), (Damage::Partial(vec![]), vec![]));
}

#[test]
fn scroll_damage_coalesces_and_falls_back_at_a_bounded_record_count() {
    let mut grid = grid(8, 5, 0);
    grid.take_damage();
    for _ in 0..100 {
        grid.scroll_up(1);
    }
    let (damage, scrolls) = grid.take_render_damage();
    assert_ne!(damage, Damage::Full);
    assert_eq!(
        scrolls,
        vec![ViewportScroll {
            top: 0,
            bottom: 5,
            lines: 5
        }]
    );
    for _ in 0..MAX_SCROLL_DAMAGE {
        grid.scroll_up(1);
        grid.scroll_down(1);
    }
    assert_eq!(grid.take_render_damage(), (Damage::Full, vec![]));
    assert_eq!(grid.pending_scrolls.capacity(), MAX_SCROLL_DAMAGE);
}

#[test]
fn cell_only_damage_covers_scrolls_and_full_invalidations_discard_them() {
    let mut grid = grid(8, 5, 10);
    print(&mut grid, "one\r\ntwo\r\nthree\r\nfour\r\nfive");
    grid.take_damage();
    grid.scroll_up(1);
    let Damage::Partial(spans) = grid.take_damage() else {
        panic!("expected spans")
    };
    assert_eq!(
        spans,
        (0..5)
            .map(|row| DirtySpan {
                row,
                start: 0,
                end: 8
            })
            .collect::<Vec<_>>()
    );
    grid.scroll_up(1);
    grid.set_alternate(true, true, true);
    assert_eq!(grid.take_render_damage(), (Damage::Full, vec![]));
    grid.set_alternate(false, false, true);
    grid.take_damage();
    assert!(grid.scroll_display(1));
    grid.take_damage();
    grid.scroll_up(1);
    assert_eq!(grid.take_render_damage(), (Damage::Full, vec![]));
}

#[test]
fn randomized_scroll_damage_reconstructs_cell_and_wrap_metadata() {
    let mut grid = grid(9, 6, 12);
    let mut cached = Vec::new();
    replay_damage(&mut grid, &mut cached);
    let mut random = 0xabcddcba12344321_u64;
    let mut scrolls = 0;
    for _ in 0..500 {
        for _ in 0..7 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = (random >> 32) as usize;
            match value % 12 {
                0 => grid.goto(value / 12 % grid.size.rows, value / 100 % grid.size.cols),
                1 => grid.scroll_up(1 + value % 3),
                2 => grid.scroll_down(1 + value % 3),
                3 => print(&mut grid, "A界e\u{301}"),
                4 => grid.insert_lines(1 + value % 2),
                5 => grid.delete_lines(1 + value % 2),
                6 => grid.linefeed(),
                7 => grid.set_scroll_region(value / 12 % 3, 4 + value % 3),
                8 => grid.erase_chars(1 + value % 3),
                9 => grid.carriage_return(),
                10 => grid.delete_chars(1 + value % 3),
                _ => print(&mut grid, "123456789012345"),
            }
        }
        scrolls += replay_damage(&mut grid, &mut cached);
    }
    assert!(scrolls > 100);
}

#[test]
fn widening_history_bounds_intermediate_rows_and_clamps_evicted_viewport_anchor() {
    let mut grid = grid(1, 3, 2048);
    for index in 0..1024 {
        grid.put_char(char::from(b'a' + (index % 26) as u8));
        grid.carriage_return();
        grid.linefeed();
    }
    assert_eq!(grid.history_size(), 1022);
    assert!(grid.scroll_display(400));
    grid.resize(Size {
        cols: 4096,
        rows: 3,
    });
    let limit = Size::MAX_CELLS / 4096;
    assert_eq!(grid.history_size(), limit);
    assert_eq!(grid.display_offset(), limit);
    assert!(grid.last_reflow_row_allocations <= limit + 3 + 1);
    assert_eq!(
        grid.row(-1).unwrap().cells()[0].character,
        char::from(b'a' + (1021 % 26) as u8)
    );
    assert_eq!(
        grid.visible_row(0).unwrap().cells()[0].character,
        char::from(b'a' + (766 % 26) as u8)
    );
    assert_eq!((grid.cursor.row, grid.cursor.col), (2, 0));
}

#[test]
fn narrowing_one_long_logical_line_recycles_rows_and_preserves_its_tail() {
    let mut grid = grid(80, 3, 1000);
    let pattern = "abcdefghijklmnopqrstuvwxyz";
    let input = pattern.repeat(3000);
    grid.write_ascii(input.as_bytes());
    grid.resize(Size { cols: 1, rows: 3 });
    assert_eq!(grid.history_size(), 1000);
    assert!(grid.last_reflow_row_allocations <= 1000 + 3 + 1);
    assert_eq!(grid.primary.rows.len(), 3);
    assert_eq!(text(&grid, 0), "x");
    assert_eq!(text(&grid, 1), "y");
    assert_eq!(text(&grid, 2), "z");
    assert_eq!((grid.cursor.row, grid.cursor.col), (2, 0));
    assert!(grid.pending_wrap);
    assert_eq!(grid.row(-1).unwrap().cells()[0].character, 'w');
}

#[test]
fn reflow_drops_below_cursor_rows_without_losing_the_cursor_window() {
    let mut grid = grid(80, 4, 5);
    grid.write_ascii(&vec![b'x'; 320]);
    grid.goto(0, 1);
    grid.resize(Size { cols: 1, rows: 4 });
    assert_eq!(grid.history_size(), 1);
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 0));
    assert!(grid.last_reflow_row_allocations <= 5 + 4 + 1);
    assert!((0..4).all(|row| text(&grid, row) == "x"));
}

#[test]
fn alternate_width_growth_and_height_shrink_preserve_only_surviving_rows() {
    let mut grid = grid(1, 4096, 0);
    grid.set_alternate(true, true, true);
    grid.put_char('a');
    grid.goto(4095, 0);
    grid.put_char('z');
    grid.resize(Size {
        cols: 4096,
        rows: 1,
    });
    assert!(grid.alternate_active);
    assert_eq!(grid.screen().rows.len(), 1);
    assert_eq!(grid.screen().rows[0].cells.len(), 4096);
    assert_eq!(grid.screen().rows[0].cells[0].character, 'a');
    assert_eq!((grid.cursor.row, grid.cursor.col), (0, 0));
    assert!(!grid.pending_wrap);
    grid.set_alternate(false, false, true);
    assert_eq!(grid.primary.rows.len(), 1);
    assert_eq!(grid.primary.rows[0].cells.len(), 4096);
}

fn assert_history_compacted(grid: &mut Grid) {
    while grid.needs_compaction() {
        grid.compact_history(256);
    }
    let dense = grid
        .history
        .iter()
        .filter(|row| row.packed.is_none())
        .count();
    assert_eq!(
        dense,
        0,
        "{dense} of {} history rows stayed dense",
        grid.history.len()
    );
}

#[test]
fn height_shrink_keeps_older_history_in_the_compaction_window() {
    let mut grid = grid(120, 10, 100);
    for line in 0..30 {
        print(&mut grid, &format!("line {line}\r\n"));
    }
    grid.resize(Size { cols: 120, rows: 4 });
    assert!(grid.history_activity);
    assert_history_compacted(&mut grid);
    grid.resize(Size { cols: 120, rows: 8 });
    grid.resize(Size { cols: 120, rows: 3 });
    assert_history_compacted(&mut grid);
}

#[test]
fn width_resize_defers_history_compaction_to_idle_steps() {
    let mut grid = grid(120, 4, 100);
    for line in 0..60 {
        print(&mut grid, &format!("line {line}\r\n"));
    }
    assert_history_compacted(&mut grid);
    grid.history_activity = false;
    grid.resize(Size { cols: 100, rows: 4 });
    assert!(grid.history.iter().all(|row| row.packed.is_none()));
    assert!(grid.history_activity);
    assert!(grid.needs_compaction());
    assert_history_compacted(&mut grid);
}

#[test]
fn forced_compaction_does_not_pack_rows_as_they_scroll() {
    let mut grid = grid(120, 4, 100);
    for line in 0..20 {
        print(&mut grid, &format!("line {line}\r\n"));
    }
    while grid.needs_compaction() {
        grid.compact_pending_history(256);
    }
    assert!(grid.history.iter().all(|row| row.packed.is_some()));
    print(&mut grid, "next\r\n");
    assert!(grid.history.back().unwrap().packed.is_none());
    grid.compact_history(256);
    print(&mut grid, "quiet\r\n");
    assert!(grid.history.back().unwrap().packed.is_some());
}
