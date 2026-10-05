use termy_core::{Terminal, TerminalSize, TermyFrame};

fn size(cols: u16, rows: u16) -> TerminalSize {
    TerminalSize {
        cols,
        rows,
        cell_width: 9.0,
        cell_height: 18.0,
    }
}

fn visible_text(frame: &TermyFrame) -> String {
    frame
        .cells
        .chunks(usize::from(frame.cols))
        .map(|row| row.iter().map(|cell| cell.char).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn clear_keeps_scrollback_hidden_while_resizing() {
    let mut terminal = Terminal::new_display(size(40, 10), None);
    for line in 0..18 {
        terminal
            .feed_output(format!("HISTORY-{line:02}-abcdefghijklmnopqrstuvwxyz\r\n").as_bytes());
    }

    terminal.feed_output(b"\x1b[H\x1b[2J$ ");

    let cleared = terminal.snapshot();
    assert!(cleared.history_size > 0, "clear should preserve scrollback");
    assert!(
        !visible_text(&cleared).contains("HISTORY-"),
        "clear should remove history from the live viewport"
    );

    for cols in [20, 40, 20, 40] {
        terminal.resize(size(cols, 10));
        let resized = terminal.snapshot();
        assert!(
            !visible_text(&resized).contains("HISTORY-"),
            "resizing to {cols} columns resurfaced cleared scrollback:\n{}",
            visible_text(&resized)
        );
    }

    assert!(
        terminal.scroll_display(i32::MAX),
        "preserved scrollback should remain reachable"
    );
    let scrolled = terminal.snapshot();
    assert!(
        visible_text(&scrolled).contains("HISTORY-"),
        "scrolling up should reveal preserved history"
    );
}

#[test]
fn ordinary_output_still_uses_bottom_anchored_resize() {
    let mut terminal = Terminal::new_display(size(20, 10), None);
    terminal.feed_output(b"AAAAAAAAAAAAAAAAAA\r\nBBBBBBBBBBBBBBBBBB\r\nCCCCCCCCCCCCCCCCCC\r\n$ ");

    terminal.resize(size(10, 10));

    let resized = terminal.snapshot();
    assert_eq!(resized.history_size, 0);
    let text = visible_text(&resized);
    assert!(text.contains("AAAAAAAAAA"));
    assert!(text.contains("BBBBBBBBBB"));
    assert!(text.contains("CCCCCCCCCC"));
    assert!(text.contains("$ "));
    assert_eq!(resized.cursor.unwrap().row, 6);
    assert_eq!(
        &resized.cells[..10]
            .iter()
            .map(|cell| cell.char)
            .collect::<String>(),
        "AAAAAAAAAA"
    );
}

#[test]
fn clear_preserves_the_live_boundary_during_height_growth() {
    let mut terminal = Terminal::new_display(size(20, 4), None);
    for index in 0..12 {
        terminal.feed_output(format!("HISTORY-{index}\r\n").as_bytes());
    }
    terminal.feed_output(b"\x1b[H\x1b[2J$ ");
    terminal.resize(size(20, 8));
    let frame = terminal.snapshot();
    assert!(!visible_text(&frame).contains("HISTORY"));
    assert!(frame.history_size > 0);
    assert_eq!(frame.cursor.unwrap().row, 0);
}

#[test]
fn new_output_at_the_bottom_reenables_normal_history_growth() {
    let mut terminal = Terminal::new_display(size(20, 4), None);
    for index in 0..12 {
        terminal.feed_output(format!("HISTORY-{index}\r\n").as_bytes());
    }
    terminal.feed_output(b"\x1b[H\x1b[2J$ \r\nnew one\r\nnew two\r\nnew three");
    let before = terminal.snapshot();
    terminal.resize(size(20, 6));
    let grown = terminal.snapshot();
    assert_eq!(grown.history_size, before.history_size - 2);
    assert!(visible_text(&grown).contains("HISTORY"));
    assert_eq!(grown.cursor.unwrap().row, 5);
}

#[test]
fn alternate_screen_clear_does_not_resurface_primary_history() {
    let mut terminal = Terminal::new_display(size(20, 4), None);
    for index in 0..12 {
        terminal.feed_output(format!("HISTORY-{index}\r\n").as_bytes());
    }
    terminal.feed_output(b"\x1b[H\x1b[2J$ \x1b[?1049h\x1b[2J\x1b[?1049l");
    terminal.resize(size(20, 8));
    let frame = terminal.snapshot();
    assert!(!visible_text(&frame).contains("HISTORY"));
    assert!(frame.history_size > 0);
}

#[test]
fn single_wrapped_line_uses_available_blank_rows_on_resize() {
    let mut terminal = Terminal::new_display(size(20, 10), None);
    terminal.feed_output(b"AAAAAAAAAAAAAAAAAA\r\n$ ");
    terminal.resize(size(10, 10));
    let frame = terminal.snapshot();
    assert_eq!(frame.history_size, 0);
    assert_eq!(frame.cursor.unwrap().row, 2);
    assert!(visible_text(&frame).starts_with("AAAAAAAAAA\nAAAAAAAA  \n$ "));
}
