//! Reproducible parser/grid throughput and allocation measurements.
//! cargo run --release -p termy_core --example terminal_engine_bench -- 32

use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use termy_core::terminal_engine::{Cell, Engine, Options, Size};

struct Allocator;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

fn allocated(bytes: usize) {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    let live = LIVE.fetch_add(bytes as u64, Ordering::Relaxed) + bytes as u64;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: Every allocation operation is delegated to System with the original
// pointer/layout unchanged. Bookkeeping only uses atomics and cannot allocate.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            allocated(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: The caller supplies a live allocation and its original layout.
        unsafe {
            System.dealloc(ptr, layout);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller supplies the live allocation, original layout and valid new size.
        let result = unsafe { System.realloc(ptr, layout, size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size() as u64, Ordering::Relaxed);
            allocated(size);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn run(label: &str, line: &[u8], target: usize, fragmentation: usize, zero_allocations: bool) {
    let mut engine = Engine::new(
        Size {
            cols: 120,
            rows: 40,
        },
        Options {
            scrollback_history: 1000,
        },
    );
    let mut payload = Vec::with_capacity(64 * 1024 + line.len());
    while payload.len() < 64 * 1024 {
        payload.extend_from_slice(line);
    }
    // Fill the history and reserve parser storage before measuring steady state.
    for _ in 0..8 {
        engine.feed(&payload);
    }
    let before_allocs = ALLOCS.load(Ordering::Relaxed);
    let before_bytes = BYTES.load(Ordering::Relaxed);
    let retained = LIVE.load(Ordering::Relaxed);
    PEAK.store(retained, Ordering::Relaxed);
    let iterations = target.div_ceil(payload.len());
    let start = Instant::now();
    for _ in 0..iterations {
        for chunk in payload.chunks(fragmentation) {
            engine.feed(black_box(chunk));
        }
    }
    let elapsed = start.elapsed();
    let allocations = ALLOCS.load(Ordering::Relaxed) - before_allocs;
    let bytes = BYTES.load(Ordering::Relaxed) - before_bytes;
    let peak = PEAK.load(Ordering::Relaxed);
    black_box(engine.viewport_row(0));
    let mib = (iterations * payload.len()) as f64 / 1_048_576.0;
    println!(
        "{label}: {:.1} MiB/s; {allocations} allocations; {bytes} allocated bytes; {} KiB retained process heap; {} KiB peak process heap",
        mib / elapsed.as_secs_f64(),
        retained / 1024,
        peak / 1024
    );
    let settle_start = Instant::now();
    engine.compact_history();
    let settle_us = settle_start.elapsed().as_micros();
    let settled = LIVE.load(Ordering::Relaxed);
    println!(
        "  settled history: {} KiB retained process heap; {settle_us} us compaction",
        settled / 1024
    );
    if zero_allocations {
        assert_eq!(allocations, 0, "{label} should reuse warmed storage");
    }
}

fn main() {
    let mib = std::env::args().nth(1).map_or(32, |value| {
        value.parse::<usize>().expect("MiB must be an integer")
    });
    assert!((1..=4096).contains(&mib), "MiB must be in 1..=4096");
    println!(
        "terminal_engine: {} byte cells, 120x40, 1000 history rows, {mib} MiB per case",
        size_of::<Cell>()
    );
    let target = mib * 1_048_576;
    run(
        "plain scroll / 64KiB",
        b"the quick brown fox jumps over the lazy dog 0123456789\r\n",
        target,
        64 * 1024,
        true,
    );
    run("styled TUI / 64KiB", b"\x1b[H\x1b[38;2;120;180;255;48;5;234;1mstatus\x1b[0m\x1b[2;1H\x1b[Kbuild complete\x1b[3;1Hprogress: 100%", target, 64 * 1024, true);
    run(
        "Unicode / 64KiB",
        "ASCII café Ελληνικά 日本語 한글 🙂界\r\n".as_bytes(),
        target,
        64 * 1024,
        true,
    );
    // More distinct scalars than a small width cache can retain. Keep them
    // printable and non-combining so this measures width misses and grid writes,
    // not allocations for previously unseen combining suffixes.
    let mut varied = String::new();
    for scalar in 0x4e00..0x8e00 {
        varied.push(char::from_u32(scalar).unwrap());
        if scalar % 40 == 39 {
            varied.push_str("\r\n");
        }
    }
    run(
        "varied Unicode / 64KiB",
        varied.as_bytes(),
        target,
        64 * 1024,
        true,
    );
    drop(varied);
    run(
        "combining / 64KiB",
        "e\u{301} a\u{308} n\u{303} o\u{302} u\u{30a}\r\n".as_bytes(),
        target,
        64 * 1024,
        true,
    );
    run(
        "plain scroll / 1 byte",
        b"the quick brown fox jumps over the lazy dog 0123456789\r\n",
        target,
        1,
        true,
    );
}
