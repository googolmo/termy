# CPU and memory validation — 2026-10-05

## Changes

- Native PTY, display-only Alacritty, and tmux output processing borrow ordinary
  ASCII/ANSI input instead of allocating and copying it through the Kitty graphics
  interceptor. Graphics commands, non-ASCII input, and incomplete escape sequences
  retain the existing parser. The existing owned public API remains compatible.
- Cached drawing operations share normalized block, sextant, and braille geometry.
  A drawing operation now occupies 96 bytes instead of 240. Box drawing still uses
  geometry resolved against the snapped physical cell bounds.
- Scrollbar fades sleep through the fully opaque hold period and stop recurring
  wakeups during dragging. Pointer events still repaint movement, and opacity
  changes still animate. A default hold/fade needs 9 timer wakeups instead of 65.

No scrollback limits, terminal features, or user settings were reduced.

## Before and after

| Measurement | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| CPU time for fixed background log-output workload, median of 5 runs | 1.54 s | 1.39 s | 9.7% |
| Dense 38 × 112 viewport, allocated drawing-operation vectors | 1,167,360 B | 466,944 B | 60.0% |
| One drawing operation | 240 B | 96 B | 60.0% |
| ASCII interception of 128 MiB, median of 7 samples | 199.830 ms | 32.333 ms | 83.8% |
| ANSI interception of 128 MiB, median of 7 samples | 194.644 ms | 38.608 ms | 80.2% |
| Interceptor allocations per approximately 16 KiB ASCII/ANSI chunk | 3 | 0 | 100% |
| Scrollbar timer wakeups per default hold/fade, deterministic test | 65 | 9 | 86.2% |

The cache measurements cover drawing-operation vectors, not the entire terminal
or process. Shared geometry tables use at most 69,400 additional bytes across the
whole process, initialized by glyph family. Even charging every table to one
dense viewport, its drawing-operation storage falls by 54.1%. Plain text does
not initialize those tables. Allocation capacity, including unused vector slots,
is included in these figures; allocator bookkeeping is not.

Whole-process settled RSS was effectively unchanged: idle medians were
139.20 → 139.45 MiB; after background output, 141.28 → 141.14 MiB. Sampled peak
RSS during output was 143.33 → 141.78 MiB, a small 1.1% reduction. These results
do **not** establish a large reduction in total application RAM or idle CPU.

## Renderer CPU check

The isolated collector builds the same 38 × 112 colored/glyph viewport before
and after, using seven interleaved samples of 300 rebuilds each. It retains vector
capacity and warms lazy geometry tables before timing. Text shaping, GPU painting,
and window presentation are excluded.

| Viewport content | Before, microseconds/frame | After, microseconds/frame |
| --- | ---: | ---: |
| Colored text | 97.375 | 91.249 |
| Blocks | 82.995 | 54.913 |
| Sextants | 96.755 | 79.780 |
| Braille runs | 100.526 | 81.108 |
| Box drawing | 148.953 | 128.864 |

The smaller representation did not trade memory savings for slower collection in
these workloads. These timings do not imply the same improvement in application FPS.

## Method and limits

- Same Apple Silicon Mac (`Mac16,1`, 10 physical cores, 24 GiB RAM), macOS 27.2,
  Rust 1.99.0, locked dependencies, optimized release profile.
- Baseline: clean commit `da34d3b57ca90a376cfd1253ff985d875474b229`, built and
  saved before source changes. Both binaries used the same Cargo package/target
  selection, including `termy` and the CLI's `xtask`, to match dependency features.
- Application runs used isolated homes/configurations, Alacritty, no multiplexer
  or tmux, 1280 × 820 windows, opaque background, and cursor blink disabled.
- Five runs per binary per scenario, alternating order, without concurrent builds
  or tests. CPU time is `/usr/bin/time -lp` user plus system time for the launched
  workload. RSS comes from the app's existing process sampler; settled RSS is the
  median of its final two seconds. CPU totals include startup and the same driver.
- Log output is exactly 1,179,648 lines / 104,988,672 bytes followed by three
  seconds to drain and settle. The configured history limit stays unchanged.
- All final application runs recorded only two grid paints: the windows became
  occluded. Consequently the log comparison describes **background processing**.
  Styled-window runs are retained in the raw data but excluded from visible-rendering
  performance claims. The benchmark harness now warns when active workloads stop
  painting. Benchmark mode also bypasses normal frame-batched event draining.
- The earlier exploratory baseline painted normally, but has no matching candidate
  run and is not used in the comparison. Native visual inspection could not capture
  the QA window (`cgWindowNotFound`). The packaged launch probe did report a usable
  terminal frame in 143 ms; this is a smoke check, not a startup comparison.
- The interceptor benchmark compares the unchanged owned API with the new borrowed
  API in the same executable. Timings have allocation counting disabled. Allocation
  bytes count requested sizes, not allocator size classes or process RSS. Unicode
  remains on the original path (170.162 → 167.700 ms; no meaningful claimed gain).

## Validation

Release desktop/core tests passed (1,991 test executions, excluding one nested
isolated test's duplicate result). All 11 normally ignored tmux tests also passed
with tmux 3.7c. Strict Clippy, formatting, whitespace checks, and repository
boundaries passed.

New regression coverage compares owned/borrowed interception over every pair of
split boundaries in ANSI, Kitty APC, C1, repeated ESC, and UTF-8 fixtures. Geometry
tests compare all 347 cached codepoints to their canonical plans at four cell
sizes. Scrollbar tests cover renewed activity, dragging/release, disabled modes,
fade completion, and zero-duration fades.

## Reproduction and artifacts

```sh
cargo build --locked --release -p termy --bin termy -p termy_cli --bin xtask
# Save the baseline binary before applying the changes, then build the candidate.
python3 scripts/benchmark-resources.py \
  --baseline /absolute/path/to/before/termy \
  --candidate /absolute/path/to/after/termy \
  --output /absolute/path/to/new-report-directory --repeats 5
cargo run --locked --release -p termy_core --example graphics_interceptor
cargo test --locked --release -p termy -p termy_core
cargo test --locked --release -p termy -p termy_core -- --ignored --test-threads=1
cargo clippy --locked --release -p termy -p termy_core --all-targets -- -D warnings
cargo fmt --all -- --check
bash scripts/check-boundaries.sh
```

Local artifacts are retained under `target/performance-2026-10-05/` (ignored by Git):

- `baseline/termy` and `candidate/termy`: saved release binaries.
- `comparison/results.json`, `summary.json`: samples and median summaries.
- `comparison/*/metrics/`: raw timelines and frame/redraw counters.
- `comparison/*/time.txt`: external CPU and resource measurements.
- `graphics-interceptor.csv`: timing/allocation comparison.
- `grid-collector-results.jsonl`: renderer timing and vector capacity measurements.
- `grid-collector{,-body}.rs`, `grid-{before,after}.rs`, `grid-collector`:
  exact paired collector sources and executable. Re-run locally with
  `target/performance-2026-10-05/grid-collector 300`.
- `tests.log`, `ignored-tests.log`, `clippy.log`, `boundaries.log`: validation logs.

Saved binary SHA-256:

```text
before 41a01ce5878c893e9946e32825a01ba5f2b39b09386f367c4eba6249e97cd12a
after  eba7a3230cdbbd736a0f059c80d6be03021cda4f6dd960d21992d5b3442ea638
```
