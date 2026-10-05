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
        .map(|row| row.cells.as_ptr())
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
        .map(|row| row.cells.as_ptr())
        .collect::<Vec<_>>();
    after.sort();
    assert_eq!(
        after, pointers,
        "steady scrolling must reuse evicted row buffers"
    );
    assert_eq!(grid.history_size(), 4);
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
            assert_eq!(row.cells.len(), grid.size.cols);
            for (col, cell) in row.cells.iter().enumerate() {
                if cell.flags & Cell::WIDE != 0 {
                    assert!(col + 1 < row.cells.len());
                    assert_ne!(row.cells[col + 1].flags & Cell::WIDE_SPACER, 0);
                }
                if cell.flags & Cell::WIDE_SPACER != 0 {
                    assert!(col > 0);
                    assert_ne!(row.cells[col - 1].flags & Cell::WIDE, 0);
                }
                if cell.flags & Cell::LEADING_WIDE_SPACER != 0 {
                    assert_eq!(col + 1, row.cells.len());
                    assert!(row.wrapped);
                }
            }
        }
    }
}
