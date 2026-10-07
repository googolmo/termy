// Standalone audit probe; compile the identical source against both release libraries.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use termy_core::terminal_engine::{Engine, Options, Size};
struct Count;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size(), Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(n, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Count = Count;
fn run(payload: &[u8], compact: bool) {
    let mut e = Engine::new(
        Size {
            cols: 120,
            rows: 30,
        },
        Options {
            scrollback_history: 1000,
        },
    );
    for _ in 0..3000 {
        e.feed(payload);
    }
    if compact {
        e.compact_history();
    }
    for _ in 0..3000 {
        e.feed(payload);
    }
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    let now = Instant::now();
    for _ in 0..300000 {
        e.feed(std::hint::black_box(payload));
    }
    let t = now.elapsed();
    let a = ALLOCS.load(Ordering::Relaxed);
    let b = BYTES.load(Ordering::Relaxed);
    println!(
        "compact={compact} payload={} elapsed_us={} allocations={a} requested_bytes={b} history={}",
        payload.len(),
        t.as_micros(),
        e.history_size()
    );
}
fn main() {
    let plain = b"a short log line for testing\r\n";
    let mut varied = Vec::new();
    for n in 0..110 {
        varied.extend_from_slice(if n % 2 == 0 {
            b"\x1b[31mx"
        } else {
            b"\x1b[34mx"
        });
    }
    varied.extend_from_slice(b"\x1b[0m\r\n");
    for pair in 0..6 {
        println!("pair={pair} workload=plain");
        run(plain, pair % 2 != 0);
        run(plain, pair % 2 == 0);
        println!("pair={pair} workload=alternating-style");
        run(&varied, pair % 2 != 0);
        run(&varied, pair % 2 == 0);
    }
}
