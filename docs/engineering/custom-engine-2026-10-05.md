# Native terminal engine performance — 2026-10-05

This report records earlier revisions. The later
[throughput follow-up](custom-engine-throughput-2026-10-05.md) resolves the
measured Alacritty Unicode/combining gaps with new uninstrumented paired probes.
Its results do not replace the historical display backend comparison or
presentation limits.

The replacement improves several measured paths relative to the former native
Alacritty runtime, but it is **not a uniform performance improvement**. Mixed
Unicode is slower, and the old compact experimental display backend is
substantially faster and smaller than the new dense grid in these workloads.
Real PTY latency is similar. The measured changes do not establish a reduction
in idle CPU.

The original macOS measurements stopped at the user's request to push the work
for cloud continuation. The cloud follow-up at the end of this report records
the completed CI tracing runs and a rejected Unicode-width cache experiment.
Focused-app paired output measurements and presented-frame latency remain
unmeasured; CI render callbacks do not establish presentation latency.

## Provenance and method

- Baseline: `64945371`; candidate: `427677cb8e24244287f69d71ada95bfae5f087a3`.
  Later patches must not be described as the binaries measured here.
- Apple Silicon `Mac16,1`, 10 physical cores, 24 GiB RAM; macOS 27.2 (26B5091g).
  Rust 1.99.0 (b940084d7), optimized release profile, locked dependencies.
- Baseline was built in an isolated detached worktree. Both desktop builds
  selected `termy` and `termy_cli`'s `xtask` together. No active checkout reset.
- Measurements ran sequentially without concurrent compilation or test suites.
  Paired samples alternated baseline/candidate order.
- Heap counters measure requested memory routed through Rust's global allocator,
  including the benchmark payload. They exclude allocator bookkeeping and
  allocations outside that allocator; they are not process RSS. Timing includes
  allocator instrumentation, whose overhead can differ when allocation counts
  differ. Application RSS was measured separately.
- Parser and facade feed benchmarks do **not** consume render damage between
  feeds. They measure parsing/storage/runtime work, not renderer throughput.

Saved desktop binary SHA-256:

```text
baseline  eba7a3230cdbbd736a0f059c80d6be03021cda4f6dd960d21992d5b3442ea638
candidate 3bf6b02e28fcab9556ac99dec49527d0a9c73d9c01921d9553afbd72e141f637
```

## Parser/grid hot path

The committed `terminal_engine_bench` warms each workload with eight approximately
64 KiB blocks, then measures 128 MiB per case at 120 columns × 40 rows with a
1,000-row history limit. Five runs; 32-byte cells.

| Workload | Median MiB/s | Min–max MiB/s | Allocations in each measured run | Retained requested heap |
| --- | ---: | ---: | ---: | ---: |
| Plain scrolling, 64 KiB chunks | 311.7 | 297.8–312.7 | 0 | 4,000 KiB |
| Styled cursor-addressed redraw, 64 KiB | 221.1 | 219.4–224.2 | 0 | 218 KiB |
| Mixed Unicode, 64 KiB | 124.3 | 122.5–124.6 | 0 | 4,000 KiB |
| Repeated combining marks, 64 KiB | 75.6 | 75.5–75.7 | 0 | 4,000 KiB |
| Plain scrolling, one-byte fragments | 41.1 | 41.1–41.2 | 0 | 4,000 KiB |

All 25 measured runs allocated zero bytes; peak requested heap equaled retained
heap. Styled redraws do not fill history. Repeated combining marks reuse cached
metadata; arbitrary new combinations can allocate. Benchmark binary SHA-256:
`ae9c7ef3f4c07fefa0926584939b780d05225bb5c8a08b1568805bd8c5b8f2e3`.

## Public facade: former native Alacritty path

The same public-API helper was compiled against both revisions. It uses exactly
the five payloads, chunking, warmup and allocator implementation from
`terminal_engine_bench`, substitutes `Terminal::new_display` for `Engine::new`,
sets a 1,000-row history through `TerminalRuntimeConfig`, calls `feed_output`,
and observes `cursor_position` after timing. It permits baseline allocations.
Each case feeds 32 MiB; medians are from five interleaved paired runs.

Baseline `Terminal::new_display` defaults to the experimental display backend,
so this comparison explicitly sets the baseline's existing
`TERMY_CORE_TEST_BACKEND=alacritty` selector. The candidate has one engine and
ignores that removed selector. The former native runtime and desktop pane/tmux
path used Alacritty; the old public display facade also needs the separate
comparison below.

| Workload | Baseline MiB/s | Candidate MiB/s | Candidate / baseline |
| --- | ---: | ---: | ---: |
| Plain scrolling, 64 KiB | 143.6 | 309.0 | 2.15× |
| Styled redraw, 64 KiB | 98.9 | 214.5 | 2.17× |
| Mixed Unicode, 64 KiB | 145.4 | 120.2 | 0.83× |
| Repeated combining marks, 64 KiB | 74.7 | 75.0 | 1.00× |
| Plain scrolling, one-byte fragments | 4.4 | 23.3 | 5.30× |

Mixed Unicode throughput is **17.3% lower**. A proposed glyph-width cache had not
been applied or measured when this report was finalized. It must be tested with
both repeated and varied/cache-miss-heavy characters before claiming a fix.

| Workload | Baseline allocations per 32 MiB | Candidate allocations | Baseline retained heap | Candidate retained heap |
| --- | ---: | ---: | ---: | ---: |
| Plain scrolling, 64 KiB | 0 | 0 | 5,190 KiB | 4,009 KiB |
| Styled redraw | 2,345,472 | 0 | 2,347 KiB | 227 KiB |
| Mixed Unicode | 3,584 | 0 | 5,190 KiB | 4,009 KiB |
| Repeated combining marks | 15,983,104 | 0 | 5,556 KiB | 4,009 KiB |
| Plain scrolling, one-byte fragments | 0 | 0 | 5,190 KiB | 4,009 KiB |

These are synthetic steady-state workloads, including instrumentation. The
allocation improvement alone does not prove an application CPU improvement.

## Former public display path

With no selector, the baseline public display facade used the experimental
engine. That was relevant to embedding and multiplexer/display sessions; it must
not be confused with the former native Alacritty path.

| Workload | Former display MiB/s | Candidate MiB/s |
| --- | ---: | ---: |
| Plain scrolling, 64 KiB | 1,536.0 | 309.3 |
| Styled redraw, 64 KiB | 411.0 | 214.5 |
| Mixed Unicode, 64 KiB | 241.6 | 119.4 |
| Repeated combining marks, 64 KiB | 269.4 | 75.1 |
| Plain scrolling, one-byte fragments | 36.5 | 23.9 |

For plain scrolling, retained requested heap grows from **274 KiB to 4,009 KiB**
(14.6×). The old backend stores history compactly; the new grid uses dense cell
rows. This is a material memory and throughput regression for that former path,
not an equivalent-performance replacement. Compact history and reducing work
per cell remain worthwhile follow-up work.

Both measurements requested the same ordinary 120-column, 1,000-row history.
Separately, the new engine caps each viewport axis at 4,096 and the viewport at
1,048,576 cells. History is capped at 20,000 rows **and** 1,048,576 cells, so very
wide terminals retain fewer than the configured number of history rows. The
ordinary benchmark dimensions do not hit that cell cap.

## Real native PTY latency

The committed `native_roundtrip_bench` launches a real Unix PTY with raw input
and echo disabled, then sends alternating carriage-return-prefixed two- and
three-character strings through `/bin/cat`. It waits for the expected parsed
cursor column using the host notifier, with bounded waits. Each process excludes
100 warmup exchanges and measures 1,000 exchanges. Five interleaved pairs.

| Median of five run statistics | Baseline | Candidate |
| --- | ---: | ---: |
| p50 | 15.125 µs | 14.750 µs |
| p95 | 17.666 µs | 17.208 µs |
| p99 | 20.334 µs | 19.417 µs |

Candidate run p95 values ranged from 15.791 to 23.833 µs; baseline ranged from
17.000 to 18.125 µs. Treat the medians as similar latency, not evidence of a
small statistically established improvement. This includes input queueing, child
echo, output parsing and host wakeup; it excludes physical keyboard input,
window rendering and display presentation.

## Settled application idle samples

Three interleaved pairs of saved desktop binaries were launched through CUA in
separate minimal app bundles. They used isolated `XDG_CONFIG_HOME`, instance
storage and working directories, inherited the normal macOS home/environment,
1280 × 820 windows, opaque backgrounds, disabled cursor blink and no multiplexer
or tmux. The terminal was focused before releasing the workload start gate.
After the gate, the driver waited one second and then remained idle seven
seconds. The app's existing sampler provided CPU/RSS/runtime-wakeup counters.

| Median across three runs | Baseline | Candidate |
| --- | ---: | ---: |
| Settled process RSS, final two seconds | 143.09 MiB | 137.08 MiB |
| Sampled peak RSS | 144.53 MiB | 140.47 MiB |
| Settled app CPU, percent of one core | 0.123% | 0.167% |
| Runtime output wakeups in settled window | 0 | 0 |

The settled wakeup windows were approximately 3.7–4.2 seconds. These counters are
**runtime output wakeups**, not operating-system wakeups. Idle RSS fell about
6 MiB in these samples; no idle CPU reduction was demonstrated. The sample is
small, other desktop processes remained running, and the monitoring process also
sampled app CPU/RSS every 250 ms. Whole-process CPU totals include variable
launch/focus waits and are deliberately not compared.

Initial attempts with an isolated `HOME` produced `cgWindowNotFound` and only two
startup paints; those runs were excluded. Preserving the normal macOS home made
the windows accessible. The six accepted idle samples recorded 7–24 paints,
including startup and focus transitions.

One baseline-only fixed-output run completed before work stopped. There is no
matched candidate result, so **no application output CPU/throughput comparison
is reported**. The paced `xtask` echo-to-frame runs did not complete; **frame
latency is unmeasured**. The real PTY measurements above do not replace that
remaining display-latency check.

## Correctness and native smoke evidence

The full verification immediately before the last upstream event-overflow fix
passed 717 core unit tests, 46 integration tests, 878 desktop tests and 17 pane
tests; normal existing ignores were retained. Strict workspace Clippy, formatting
and repository boundary checks passed. The final overflow regression increased
the core unit inventory to 718; all 158 engine tests, including that regression,
passed at `427677cb`. This is not a claim that every earlier suite was rerun after
that final narrow patch.

Native app inspection verified shell input/paste, a wide `界` glyph and combining
text, ANSI green text, synchronized Kitty image layout/layers/placeholders,
red-to-green image animation, scroll/clip/chunk cursor behavior, resizing from
149 × 39 to 86 × 45, and a real tmux session with commands rendered in two split
panes. Screenshots were used; no video was recorded. Cross-platform correctness
and final CI status belong to the PR checks, not these macOS-only measurements.

## Reproduction and retained artifacts

```sh
cargo build --locked --release -p termy --bin termy -p termy_cli --bin xtask
cargo run --locked --release -p termy_core --example terminal_engine_bench -- 128
cargo run --locked --release -p termy_core --example native_roundtrip_bench
```

To recreate the paired facade helper in another checkout, keep the allocator,
workloads and timing loop from `terminal_engine_bench` and make these changes:

```rust
use termy_core::{Terminal, TerminalRuntimeConfig, TerminalSize};
let engine = Terminal::new_display(
    TerminalSize { cols: 120, rows: 40, ..Default::default() },
    Some(&TerminalRuntimeConfig { scrollback_history: 1000, ..Default::default() }),
);
// Replace engine.feed(chunk) with engine.feed_output(chunk).
// Replace the post-timing viewport observation with engine.cursor_position().
// Remove the Cell-size print and zero-allocation assertion; keep the counters.
```

Compile the identical helper with `rustc --edition=2024 -O`, linking each
revision's release `termy_core` and dependency directory. Run 32 MiB per case,
five alternating pairs. For the native-engine baseline comparison set
`TERMY_CORE_TEST_BACKEND=alacritty`; omit it for the former display default.
The candidate ignores that removed environment selector.

Local artifacts under `target/custom-engine-2026-10-05/` are ignored and will not
transfer with Git. This report contains the aggregate data and exact provenance
needed for cloud continuation. Locally retained files include:

- `builds.json`, saved `baseline/` and `candidate/` binaries and build logs.
- `parser-final-results.txt`: five final-candidate parser runs.
- `paired-alacritty-results.json`: five paired native-engine facade samples.
- `paired-core-results.json`: five native PTY pairs and five default-display pairs.
- `native_roundtrip_bench.rs`, `terminal_facade_bench.rs`: exact helper sources.
- `gui-application-v3/results.json` and each accepted run's metrics/process
  samples. Earlier `application`, `ui-pilot` and failed GUI directories contain
  rejected exploratory runs and must not be combined with the accepted samples.

No width-cache optimization or additional application measurement is included
in the original macOS results above. The following cloud evaluation is separate.

## Cloud completion: dependency removal and Unicode evaluation

The cloud audit found no Alacritty packages in Cargo.lock or the workspace's
all-target dependency tree. The boundary gate now checks **all workspace
packages, features, target platforms, and normal/build/dev dependencies** for
Alacritty packages. It captures Cargo output before matching, avoiding an early
`rg -q` exit masking a forbidden dependency under `pipefail`. A fixture with an
Alacritty entry followed by 100,000 unrelated entries was correctly rejected.
The former experimental display implementation was outside the compiled module
tree at this checkpoint and was later removed; historical comparisons remain
historical evidence.

### Width-cache decision: rejected

The proposed per-grid 512-entry boxed `u32` cache was implemented and tested,
then removed. Each entry stored the complete scalar shifted left two bits plus
its width, used the top nine bits of `scalar.wrapping_mul(0x9e3779b1)` as the
slot, checked the complete scalar on hits, and used `UnicodeWidthChar::width`
on misses. ASCII bypassed the cache; reflow was unchanged. Exhaustive checks
covered every Unicode scalar on a miss and hit, width 3 for U+17D8, and collisions
between scalars of different widths. All 160 prototype engine tests passed.

Six alternating baseline/candidate pairs ran on the Linux x86_64 cloud executor
(Intel Xeon Platinum 8573C, five virtual CPUs, Rust 1.99.0), with no concurrent
build or test suite. Each case fed 128 MiB after eight warmup blocks. Both
binaries used the committed benchmark including its new 16,384-scalar CJK case.
The baseline engine was `b651c7bd`; the candidate changed only the grid's width
lookup and added its cache. These are parser/grid comparisons with the existing
custom engine, **not new comparisons against Alacritty**.

| Workload | Baseline median MiB/s (range) | Cache median MiB/s (range) | Ratio |
| --- | ---: | ---: | ---: |
| Plain scrolling, 64 KiB | 115.40 (94.7–119.1) | 145.40 (93.5–157.7) | 1.260× |
| Styled redraw | 99.40 (83.7–101.6) | 98.55 (59.2–107.3) | 0.991× |
| Repeated mixed Unicode | 54.65 (46.4–55.4) | 55.55 (38.2–60.1) | 1.016× |
| Varied Unicode | 62.55 (61.3–66.1) | 57.20 (50.8–63.4) | 0.914× |
| Combining marks | 16.35 (15.6–17.2) | 15.30 (13.6–18.3) | 0.936× |
| Plain scrolling, one byte | 16.60 (12.9–17.3) | 16.50 (15.6–17.7) | 0.994× |

All 72 measured cases allocated zero times after warmup. The cache added 2 KiB
of retained requested heap per engine. Timing varied substantially on the shared
cloud executor, including workloads that bypass the cache; these samples do not
establish precise causal speedups. The small repeated-Unicode gain and lower
varied/combining medians do not justify adopting the cache. The final engine
therefore retains its original lookup. The varied-Unicode benchmark is retained
in CI to expose this tradeoff in future proposals. The original mixed-Unicode
regression and the dense-history tradeoffs relative to the former display
backend remain unresolved and documented.

Benchmark binary SHA-256 values:

```text
baseline fe551d09628a5578cba5cc6b38e9816adc86085b9c0e0c9b1c67acd6ef87cd8c
cache    86d991d0dc212739667c97155a8b5853375a3f10f0f16c17fb53d11837c03ad2
```

Local raw samples, paired JSON, saved binaries and the rejected prototype are
under `target/custom-engine-cloud/` (ignored artifacts). Reproduce the retained
six-case allocation probe with `cargo run --locked --release -p termy_core
--example terminal_engine_bench -- 128`.

### Completed hosted tracing

All four [macOS performance gates at `b651c7bd`](https://github.com/lassejlv/termy/actions/runs/37296537917)
passed, including echo-train and steady-scroll. This confirms that the separate
startup allowance resolves the observed baseline trace timeout in that run.
The architecture/workspace/platform/tmux checks and parser allocation benchmark
also passed at that commit. Final-head status belongs to the PR checks.

The hosted echo-train report recorded 40 samples and zero missed echoes for
both binaries. First render-callback p95 was 23.37 ms baseline and 24.44 ms
candidate; p99 was 27.13 ms and 79.78 ms respectively. These are single-run
diagnostics, not a demonstrated latency improvement. Displayed-frame samples
were unavailable because Animation Hitches tracing was disabled. This Linux
executor cannot perform the handoff's focused native macOS window measurements;
neither the hosted callbacks nor a virtual Linux display replaces those checks.
