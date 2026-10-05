//! Native input-to-parsed-state latency through a real no-echo PTY.
//!
//! Runs 100 warmup and 1000 measured round trips through `/bin/cat`. This measures
//! transport, parsing and host wakeups; it does not measure displayed frames.
//! `cargo run --release -p termy_core --example native_roundtrip_bench`

#[cfg(unix)]
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
#[cfg(unix)]
use termy_core::{Terminal, TerminalLaunch, TerminalSize, TerminalWakeupNotifier};

#[cfg(unix)]
fn main() {
    let (tx, rx) = mpsc::channel();
    let terminal = Terminal::new_with_launch_and_wakeup_notifier(
        TerminalSize::default(),
        None,
        Some(TerminalWakeupNotifier::new(move || {
            let _ = tx.send(());
        })),
        None,
        None,
        Some(&TerminalLaunch::Program {
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                "stty raw -echo; printf READY; exec /bin/cat".into(),
            ],
        }),
    )
    .expect("spawn raw no-echo PTY");
    let mut host = |_| None;
    let ready_deadline = Instant::now() + Duration::from_secs(5);
    while terminal.cursor_position().0 != 5 {
        terminal.drain_events(&mut host);
        rx.recv_timeout(ready_deadline.saturating_duration_since(Instant::now()))
            .expect("child readiness");
    }
    let mut times = Vec::with_capacity(1000);
    for sample in 0..1100 {
        terminal.drain_events(&mut host);
        while rx.try_recv().is_ok() {}
        let (payload, target) = if sample % 2 == 0 {
            (b"\rAB".as_slice(), 2)
        } else {
            (b"\rABC".as_slice(), 3)
        };
        let start = Instant::now();
        terminal.write(payload);
        let deadline = start + Duration::from_secs(2);
        while terminal.cursor_position().0 != target {
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("PTY round trip");
            terminal.drain_events(&mut host);
        }
        if sample >= 100 {
            times.push(start.elapsed().as_nanos() as u64);
        }
    }
    times.sort_unstable();
    println!(
        "samples={}, p50_us={:.3}, p95_us={:.3}, p99_us={:.3}, max_us={:.3}",
        times.len(),
        times[499] as f64 / 1000.0,
        times[949] as f64 / 1000.0,
        times[989] as f64 / 1000.0,
        times[999] as f64 / 1000.0
    );
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This benchmark requires a Unix no-echo PTY and /bin/cat.");
}
