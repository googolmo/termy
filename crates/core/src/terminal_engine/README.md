# Terminal engine replacement

This directory owns the parser, screen storage, and platform PTY transport used
by native, display-only, tmux and persistent Termy sessions. The legacy tmon
engine implementation stays in its original folder outside the compiled module
tree; its obsolete external-oracle test glue has been removed.

## Design

- `parser.rs` is a streaming UTF-8/VT state machine. Printable ASCII is passed
  to the screen in runs. CSI parameters use fixed arrays; OSC/DCS/APC share a
  reusable, bounded buffer. Fragment boundaries must never change behavior.
- `grid.rs` owns primary/alternate screens, scrollback, wide characters,
  cursor state and dirty ranges. Scrolling moves row ownership and reuses
  evicted row allocations once history is full. At most 32 ordered scroll
  operations let renderers move cached rows and repaint only exposed spans.
  Resize streams reflow into a bounded window and reuses discarded row buffers,
  avoiding a temporary allocation proportional to old history times new width.
- `types.rs` provides compact engine-owned values. Colors occupy four bytes;
  cells occupy at most 32 bytes. Combining marks and links use shared optional
  metadata, so ordinary cells and ASCII writes allocate nothing.
- `dispatch.rs` applies control sequences and owns modes, palette changes,
  hyperlinks and bounded event/reply queues. Clipboard controls share the parser
  and synchronized commits, avoiding a second scan and filtered input copy.
  `queries.rs` reports live VT state.
- `sync.rs` buffers synchronized output with a 2 MiB limit and a 150 ms timeout.
  A syntax-aware marker scanner preserves ordering across fragmented strings.
- `graphics.rs` applies image commands and ordered scroll/clear effects inside
  the same parser commits as text. Animation revision polling allocates nothing.
- `transport/` provides bounded native PTY input/output on Unix and Windows.
  The runtime watchdog sleeps while idle and wakes only for pending synchronized
  output deadlines.
- `Engine` is single-owner state. Transport/runtime synchronization belongs
  outside it. Viewport and history reads borrow row slices rather than building
  intermediate cell vectors.

The initial history representation is dense. It prioritizes predictable access
and row reuse. Allocation benchmarks report retained heap as well as throughput;
compact history is a remaining optimization to assess against real workloads.
Grid dimensions are clamped to 4,096 per axis and 1,048,576 cells in total.
History retains at most 20,000 rows and 1,048,576 cells, so wide grids can retain
fewer rows than the configured history count. Combining suffixes, CSI parameters,
strings, title stacks, keyboard stacks, events and replies are also bounded. The runtime event queue
has an 8 MiB retained-payload budget as well as a count limit; dropping clipboard
chunks aborts the transaction, and resets still revoke permission grants.
Large direct feeds and synchronized replay apply graphics effects in 64 KiB
segments, so effect queues cannot grow with an arbitrarily large caller buffer.

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
and wide Unicode must reuse their warmed storage. Repeated combining suffixes
also allocate nothing after warmup: a per-grid, 256-entry cache shares immutable
metadata, including hyperlink identity, without changing older cells when a new
mark is appended. Suffixes are capped at 256 bytes, and links with more than
1024 bytes of allocated string storage bypass the cache to bound retained memory.
New combinations still allocate; the benchmark asserts zero allocations for its
repeated combining workload as well as ordinary text. Timings include
allocator instrumentation and are not a substitute for PTY/UI latency tests.
The heap figures are allocator-requested bytes for the process, not OS RSS.
The varied-Unicode case cycles through 16,384 distinct CJK scalars to exercise
width lookups that a small cache cannot retain. Keep it alongside the repeated
Unicode case when evaluating width optimizations. The cloud evaluation rejected
a 512-entry width cache because its mixed results did not justify adopting it.

## Integration validation

Keep these checks green when changing the engine:

- Native PTYs on macOS/Linux and Windows ConPTY use this engine, with bounded
  input queues, ordered replies, resize, shutdown and child-exit handling.
- Native, display, tmux and remote sessions share the public core render and
  terminal contracts. Search, links, selection, palette, clipboard, shell
  integration, Kitty graphics and synchronized output retain their behavior.
- Resize/reflow, history anchoring, Unicode, screen editing and damage replay
  are covered by expected-state regression tests. DCS/APC handlers and
  synchronized-output commit/timeout behavior must pass fragmentation and native
  transport tests.
- All active Alacritty imports, adapters, comparisons and Cargo dependencies
  are removed. The legacy tmon engine implementation remains outside the
  compiled module tree; fixed expected-byte tests replace its old external-oracle
  assertions.
- Core, desktop, FFI, IPC and tmux integration suites pass, along with workspace
  checks, formatting, Clippy and repository architecture gates.
- A real shell and tmux pane are exercised in the app. Parser throughput,
  steady-state allocations, retained memory, idle wakeups and input/frame
  latency are measured. Record regressions and memory tradeoffs alongside gains.
- The branch and PR contain the verified changes and required CI checks have
  reached successful terminal states.

The public runtime facade remains stable. Desktop panes use the shared facade,
so tmux and native sessions exercise the same engine and graphics pipeline.

Measured performance and native QA are recorded in
[`custom-engine-2026-10-05.md`](../../../../docs/engineering/custom-engine-2026-10-05.md).
