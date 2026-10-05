use super::*;

fn engine(cols: usize, rows: usize, history: usize) -> Engine {
    Engine::new(
        Size { cols, rows },
        Options {
            scrollback_history: history,
        },
    )
}

fn text(engine: &Engine, row: usize) -> String {
    engine
        .viewport_row(row)
        .unwrap()
        .iter()
        .filter(|cell| cell.flags & Cell::WIDE_SPACER == 0)
        .map(|cell| {
            let mut text = cell.character.to_string();
            text.push_str(cell.combining());
            text
        })
        .collect()
}

fn replies(engine: &mut Engine) -> Vec<u8> {
    let mut result = Vec::new();
    engine.drain_replies(&mut result);
    result
}

#[test]
fn clipboard_events_commit_in_stream_order_with_original_terminators() {
    let mut engine = engine(30, 3, 0);
    let input = b"\x1b[?2026h\x1b[?5522h\x1b]5522;type=read:id=a;Lg==\x07\x1b]5522;type=read:id=b;Lg==\x1b\\";
    for byte in input {
        engine.feed(&[*byte]);
    }
    assert!(engine.pop_event().is_none());
    assert!(!engine.modes().clipboard_paste_events);
    engine.feed(b"\x1b[?2026l");
    assert!(engine.modes().clipboard_paste_events);
    assert_eq!(
        engine.pop_event(),
        Some(Event::KittyClipboardControl(
            crate::KittyClipboardControl::Set(true)
        ))
    );
    for (body, terminator) in [
        (
            b"type=read:id=a;Lg==".as_slice(),
            crate::KittyClipboardOscTerminator::Bell,
        ),
        (
            b"type=read:id=b;Lg==".as_slice(),
            crate::KittyClipboardOscTerminator::StringTerminator,
        ),
    ] {
        assert_eq!(
            engine.pop_event(),
            Some(Event::KittyClipboard(crate::KittyClipboardOsc::from_body(
                body, terminator
            )))
        );
    }
    assert!(engine.pop_event().is_none());
}

#[test]
fn unicode_continuation_bytes_do_not_start_clipboard_protocols() {
    let mut engine = engine(30, 3, 0);
    for byte in "ŝ5522;ordinary".as_bytes() {
        engine.feed(&[*byte]);
    }
    assert_eq!(text(&engine, 0).trim_end(), "ŝ5522;ordinary");
    assert!(engine.pop_event().is_none());
}

#[test]
fn event_pressure_keeps_clipboard_resets_and_aborts_incomplete_writes() {
    let mut engine = engine(30, 3, 0);
    engine.feed(b"\x1b[?5522h");
    while engine.pop_event().is_some() {}
    engine.feed(&[7; MAX_EVENTS]);
    engine.feed(b"\x1b]5522;type=wdata;\x07");
    assert_eq!(
        engine.pop_event(),
        Some(Event::KittyClipboardControl(
            crate::KittyClipboardControl::Reset
        ))
    );
    assert_eq!(
        engine.pop_event(),
        Some(Event::KittyClipboardControl(
            crate::KittyClipboardControl::Set(true)
        ))
    );
    assert!(matches!(engine.pop_event(), Some(Event::KittyClipboard(_))));
    assert!(engine.pop_event().is_none());

    engine.feed(&[7; MAX_EVENTS]);
    engine.feed(b"\x1bc");
    assert_eq!(
        engine.pop_event(),
        Some(Event::KittyClipboardControl(
            crate::KittyClipboardControl::Reset
        ))
    );
    assert!(!engine.modes().clipboard_paste_events);
    assert!(engine.dropped_events() >= MAX_EVENTS as u64);
}

#[test]
fn cell_layout_and_screen_dimensions_stay_bounded() {
    assert!(
        size_of::<Cell>() <= 32,
        "hot cell grew to {} bytes",
        size_of::<Cell>()
    );
    assert_eq!(size_of::<Color>(), 4);
    assert_eq!(
        Size { cols: 0, rows: 0 }.clamped(),
        Size { cols: 1, rows: 1 }
    );
    let size = Size {
        cols: usize::MAX,
        rows: usize::MAX,
    }
    .clamped();
    assert!(size.cols * size.rows <= Size::MAX_CELLS);
}

#[test]
fn wraps_only_when_next_character_arrives() {
    let mut engine = engine(4, 2, 10);
    engine.feed(b"abcd");
    assert_eq!(text(&engine, 0), "abcd");
    assert_eq!((engine.cursor().row, engine.cursor().col), (0, 3));
    engine.feed(b"e");
    assert_eq!(text(&engine, 1), "e   ");
    assert!(engine.viewport_row_wrapped(0));
    assert_eq!(engine.history_size(), 0);
}

#[test]
fn cursor_editing_and_scroll_regions_preserve_outside_rows() {
    let mut engine = engine(8, 4, 10);
    engine.feed(b"header\r\none\r\ntwo\r\nfooter\x1b[2;3r\x1b[3;1H\nnew");
    assert_eq!(text(&engine, 0), "header  ");
    assert_eq!(text(&engine, 1), "two     ");
    assert_eq!(text(&engine, 2), "new     ");
    assert_eq!(text(&engine, 3), "footer  ");
    assert_eq!(engine.history_size(), 0);
    engine.feed(b"\x1b[r\x1b[2;2H\x1b[2@AB");
    assert_eq!(text(&engine, 1), "tABwo   ");
    engine.feed(b"\x1b[2;2H\x1b[2P");
    assert_eq!(text(&engine, 1), "two     ");
}

#[test]
fn sgr_semicolon_and_colon_forms_keep_style_and_hyperlink_independent() {
    let mut engine = engine(10, 2, 0);
    engine.feed(b"\x1b]8;id=build;https://example.com\x1b\\\x1b[1;3;4:3;38:2::1:2:3;48;5;42;58:2::4:5:6mA\x1b[0mB\x1b]8;;\x1b\\C");
    let cells = engine.viewport_row(0).unwrap();
    assert_eq!(cells[0].style.foreground.as_rgb(), Some((1, 2, 3)));
    assert_eq!(cells[0].style.background.as_indexed(), Some(42));
    assert_eq!(cells[0].style.underline_color.as_rgb(), Some((4, 5, 6)));
    assert_eq!(cells[0].style.underline, UnderlineStyle::Curly);
    assert_eq!(cells[0].style.attributes, Style::BOLD | Style::ITALIC);
    assert_eq!(cells[1].style, Style::default());
    assert_eq!(cells[1].hyperlink().unwrap().uri, "https://example.com");
    assert!(cells[2].hyperlink().is_none());
}

#[test]
fn wide_and_combining_text_survive_bytewise_input() {
    let mut engine = engine(8, 2, 0);
    for byte in "A界e\u{301}🙂".as_bytes() {
        engine.feed(&[*byte]);
    }
    let cells = engine.viewport_row(0).unwrap();
    assert_eq!(cells[1].character, '界');
    assert_ne!(cells[2].flags & Cell::WIDE_SPACER, 0);
    assert_eq!(cells[3].character, 'e');
    assert_eq!(cells[3].combining(), "\u{301}");
    assert_eq!(cells[4].character, '🙂');
    assert_ne!(cells[5].flags & Cell::WIDE_SPACER, 0);
}

#[test]
fn alternate_screen_preserves_primary_and_keyboard_stack_is_per_screen() {
    let mut engine = engine(10, 3, 10);
    engine.feed(b"shell\x1b[>3u\x1b[?1049h\x1b[>7uTUI");
    assert!(engine.alternate_screen());
    assert_eq!(engine.modes().kitty_keyboard, 7);
    assert_eq!(text(&engine, 0), "TUI       ");
    engine.feed(b"\x1b[?1049l");
    assert_eq!(engine.modes().kitty_keyboard, 3);
    assert_eq!(text(&engine, 0), "shell     ");
    assert_eq!(engine.cursor().col, 5);
    engine.feed(b"\x1b[<u\x1b[?u");
    assert_eq!(replies(&mut engine), b"\x1b[?0u");
}

#[test]
fn terminal_queries_report_cursor_and_dimensions() {
    let mut engine = engine(80, 24, 0);
    engine.feed(b"\x1b[4;9H\x1b[6n\x1b[5n\x1b[18t");
    assert_eq!(replies(&mut engine), b"\x1b[4;9R\x1b[0n\x1b[8;24;80t");
    assert!(replies(&mut engine).is_empty());
}

#[test]
fn palette_queries_and_reset_are_engine_owned() {
    let mut engine = engine(10, 2, 0);
    engine.feed(b"\x1b]4;42;#123456\x1b\\\x1b]4;42;?\x1b\\\x1b]10;rgb:ff/80/00\x07");
    assert_eq!(engine.palette()[42], Some(Color::rgb(0x12, 0x34, 0x56)));
    assert_eq!(engine.foreground(), Some(Color::rgb(255, 128, 0)));
    assert_eq!(replies(&mut engine), b"\x1b]4;42;rgb:1212/3434/5656\x1b\\");
    let revision = engine.palette_revision();
    engine.feed(b"\x1b]104;42\x1b\\\x1b]110\x1b\\");
    assert_eq!(engine.palette()[42], None);
    assert_eq!(engine.foreground(), None);
    assert!(engine.palette_revision() > revision);
}

#[test]
fn events_and_replies_have_backpressure_limits() {
    let mut engine = engine(10, 2, 0);
    engine.feed(&[7; MAX_EVENTS + 10]);
    assert_eq!(engine.dropped_events(), 10);
    let mut count = 0;
    while engine.pop_event().is_some() {
        count += 1;
    }
    assert_eq!(count, MAX_EVENTS);
    for _ in 0..20_000 {
        engine.feed(b"\x1b[5n");
    }
    let output = replies(&mut engine);
    assert!(output.len() <= MAX_REPLY_BYTES);
    assert!(engine.dropped_reply_bytes() > 0);
    assert_eq!(
        output.len() % b"\x1b[0n".len(),
        0,
        "never queue a partial reply"
    );
}

#[test]
fn all_chunk_boundaries_produce_identical_terminal_state() {
    let input = "\x1b]2;editor\x07plain\r\n界e\u{301}\x1b[38:2::7:8:9mXYZ\x1b[0m\x1b[2;3H\x1b[2@!!\x1b[6n\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\".as_bytes();
    let mut whole = engine(16, 4, 5);
    whole.feed(input);
    let expected_rows: Vec<_> = (0..4)
        .map(|row| whole.viewport_row(row).unwrap().to_vec())
        .collect();
    let expected_reply = replies(&mut whole);
    for split in 0..=input.len() {
        let mut fragmented = engine(16, 4, 5);
        fragmented.feed(&input[..split]);
        fragmented.feed(&input[split..]);
        assert_eq!(fragmented.cursor(), whole.cursor(), "split {split}");
        for (row, expected) in expected_rows.iter().enumerate() {
            assert_eq!(
                fragmented.viewport_row(row).unwrap(),
                expected,
                "split {split}, row {row}"
            );
        }
        assert_eq!(replies(&mut fragmented), expected_reply, "split {split}");
    }
}

#[test]
fn dec_line_drawing_and_character_set_switching() {
    let mut engine = engine(12, 2, 0);
    engine.feed(b"\x1b(0lqk\x1b(Babc\x1b)0\x0ex\x0fq");
    assert_eq!(text(&engine, 0), "┌─┐abc│q    ");
}

#[test]
fn damage_covers_every_changed_cell_across_screen_operations() {
    let mut engine = engine(12, 5, 20);
    let operations: &[&[u8]] = &[
        b"hello",
        b"\r\nnext",
        "界e\u{301}".as_bytes(),
        b"\x1b[1;2H!",
        b"\x1b[2@",
        b"\x1b[P",
        b"\x1b[K",
        b"\x1b[2J",
        b"first\r\nsecond\r\nthird\r\nfourth\r\nfifth\r\nsixth",
        b"\x1b[2;4r\x1b[2;1H\x1b[L",
        b"\x1b[M",
        b"\x1b[S",
        b"\x1b[T",
        b"\x1b[r",
        b"\x1b[?1049hfull screen",
        b"\x1b[?1049l",
        b"\x1b[H\x1b[3X",
    ];
    for operation in operations.iter().cycle().take(100) {
        let before: Vec<_> = (0..5)
            .map(|row| engine.viewport_row(row).unwrap().to_vec())
            .collect();
        engine.take_damage();
        engine.feed(operation);
        let damage = engine.take_damage();
        if let Damage::Partial(spans) = damage {
            for (row, cells) in before.iter().enumerate() {
                for (col, cell) in cells.iter().enumerate() {
                    if cell != &engine.viewport_row(row).unwrap()[col] {
                        assert!(
                            spans
                                .iter()
                                .any(|span| span.row == row && span.start <= col && col < span.end),
                            "missing damage at {row}:{col} for {operation:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn history_limit_updates_trim_history_and_clamp_viewport() {
    let mut engine = engine(8, 3, 20);
    for _ in 0..30 {
        engine.feed(b"line\r\n");
    }
    assert_eq!(engine.history_size(), 20);
    assert!(engine.scroll_display(20));
    engine.set_options(Options {
        scrollback_history: 2,
    });
    assert_eq!(engine.history_size(), 2);
    assert!(engine.display_offset() <= 2);
    for row in 0..engine.size().rows {
        assert!(engine.viewport_row(row).is_some());
    }
}

#[test]
fn arbitrary_bytes_and_resizes_keep_grid_invariants() {
    let mut engine = engine(13, 7, 17);
    let mut random = 0x1234_5678u32;
    for step in 0..2000 {
        let mut bytes = [0; 31];
        for byte in &mut bytes {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            *byte = random as u8;
        }
        engine.feed(&bytes);
        if step % 17 == 0 {
            engine.resize(Size {
                cols: (random as usize % 31) + 1,
                rows: (random as usize % 9) + 1,
            });
        }
        let size = engine.size();
        assert!(engine.cursor().row < size.rows);
        assert!(engine.cursor().col < size.cols);
        for row in 0..size.rows {
            let cells = engine.viewport_row(row).unwrap();
            assert_eq!(cells.len(), size.cols);
            for (col, cell) in cells.iter().enumerate() {
                if cell.flags & Cell::WIDE_SPACER != 0 {
                    assert!(col > 0 && cells[col - 1].flags & Cell::WIDE != 0);
                }
                if cell.flags & Cell::WIDE != 0 {
                    assert!(col + 1 < size.cols && cells[col + 1].flags & Cell::WIDE_SPACER != 0);
                }
            }
        }
    }
}
