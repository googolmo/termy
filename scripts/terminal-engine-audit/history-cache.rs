// Standalone audit probe; compile the identical source against both release libraries.
use std::time::Instant;
use termy_core::terminal_engine::{Engine, Options, Size};
fn run(history: usize) {
    let mut e = Engine::new(
        Size { cols: 40, rows: 24 },
        Options {
            scrollback_history: history,
        },
    );
    for _ in 0..history + 24 {
        e.feed(b"a short line\r\n");
    }
    e.compact_history();
    e.feed(&[0; 4096]);
    e.scroll_display(200);
    for _ in 0..100 {
        for r in 0..24 {
            std::hint::black_box(e.viewport_row(r));
        }
        e.feed(b"\0");
    }
    let now = Instant::now();
    for _ in 0..2000 {
        for r in 0..24 {
            std::hint::black_box(e.viewport_row(r));
        }
        e.feed(b"\0");
    }
    println!(
        "history={} iterations=2000 viewport_rows=24 elapsed_us={}",
        e.history_size(),
        now.elapsed().as_micros()
    );
}
fn main() {
    for p in 0..4 {
        println!("pair={p}");
        if p % 2 == 0 {
            run(1000);
            run(20000);
        } else {
            run(20000);
            run(1000);
        }
    }
}
