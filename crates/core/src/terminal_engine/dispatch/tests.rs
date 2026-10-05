use super::super::{
    Color, CursorShape, Damage, Engine, MouseEncoding, MouseTracking, Options, Size, Style,
    UnderlineStyle,
};

fn engine(cols: usize, rows: usize) -> Engine {
    Engine::new(
        Size { cols, rows },
        Options {
            scrollback_history: 10,
        },
    )
}

fn replies(engine: &mut Engine) -> Vec<u8> {
    let mut bytes = Vec::new();
    engine.drain_replies(&mut bytes);
    bytes
}

fn text(engine: &Engine, row: usize) -> String {
    engine
        .viewport_row(row)
        .unwrap()
        .iter()
        .map(|cell| cell.character)
        .collect()
}

#[test]
fn truecolor_accepts_legacy_and_colon_component_groups() {
    for rendition in [
        "38;2;1;2;3",
        "38:2:1:2:3",
        "38:2::1:2:3",
        "38:2:0:1:2:3",
        "38;2::1:2:3",
        "38;2:1:2:3",
    ] {
        let mut engine = engine(4, 2);
        engine.feed(format!("\x1b[{rendition}mX").as_bytes());
        assert_eq!(
            engine.viewport_row(0).unwrap()[0].style.foreground,
            Color::rgb(1, 2, 3),
            "{rendition}"
        );
    }
}

#[test]
fn invalid_extended_colors_preserve_style_and_do_not_leak_components() {
    for rendition in [
        "38:2::999:2:3",
        "38:2::1::3",
        "38:2::1:2",
        "38:2::1:2:3:4",
        "38:5:256",
        "38:5:3:4",
        "38;2;255:1;0;0",
        "38;2;0;;0",
        "48;5;999",
        "58:2::1:2:999",
        "38;2;0;0",
        "38;5",
    ] {
        let mut engine = engine(4, 2);
        engine.feed(b"\x1b[31;44;58:5:9;4:3mA");
        engine.feed(format!("\x1b[{rendition}mB").as_bytes());
        let row = engine.viewport_row(0).unwrap();
        assert_eq!(row[0].style, row[1].style, "{rendition}");
    }
}

#[test]
fn malformed_color_does_not_swallow_following_independent_rendition() {
    let mut engine = engine(4, 2);
    engine.feed(b"\x1b[31;38;2;999;0;0;1mX");
    let style = engine.viewport_row(0).unwrap()[0].style;
    assert_eq!(style.foreground, Color::indexed(1));
    assert_eq!(style.attributes, Style::BOLD);
}

#[test]
fn unsupported_subparameters_do_not_activate_unrelated_renditions() {
    for rendition in ["1:2", "0:2", ":2", "31:2", "4:99", "4:2:3", "24:3"] {
        let mut engine = engine(4, 2);
        engine.feed(b"\x1b[32;4:3mA");
        engine.feed(format!("\x1b[{rendition}mB").as_bytes());
        let row = engine.viewport_row(0).unwrap();
        assert_eq!(row[0].style, row[1].style, "{rendition}");
    }
}

#[test]
fn underline_subparameters_set_and_reset_known_styles() {
    let mut engine = engine(8, 2);
    engine.feed(b"\x1b[4:0mA\x1b[4:1mB\x1b[4:2mC\x1b[4:3mD\x1b[4:4mE\x1b[4:5mF\x1b[4:mG");
    let row = engine.viewport_row(0).unwrap();
    let styles: Vec<_> = row[..7].iter().map(|cell| cell.style.underline).collect();
    assert_eq!(
        styles,
        [
            UnderlineStyle::None,
            UnderlineStyle::Single,
            UnderlineStyle::Double,
            UnderlineStyle::Curly,
            UnderlineStyle::Dotted,
            UnderlineStyle::Dashed,
            UnderlineStyle::Single
        ]
    );
}

#[test]
fn sgr_reset_preserves_character_protection_for_selective_erase() {
    let mut engine = engine(5, 2);
    engine.feed(b"\x1b[1\"q\x1b[31mA\x1b[0mB\x1b[99\"qC\x1b[0\"qD\x1b[H\x1b[?2K");
    assert_eq!(text(&engine, 0), "ABC  ");
    let row = engine.viewport_row(0).unwrap();
    assert_eq!(row[0].style.foreground, Color::indexed(1));
    assert_eq!(row[1].style.foreground, Color::DEFAULT);
    assert_ne!(row[1].style.attributes & Style::PROTECTED, 0);
}

#[test]
fn origin_mode_cursor_reports_are_relative_to_scrolling_region() {
    let mut engine = engine(20, 8);
    engine.feed(b"\x1b[3;7r\x1b[?6h\x1b[2;4H\x1b[6n\x1b[?6n");
    assert_eq!((engine.cursor().row, engine.cursor().col), (3, 3));
    assert_eq!(replies(&mut engine), b"\x1b[2;4R\x1b[?2;4R");
    engine.feed(b"\x1b[?6l\x1b[4;4H\x1b[6n");
    assert_eq!(replies(&mut engine), b"\x1b[4;4R");
}

#[test]
fn explicit_zero_margins_restore_full_scrolling_region() {
    let mut engine = engine(8, 6);
    engine.feed(b"\x1b[3;4r\x1b[0;0r\x1b[?6h\x1b[6;1H\x1b[6n");
    assert_eq!(engine.cursor().row, 5);
    assert_eq!(replies(&mut engine), b"\x1b[6;1R");
}

#[test]
fn counted_tabs_stop_at_screen_edges_without_unbounded_iteration() {
    let mut engine = engine(40, 2);
    engine.feed(b"\t\x1b[2I");
    assert_eq!(engine.cursor().col, 24);
    engine.feed(b"\x1b[2Z");
    assert_eq!(engine.cursor().col, 8);
    engine.feed(b"\x1b[65535I");
    assert_eq!(engine.cursor().col, 39);
    engine.feed(b"\x1b[65535Z");
    assert_eq!(engine.cursor().col, 0);
}

#[test]
fn index_ignores_newline_mode_but_line_feed_obeys_it() {
    let mut engine = engine(10, 4);
    engine.feed(b"\x1b[20hab\x1bD");
    assert_eq!((engine.cursor().row, engine.cursor().col), (1, 2));
    engine.feed(b"\n");
    assert_eq!((engine.cursor().row, engine.cursor().col), (2, 0));
}

#[test]
fn cursor_visibility_shape_and_blinking_changes_emit_damage() {
    let mut engine = engine(10, 4);
    engine.take_damage();
    for sequence in [
        b"\x1b[?25l".as_slice(),
        b"\x1b[?25h",
        b"\x1b[5 q",
        b"\x1b[?12l",
    ] {
        engine.feed(sequence);
        assert!(
            matches!(engine.take_damage(), Damage::Partial(spans) if spans.iter().any(|span| span.row == 0 && span.start == 0 && span.end >= 1))
        );
    }
    assert_eq!(engine.cursor().shape, CursorShape::Beam);
    assert!(!engine.cursor().blinking);
    engine.feed(b"\x1b[?12l");
    assert_eq!(engine.take_damage(), Damage::Partial(Vec::new()));
}

#[test]
fn palette_mutations_invalidate_screen_but_queries_do_not() {
    let mut engine = engine(10, 4);
    engine.take_damage();
    for sequence in [
        b"\x1b]4;1;#123456\x07".as_slice(),
        b"\x1b]10;#abcdef\x07",
        b"\x1b]104;1\x07",
        b"\x1b]110\x07",
    ] {
        engine.feed(sequence);
        assert_eq!(engine.take_damage(), Damage::Full);
    }
    engine.feed(b"\x1b]4;1;?\x07\x1b]10;?\x07\x1b]4;1;invalid\x07");
    assert_eq!(engine.take_damage(), Damage::Partial(Vec::new()));
}

#[test]
fn cursor_save_restore_preserves_designated_and_active_character_sets() {
    for (save, restore) in [
        ("\x1b7", "\x1b8"),
        ("\x1b[s", "\x1b[u"),
        ("\x1b[?1048h", "\x1b[?1048l"),
    ] {
        let mut engine = engine(5, 2);
        engine.feed(format!("\x1b)0\x0e{save}\x0f\x1b)B{restore}q").as_bytes());
        assert_eq!(text(&engine, 0), "─    ");
    }
}

#[test]
fn alternate_screen_1049_restores_primary_character_set() {
    let mut engine = engine(5, 2);
    engine.feed(b"\x1b(0\x1b[?1049h\x1b(B\x1b7\x1b[?1049lq");
    assert_eq!(text(&engine, 0), "─    ");
}

#[test]
fn alternate_screen_1047_preserves_on_entry_and_clears_on_exit() {
    let mut engine = engine(10, 2);
    engine.feed(b"\x1b[?47hretained\x1b[?47l\x1b[?1047h");
    assert_eq!(text(&engine, 0), "retained  ");
    engine.feed(b"\x1b[?1047l\x1b[?47h");
    assert_eq!(text(&engine, 0), "          ");
}

#[test]
fn unrelated_mouse_mode_resets_preserve_active_tracking_and_encoding() {
    let mut engine = engine(5, 2);
    engine.feed(b"\x1b[?1003h\x1b[?1000l\x1b[?1016h\x1b[?1006l");
    assert_eq!(engine.modes().mouse_tracking, MouseTracking::Motion);
    assert_eq!(engine.modes().mouse_encoding, MouseEncoding::SgrPixels);
    engine.feed(b"\x1b[?1003l\x1b[?1016l");
    assert_eq!(engine.modes().mouse_tracking, MouseTracking::None);
    assert_eq!(engine.modes().mouse_encoding, MouseEncoding::Default);
}

#[test]
fn kitty_flags_mask_unknown_bits_instead_of_enabling_every_flag() {
    let mut engine = engine(5, 2);
    engine.feed(b"\x1b[>32u");
    assert_eq!(engine.modes().kitty_keyboard, 0);
    engine.feed(b"\x1b[>37u\x1b[=34;2u\x1b[=33;3u\x1b[?u");
    assert_eq!(replies(&mut engine), b"\x1b[?6u");
    engine.feed(b"\x1b[<0u");
    assert_eq!(engine.modes().kitty_keyboard, 6);
    engine.feed(b"\x1b[<65535u");
    assert_eq!(engine.modes().kitty_keyboard, 0);
}

#[test]
fn full_reset_clears_saved_character_sets_keyboard_flags_and_styles() {
    let mut engine = engine(10, 4);
    engine.feed(b"\x1b(0\x1b7\x1b[>7u\x1b[31m\x1b[?25l\x1b[5 q\x1bc\x1b8q");
    assert_eq!(text(&engine, 0), "q         ");
    assert_eq!(engine.modes().kitty_keyboard, 0);
    assert!(engine.cursor().visible);
    assert_eq!(engine.cursor().shape, CursorShape::Block);
    assert_eq!(engine.viewport_row(0).unwrap()[0].style, Style::default());
}
