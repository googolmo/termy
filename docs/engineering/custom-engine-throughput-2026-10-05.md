# Custom engine throughput follow-up — 2026-10-05

The custom engine exceeds the former native Alacritty backend's median feed
throughput in all seven measured workloads on this Linux executor. Six
alternating pairs show gains of **1.36–4.80×** without damage consumption and
**1.20–4.95×** with it. These results address the Unicode and combining-text
throughput gaps; they do not establish universal application or rendering parity.

This is the performance follow-up to `fed48fda`, superseding the earlier
revision's throughput conclusions in the [original report](custom-engine-2026-10-05.md).
That report's historical display backend comparison, dense-history tradeoff and
native macOS smoke evidence remain relevant. The former experimental display
backend was not changed or remeasured by this follow-up and was later removed.

## Changes and correctness

- Decode complete valid UTF-8 scalars once per scalar. Malformed and fragmented
  input keeps the original streaming decoder and replacement behavior.
- Replace scalar destination cells directly, repairing only outside halves of
  overwritten wide glyphs. Combine cell and cursor damage updates.
- Track each row's conservative occupied prefix and erase background. Recycled
  rows clear only that prefix unless the background changes. Editing and reflow
  preserve the invariant across primary, alternate and history rows.
- Keep the combining cache bounded at 64 buckets × four entries. Use a small
  byte hash and inline comparisons for keys up to four bytes, while retaining
  exact byte and hyperlink-pointer checks and immutable shared metadata.

Software CPU sampling identified repeated row clearing and general-purpose
`memcmp` calls for tiny combining keys as substantial costs. The implementation
uses safe Rust, adds no dependency and does not change the glyph-width lookup.

All 169 engine tests pass locally. Added coverage includes every Unicode scalar,
incomplete and malformed sequences, wide-cell overwrite/damage boundaries,
combining-cache collisions and eviction, and row-prefix invariants through
randomized editing, background changes, history and alternate-screen operations.

The complete local core and CLI suites also pass: 721 core unit tests and 46 core
integration tests, with existing ignores retained. Core library/example strict
Clippy, workspace formatting and architecture boundary checks pass.
The Linux desktop build and virtual-display smoke also pass: a visible rendered
window accepts keyboard input and executes a command through a real PTY shell.

## Comparable measurements

- Baseline: `64945371249fb123db92dc0ce55f32970c7536e1`, built in an isolated worktree.
  `TERMY_CORE_TEST_BACKEND=alacritty` selects its historical native engine;
  otherwise the old display facade defaults to the experimental engine. Every
  process reports its engine, and the runner verifies the labels.
- Candidate runtime: `6b3613b27519f015673ad8ed429a80c901432618`, the performance
  follow-up to `fed48fda`. Saved binary hashes below identify the measured builds.
  Both use release `termy_core` libraries and the
  **identical, uninstrumented** `terminal_facade_bench.rs` source.
- Linux x86_64, Intel Xeon Platinum 8573C, Rust 1.99.0, locked dependencies.
  Timed throughput processes were pinned to CPU 2. No builds or tests overlapped.
- 120 × 40 cells, 1,000 history rows, eight warmup payloads, at least 32 MiB per
  workload per process. Six pairs alternate baseline/candidate order. The runner
  retains raw output, binary hashes, individual ratios and ranges in JSON.
- Throughputs below are medians. Ratios are medians of the six **paired** ratios,
  so they need not equal the ratio of the two displayed throughput medians.

| Workload | Alacritty MiB/s | Custom MiB/s | Paired ratio |
| --- | ---: | ---: | ---: |
| Plain scrolling | 71.430 | 169.635 | 2.428× |
| Styled redraw | 47.423 | 90.438 | 1.887× |
| Mixed Unicode | 68.966 | 92.939 | 1.360× |
| 16,384 distinct Unicode scalars | 63.189 | 110.355 | 1.787× |
| Repeated combining marks | 39.122 | 60.747 | 1.551× |
| 112 distinct combining marks | 31.132 | 62.401 | 2.032× |
| One-byte plain-text fragments | 1.780 | 8.550 | 4.804× |

The second mode drains damage after each full payload, approximately 64 KiB
(approximately 100 KiB for varied Unicode). This measures damage reset and
consumption overhead. Scrolling can saturate full damage within a payload;
this does not model incremental renderer cadence or render any cells.

| Workload, with damage consumption | Alacritty MiB/s | Custom MiB/s | Paired ratio |
| --- | ---: | ---: | ---: |
| Plain scrolling | 71.774 | 154.488 | 2.091× |
| Styled redraw | 49.181 | 99.042 | 1.991× |
| Mixed Unicode | 69.825 | 84.422 | 1.201× |
| 16,384 distinct Unicode scalars | 61.454 | 99.589 | 1.654× |
| Repeated combining marks | 37.668 | 54.669 | 1.440× |
| 112 distinct combining marks | 29.584 | 58.091 | 1.922× |
| One-byte plain-text fragments | 1.724 | 8.489 | 4.954× |

Cloud timings vary: the mixed-Unicode damage case's individual paired ratios
range from 0.847× to 1.276×. The claims above concern the six-pair medians,
not every individual run. Independent hosted comparisons are retained in CI.

Source and executable SHA-256:

```text
helper     d6e2787dfcef87352cf4615df9868c5177a4342d055ea79a5a8bab046d16a7e8
baseline   1b5e1cd4f7a9b8ad5ba95cf0607ff443686e9c69179448cce1a8276967f76825
candidate  a450b5498619eb2318498af88b6e6cf222f62572390bc8488f4482350cda5aba
```

## Allocation and PTY checks

The separate instrumented `terminal_engine_bench` passes all six warmed
allocation gates with **zero allocations and zero allocated bytes**. Retained
requested process heap is 4,008 KiB for plain/Unicode/fragmented scrolling,
218 KiB for styled redraw, 4,121 KiB for varied Unicode and 4,009 KiB for repeated
combining marks. Payload storage is included; these figures are not OS RSS or
an equivalent-facade comparison with Alacritty. Dense history remains unchanged.

Identical `native_roundtrip_bench.rs` source was also compiled against both saved
libraries. Six alternating pairs each used a real no-echo PTY, 100 warmup and
1,000 measured exchanges, without CPU pinning or concurrent builds.

| Median of per-run latency statistics | Alacritty | Custom |
| --- | ---: | ---: |
| p50 | 57.853 µs | 55.990 µs |
| p95 | 152.134 µs | 140.271 µs |
| p99 | 520.320 µs | 407.853 µs |

These cloud results show no median latency regression in this probe; noisy
scheduling and tail outliers preclude claiming a general latency improvement.
This measures transport, parsing and host wakeups, not physical keyboard input
or displayed frames.

## Reproduction and CI

Build each revision's release core library, then compile the same helper against
each using the command in its source header. Run
`scripts/benchmark-terminal-facades.py` with `--mib 32 --pairs 6`, first normally
and then with `--consume-damage`. See the [engine README](../../crates/core/src/terminal_engine/README.md)
for complete runner usage and the separate allocation probe.

PR CI now performs both paired modes on Linux and macOS, preserving provenance,
raw JSON, logs and summary tables as workflow artifacts. Each workload's median
paired ratio must reach 0.95; this 5% noise allowance is a regression gate, not
proof of parity. Examine the actual medians and recorded baseline engine. Future
PRs whose base already uses the custom engine measure custom-to-custom changes.
The PR description links the final verified CI results; local numbers here must
not be described as macOS measurements.

Local raw data, saved binaries and CPU profiles live under the ignored
`target/engine-performance/`. Existing desktop performance gates and platform
tests remain in place. Presented-frame latency, focused-window output throughput
and universal workload parity remain outside these measurements.
