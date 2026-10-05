use super::*;

#[test]
fn scanner_stops_at_grouped_end_marker_and_refreshes_on_nested_begin() {
    let now = Instant::now();
    let mut update = SynchronizedUpdate::default();
    update.begin(now);
    let later = now + Duration::from_millis(20);
    let result = update.push(b"frame\x1b[?2004;2026hmore\x1b[?25;2026lafter", later);
    assert!(result.commit);
    assert_eq!(
        result.consumed,
        b"frame\x1b[?2004;2026hmore\x1b[?25;2026l".len()
    );
    assert_eq!(update.deadline(), Some(later + SYNC_TIMEOUT));
}

#[test]
fn scanner_matches_every_end_marker_split_boundary() {
    let bytes = b"text\x1b[?2004;2026lafter";
    let expected = b"text\x1b[?2004;2026l";
    for split in 0..=bytes.len() {
        let now = Instant::now();
        let mut update = SynchronizedUpdate::default();
        update.begin(now);
        let first = update.push(&bytes[..split], now);
        if !first.commit {
            let second = update.push(&bytes[split..], now);
            assert!(second.commit, "split {split}");
        }
        assert_eq!(update.take_buffer().unwrap(), expected, "split {split}");
        assert!(update.deadline().is_none());
    }
}

#[test]
fn malformed_and_cancelled_markers_do_not_end_buffering() {
    let now = Instant::now();
    let mut update = SynchronizedUpdate::default();
    update.begin(now);
    for bytes in [
        b"\x1b[2026l".as_slice(),
        b"\x1b[?2026:1l",
        b"\x1b[?2026 l",
        b"\x1b[?2026\x18l",
    ] {
        assert!(!update.push(bytes, now).commit);
    }
    assert!(update.push(b"\x1b[?2026l", now).commit);
}

#[test]
fn oversized_string_embedded_marker_is_discarded_until_st() {
    let now = Instant::now();
    let mut update = SynchronizedUpdate::default();
    update.begin(now);
    assert!(!update.push(b"\x1b_", now).commit);
    assert!(!update.push(&vec![b'x'; 64 * 1024 + 1], now).commit);
    assert!(!update.push(b"\x1b[?2026l", now).commit);
    assert!(update.push(b"\x1b\\\x1b[?2026l", now).commit);
}

#[test]
fn byte_budget_commits_at_exact_bound_and_reuses_allocation() {
    let now = Instant::now();
    let mut update = SynchronizedUpdate::default();
    update.begin(now);
    let bytes = vec![b'x'; MAX_SYNC_BYTES + 100];
    let result = update.push(&bytes, now);
    assert_eq!(result.consumed, MAX_SYNC_BYTES);
    assert!(result.commit);
    assert!(update.bytes.capacity() <= MAX_SYNC_BYTES);
    let buffer = update.take_buffer().unwrap();
    let allocation = buffer.as_ptr();
    update.recycle_buffer(buffer);
    update.begin(now);
    assert!(!update.push(b"small next frame", now).commit);
    assert_eq!(update.bytes.as_ptr(), allocation);
}

mod engine {
    use super::super::super::{Color, Damage, Engine, Event, Options, Size};
    use super::*;

    fn engine() -> Engine {
        Engine::new(
            Size { cols: 16, rows: 3 },
            Options {
                scrollback_history: 20,
            },
        )
    }

    fn text(engine: &Engine, row: usize) -> String {
        engine
            .viewport_row(row)
            .unwrap()
            .iter()
            .map(|cell| cell.character)
            .collect()
    }

    fn replies(engine: &mut Engine) -> Vec<u8> {
        let mut bytes = Vec::new();
        engine.drain_replies(&mut bytes);
        bytes
    }

    #[test]
    fn synchronized_cells_cursor_events_and_replies_wait_for_commit() {
        let mut engine = engine();
        engine.feed(b"old");
        let old_cursor = engine.cursor();
        engine.take_damage();
        engine.feed(b"\x1b[?2026h\rnew\x1b]2;staged title\x07\x1b[6n");
        assert_eq!(text(&engine, 0), "old             ");
        assert_eq!(engine.cursor(), old_cursor);
        assert!(engine.pop_event().is_none());
        assert!(replies(&mut engine).is_empty());
        assert_eq!(engine.take_damage(), Damage::Partial(Vec::new()));
        let generation = engine.generation();
        engine.feed(b"!");
        assert_eq!(engine.generation(), generation);
        engine.feed(b"\x1b[?2026l");
        assert_eq!(text(&engine, 0), "new!            ");
        assert_eq!(
            engine.pop_event(),
            Some(Event::Title("staged title".to_owned()))
        );
        assert_eq!(replies(&mut engine), b"\x1b[1;4R");
        assert!(!engine.modes().synchronized_update);
        assert!(engine.synchronized_update_deadline().is_none());
        let generation = engine.generation();
        assert!(!engine.stop_synchronized_update());
        assert_eq!(engine.generation(), generation);
    }

    #[test]
    fn visible_prefix_and_suffix_remain_on_the_correct_side_of_markers() {
        let mut engine = engine();
        engine.feed(b"prefix\x1b[?2004;2026hframe");
        assert_eq!(text(&engine, 0), "prefix          ");
        assert!(engine.modes().bracketed_paste);
        engine.feed(b"\x1b[?25;2026lafter");
        assert_eq!(text(&engine, 0), "prefixframeafter");
        assert!(!engine.cursor().visible);
    }

    #[test]
    fn every_split_preserves_synchronized_utf8_and_query_ordering() {
        let bytes = "before\x1b[?2026h\r界é\x1b[38:2::1:2:3mX\x1b[6n\x1b[?2026l!".as_bytes();
        let mut whole = engine();
        whole.feed(bytes);
        let rows: Vec<_> = (0..3)
            .map(|row| whole.viewport_row(row).unwrap().to_vec())
            .collect();
        let expected_replies = replies(&mut whole);
        for split in 0..=bytes.len() {
            let mut fragmented = engine();
            fragmented.feed(&bytes[..split]);
            fragmented.feed(&bytes[split..]);
            assert_eq!(fragmented.cursor(), whole.cursor(), "split {split}");
            for (row, expected) in rows.iter().enumerate() {
                assert_eq!(
                    fragmented.viewport_row(row).unwrap(),
                    expected,
                    "split {split}, row {row}"
                );
            }
            assert_eq!(replies(&mut fragmented), expected_replies, "split {split}");
        }
    }

    #[test]
    fn nested_begin_refreshes_timeout_without_committing_early() {
        let mut engine = engine();
        let now = Instant::now();
        engine.feed_at(b"\x1b[?2026hfirst", now);
        let old_deadline = engine.synchronized_update_deadline().unwrap();
        engine.feed_at(b"\x1b[?2026hsecond\x1b[6n", now + Duration::from_millis(80));
        let new_deadline = engine.synchronized_update_deadline().unwrap();
        assert!(new_deadline > old_deadline);
        engine.feed_at(b"", old_deadline);
        assert_eq!(text(&engine, 0), "                ");
        assert!(replies(&mut engine).is_empty());
        engine.feed_at(b"", new_deadline);
        assert_eq!(text(&engine, 0), "firstsecond     ");
        assert_eq!(replies(&mut engine), b"\x1b[1;12R");
        assert!(engine.synchronized_update_deadline().is_none());
    }

    #[test]
    fn forced_commit_preserves_incomplete_utf8_csi_and_string_state() {
        let mut engine = engine();
        engine.feed(b"\x1b[?2026h\xe2");
        assert!(engine.stop_synchronized_update());
        engine.feed(b"\x82\xac\x1b[?2026h\x1b[31");
        assert!(engine.stop_synchronized_update());
        engine.feed(b"mR\x1b[?2026h\x1b]2;partial");
        assert!(engine.stop_synchronized_update());
        assert!(engine.pop_event().is_none());
        engine.feed(b" title\x07");
        assert_eq!(text(&engine, 0), "€R              ");
        assert_eq!(
            engine.viewport_row(0).unwrap()[1].style.foreground,
            Color::indexed(1)
        );
        assert_eq!(
            engine.pop_event(),
            Some(Event::Title("partial title".to_owned()))
        );
    }

    #[test]
    fn buffer_limit_commits_without_dropping_the_following_bytes() {
        let mut engine = engine();
        engine.feed(b"\x1b[?2026h");
        let mut bytes = vec![0x7f; MAX_SYNC_BYTES];
        bytes.extend_from_slice(b"after");
        engine.feed(&bytes);
        assert_eq!(text(&engine, 0), "after           ");
        assert!(engine.synchronized_update_deadline().is_none());
        assert!(!engine.modes().synchronized_update);
    }

    #[test]
    fn starting_a_batch_keeps_existing_grid_and_history_allocations() {
        let mut engine = engine();
        engine.feed(b"one\r\ntwo\r\nthree\r\nfour");
        let row_allocation = engine.viewport_row(0).unwrap().as_ptr();
        let history_allocation = engine.line(-1).unwrap().as_ptr();
        engine.feed(b"\x1b[?2026hstaged changes\nmore changes");
        assert_eq!(engine.viewport_row(0).unwrap().as_ptr(), row_allocation);
        assert_eq!(engine.line(-1).unwrap().as_ptr(), history_allocation);
        assert_eq!(text(&engine, 0), "two             ");
    }

    #[test]
    fn resize_commits_pending_output_before_changing_geometry() {
        let mut engine = engine();
        engine.feed(b"\x1b[?2026hframe\x1b[6n");
        engine.resize(Size { cols: 20, rows: 4 });
        assert_eq!(text(&engine, 0), "frame               ");
        assert_eq!(replies(&mut engine), b"\x1b[1;6R");
        assert!(engine.synchronized_update_deadline().is_none());
    }

    #[test]
    fn full_reset_ends_synchronization_and_discards_staged_screen_content() {
        let mut engine = engine();
        engine.feed(b"old\x1b[?2026hstaged\x1bctail");
        assert_eq!(text(&engine, 0), "tail            ");
        assert!(engine.synchronized_update_deadline().is_none());
        assert!(!engine.modes().synchronized_update);
    }
}
