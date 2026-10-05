//! Isolate the graphics interception work performed for every native PTY read.
//! The unchanged owned API is the before path; the runtime now uses the borrowed
//! API. This measures interception CPU and transient allocations, not app RSS.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

use termy_core::KittyGraphicsInterceptor;

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

fn allocated(size: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: every allocation operation is forwarded to the system allocator with
// its original pointer/layout. Counters only observe successful allocations.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the allocator's required valid layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && COUNTING.load(Ordering::Relaxed) {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the allocator's required valid layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && COUNTING.load(Ordering::Relaxed) {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if COUNTING.load(Ordering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: pointer/layout are the original system allocation pair.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: pointer/layout identify a live system allocation; size is the
        // new allocation size supplied by the caller.
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() && COUNTING.load(Ordering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
            allocated(size);
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn consume(parser: &mut KittyGraphicsInterceptor, input: &[u8], borrowed: bool) {
    if borrowed {
        for item in parser.process_borrowed(black_box(input)) {
            black_box(item);
        }
    } else {
        for item in parser.process(black_box(input)) {
            black_box(item);
        }
    }
}

fn measure(label: &str, chunk: &[u8], borrowed: bool) {
    const SAMPLES: usize = 7;
    const TARGET_BYTES: usize = 128 * 1024 * 1024;
    const ALLOCATION_ITERATIONS: usize = 1000;
    let iterations = TARGET_BYTES.div_ceil(chunk.len());
    let mut elapsed = [0.0f64; SAMPLES];
    for sample in &mut elapsed {
        let mut parser = KittyGraphicsInterceptor::default();
        for _ in 0..100 {
            consume(&mut parser, chunk, borrowed);
        }
        let start = Instant::now();
        for _ in 0..iterations {
            consume(&mut parser, chunk, borrowed);
        }
        *sample = start.elapsed().as_secs_f64();
    }
    elapsed.sort_by(f64::total_cmp);

    let mut parser = KittyGraphicsInterceptor::default();
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_BYTES.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for _ in 0..ALLOCATION_ITERATIONS {
        consume(&mut parser, chunk, borrowed);
    }
    COUNTING.store(false, Ordering::Relaxed);
    let median = elapsed[SAMPLES / 2];
    let mib_per_second = iterations as f64 * chunk.len() as f64 / (1024.0 * 1024.0) / median;
    println!(
        "{label},{},{},{median:.6},{mib_per_second:.1},{},{},{}",
        if borrowed { "after" } else { "before" },
        chunk.len(),
        ALLOCATIONS.load(Ordering::Relaxed) / ALLOCATION_ITERATIONS,
        ALLOCATED_BYTES.load(Ordering::Relaxed) / ALLOCATION_ITERATIONS,
        PEAK_BYTES.load(Ordering::Relaxed),
    );
}

fn main() {
    println!(
        "scenario,path,chunk_bytes,median_seconds,mib_per_second,allocations_per_chunk,allocated_bytes_per_chunk,peak_transient_bytes"
    );
    for (label, line) in [
        (
            "plain",
            b"the quick brown fox jumps over the lazy dog 0123456789\r\n".as_slice(),
        ),
        (
            "ansi",
            b"\x1b[1;32mbuild complete\x1b[0m\r\n\x1b[2Kprogress: 100%\r\n".as_slice(),
        ),
        (
            "unicode",
            "ASCII caf\u{e9} \u{1f7e2}G \u{754c}\r\n".as_bytes(),
        ),
    ] {
        let chunk = line.repeat((16 * 1024 / line.len()).max(1));
        measure(label, &chunk, false);
        measure(label, &chunk, true);
    }
}
