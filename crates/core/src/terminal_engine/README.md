# Terminal engine replacement

This directory owns the new terminal engine. The goal is to remove Alacritty
from every shipped Termy path and from the dependency graph, while leaving
`crates/core/src/tmon` unchanged. This is an in-progress replacement, not yet
the default runtime.

## Design

- `parser.rs` is a streaming UTF-8/VT state machine. Printable ASCII is passed
  to the screen in runs. CSI parameters use fixed arrays; OSC/DCS/APC share a
  reusable, bounded buffer. Fragment boundaries must never change behavior.
- `grid.rs` owns primary/alternate screens, scrollback, wide characters,
  cursor state and dirty ranges. Full-screen scrolling moves row ownership,
  and reuses evicted row allocations once history is full.
- `types.rs` provides compact engine-owned values. Colors occupy four bytes;
  cells occupy at most 32 bytes. Combining marks and links use shared optional
  metadata, so ordinary cells and ASCII writes allocate nothing.
- `dispatch.rs` applies control sequences and owns modes, palette changes,
  hyperlinks and bounded event/reply queues.
- `Engine` is single-owner state. Transport/runtime synchronization belongs
  outside it. Viewport and history reads borrow row slices rather than building
  intermediate cell vectors.

The initial history representation is dense. It prioritizes predictable access
and row reuse. Allocation benchmarks report retained heap as well as throughput;
compact history is a remaining optimization to assess against real workloads.
Grid dimensions are clamped by both axis and total-cell limits. History has a
separate cell limit. Combining suffixes, CSI parameters, strings, title stacks,
keyboard stacks, events and replies are also bounded.

VT behavior is checked against the
[XTerm control sequence reference](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
and the [Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/).
Tests use explicit expected states, malformed-input fixtures and all-boundary
fragmentation checks, without an Alacritty test oracle.

## Verification

```sh
cargo test -p termy_core terminal_engine
cargo run --release -p termy_core --example terminal_engine_bench -- 32
```

The benchmark warms each workload first, feeds both 64 KiB and one-byte chunks,
and counts actual allocator calls. Steady plain-text scrolling, styled redraws
and wide Unicode must reuse their warmed storage. Combining text is measured
separately because its uncommon metadata currently allocates. Timings include
allocator instrumentation and are not a substitute for PTY/UI latency tests.
The heap figures are allocator-requested bytes for the process, not OS RSS.

## Remaining completion gates

The replacement is complete only when all of these hold:

- Native PTYs on macOS/Linux and Windows ConPTY use this engine, with bounded
  input queues, ordered replies, resize, shutdown and child-exit handling.
- Native, display, tmux and remote sessions share the public core render and
  terminal contracts. Search, links, selection, palette, clipboard, shell
  integration, Kitty graphics and synchronized output retain their behavior.
- Resize/reflow, history anchoring, Unicode, screen editing and damage replay
  are covered by expected-state regression tests. Protocol features not yet
  implemented here (including DCS/APC handlers and synchronized-output commit/
  timeout behavior) must be completed before the default runtime switches.
- All active Alacritty imports, adapters, comparisons and Cargo dependencies
  are removed. The legacy tmon source stays unchanged and leaves the compiled
  module tree so its old oracle tests do not retain the dependency.
- Core, desktop, FFI, IPC and tmux integration suites pass, along with workspace
  checks, formatting, Clippy and repository architecture gates.
- A real shell and tmux pane are exercised in the app. Parser throughput,
  steady-state allocations, retained memory, idle wakeups and input/frame
  latency are measured. Any performance regression is explained and addressed.
- The branch and PR contain the verified changes and required CI checks have
  reached successful terminal states.

The public runtime facade remains stable during this work. Desktop panes are
being migrated to that facade first, eliminating the duplicate desktop emulator
so the eventual engine switch applies to tmux and native sessions together.
