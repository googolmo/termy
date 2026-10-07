# Terminal engine audit fixes — 2026-10-07

This follow-up fixes the 14 reproduced or source-traced findings from the
terminal engine audit. Each fix was committed and pushed separately to
[PR #402](https://github.com/termylabs/termy/pull/402), starting from `308bc090`.
Review corrections cover Unicode search coordinates, hidden-cursor graphics
invalidation, and repeated legacy PNG serialization.

## Findings and fixes

| Finding | Result and regression coverage |
| --- | --- |
| Packing continued indefinitely after idle when output arrived in small chunks | A cumulative 4 KiB output budget returns scrolling to dense rows; fragmented feeds, quiet recovery, and bounded compaction steps are covered. |
| Graphics upload chunks repeatedly scanned the entire viewport | Placeholder scans are restricted to commands needing their positions; visual edits invalidate placements, including hidden-cursor Unicode writes while full damage is pending. |
| Large completed or aborted APC strings retained oversized buffers | Parser exits release string capacity above 64 KiB, including cancellation and reset; small buffers remain reusable. |
| Alternate-screen resize left the saved primary cursor at stale coordinates | Resize follows the parked primary cursor through reflow and height changes while preserving a distinct independently saved cursor. |
| Unix child processes could inherit an ignored SIGPIPE | The forked child restores SIGPIPE with the other inherited signal dispositions before execution. |
| Search dropped combining suffixes and inserted wide-spacer artifacts | Runtime and desktop search preserve cell text and use explicit physical-column mappings, including joined/separate emoji, hidden wide cells, and one-column grids. Legacy frame search uses the metadata available in that frame. |
| Deferred wrap metadata was omitted from damage with a hidden cursor | The preceding row's wrap cell is marked independently of cursor visibility, for both ordinary and wide wrapping. |
| DECSTR soft reset was ignored | VT modes, pen, margins, character sets, and saved-cursor state reset while preserving current screen contents and position. |
| DEC private-mode save/restore was ignored | Supported modes have bounded saved state; restoration runs normal mode effects, including synchronized-output commits. |
| Lazy PNG exports escaped the graphics retention budget | Exports are owned handles with weak reuse; their encoded buffers release with the final consumer. Original PNG source capacity is charged. FFI and legacy remote batches share each image's encoding. |
| Windows control threads woke every 10 ms while idle | An auto-reset event and process-handle wait wake only for a request or child exit; tests cover coalescing and requests signaled before waiting. |
| Releasing borrowed history rows scanned all history | Cache invalidation visits the interval containing decoded reads; trim, clear, resize, and reset release or reset that interval before structural changes. |
| Synchronized-output scanning used a different APC limit | The scanner shares the main parser's string boundaries without retaining a duplicate payload; fragmented/interrupted strings preserve commit ordering. |
| Windows reply shutdown could discard final child output | Reply failure still starts shutdown, but the reader drains output to EOF before reporting exit. A pipe regression includes more than one read buffer and a final marker. |

`GraphicsImage::png()` now returns an owned `Arc<Vec<u8>>`; Rust callers should
retain it across an export batch. The C ABI, graphics wire format, and native
RGBA rendering path remain unchanged.

## Validation

At code checkpoint `04abfd8d`, local validation passed:

- `cargo test --workspace --offline` (all suites, including core, desktop, FFI,
  IPC, remote graphics/selection, and doctests).
- `just test-tmux-integration` (all 11 explicit cases).
- `cargo clippy --workspace --all-targets --offline -- -D warnings`.
- `cargo fmt --all -- --check` and `just check-boundaries`.
- Release core library and instrumented allocation example builds.

CI extends these checks with native Windows/Linux/macOS core tests,
Windows/Linux desktop checks, Linux release workspace tests, and macOS/Linux
tmux integration. The PR's checks show results for its current head, including
native execution of the Windows event and output-drain regressions.

## Measurement method

The initial local comparison uses baseline `308bc090` and code checkpoint `04abfd8d`,
release libraries, Rust 1.99.0, and macOS 27.2. Builds and tests finish before
timing. Both revisions use identical probe source and compiler options.

The standard facade gate uses six alternating pairs per workload at 32 MiB,
both with and without damage consumption. Focused probes are checked in under
[`scripts/terminal-engine-audit`](../../scripts/terminal-engine-audit/README.md)
and run in baseline/candidate/candidate/baseline order. Raw logs, binary/source
hashes, and the measurement manifest are retained with the local evidence.

Hosted macOS CI produced 8–24% sample variation and inconsistent per-case
failures across two runs of identical production code. Its facade gate now uses
twelve alternating 128 MiB pairs to improve temporal averaging; Linux retains
six 32 MiB pairs. Both retain the same 0.95 threshold, every raw sample, and
identical baseline/candidate workloads. The local results below use the original
six-pair 32 MiB method and do not substitute for the larger CI run.

## Initial repair checkpoint results

Median elapsed times; lower is better. Focused probes ran in ABBA order with
12 samples per compaction case, 10 per graphics case, and 8 per history case.

| Workload | Baseline | Candidate |
| --- | ---: | ---: |
| 300,000 short plain feeds after compaction | 45.80 ms | 31.53 ms |
| 300,000 alternating-style feeds after compaction | 1,049.45 ms | 889.36 ms |
| Styled feed allocator requests after compaction | 2,100,000 / 1,192,800,000 cumulative bytes | 0 / 0 bytes |
| 2,000 borrowed viewport reads + feeds, 1,000 history rows | 4.73 ms | 3.31 ms |
| Same reads + feeds, 20,000 history rows | 40.05 ms | 3.42 ms |
| Chunked upload with virtual prototype, 120×40 | 2.535 ms | 1.423 ms |
| Same upload, 240×80 | 4.920 ms | 1.354 ms |
| Same upload, 1,024×1,024 | 247.683 ms | 1.219 ms |

The post-idle packing penalty is removed without abandoning quiet compaction.
The large-history read case is about 11.7× faster; the extreme-grid upload case
is about 203× faster. These gains apply to these specific regressions, not
terminal rendering in general.

The six instrumented, warmed 32 MiB allocation workloads all made zero allocator
requests. The standard facade throughput ratios below compare candidate to
baseline; higher is better. All 14 local measurements meet the existing 0.95 gate.

| Facade workload | Feed only | With damage consumption |
| --- | ---: | ---: |
| Plain text | 0.989× | 0.984× |
| Styled redraw | 1.051× | 1.058× |
| Repeated Unicode | 0.972× | 0.981× |
| Varied Unicode | 0.982× | 0.977× |
| Repeated combining marks | 0.993× | 0.983× |
| Varied combining marks | 0.977× | 0.995× |
| One-byte fragmented text | 1.016× | 1.004× |

Passing that gate does not establish throughput parity. Some local medians are
up to 2.8% lower. The focused dense plain control measured 30.67→31.54 ms
(+2.84%), while dense styled output improved 958.93→885.99 ms (-7.60%). Sample
dispersion and the full paired results are preserved in the
[measurement data](terminal-engine-audit-results-2026-10-07.json).

Final validation caught a styled-output regression from inlining rare parser
cleanup into the CSI byte path. Isolating string transitions and large-buffer
release removed four extra register saves/restores per CSI byte on the measured
ARM64 build, bringing its stack frame back from 48 to 16 bytes. The final styled
results above include that repair. Graphics also coalesces content invalidation
and excludes cursor-only damage while preserving real placeholder edits.

## Final Unicode throughput follow-up

Three hosted macOS runs of identical binaries exposed a remaining Unicode
damage-consumption slowdown: their pooled median paired ratio was 0.943.
The longer run also failed plain text amid substantial timing dispersion.
The follow-up resolves uniform width and classification ranges together and
removes a redundant old-cursor snapshot from scalar cell writes. Both wrapping
paths already mark that cursor cell before moving or scrolling; grapheme
extension retains its separate cursor mark. Every valid Unicode scalar is
checked against the original width and classification functions. No cache or
additional per-terminal memory is introduced.

All 214 engine tests and all 14 local throughput gates pass for this follow-up.
The comparison again uses baseline `308bc090`, six alternating 32 MiB pairs,
identical helper source, and no overlapping builds or tests. These facade
results supersede the facade table at the earlier checkpoint; the focused
regression measurements above remain explicitly attributed to `04abfd8d`.

| Facade workload | Feed only | With damage consumption |
| --- | ---: | ---: |
| Plain text | 0.993× | 0.997× |
| Styled redraw | 1.051× | 1.055× |
| Repeated Unicode | 0.951× | 1.073× |
| Varied Unicode | 0.965× | 1.087× |
| Repeated combining marks | 1.046× | 1.015× |
| Varied combining marks | 1.038× | 1.027× |
| One-byte fragmented text | 1.021× | 0.997× |

Feed-only Unicode remains up to 4.9% below baseline, close to the existing 5%
tolerance; passing does not establish parity. The
[follow-up data](terminal-engine-audit-unicode-followup-2026-10-07.json)
retains every pair and identifies the candidate through its parent revision,
modified source hash, and binary/library hashes. The PR checks track the
hosted result for the final head.

These measurements cover parser/grid work, allocator requests, and graphics
assembly. They do not measure presented frames, input-to-display latency, or
OS RSS. This patch does not establish that the earlier native graphics hitch
is resolved; the [previous presentation measurements](unicode-history-performance-2026-10-05.md)
remain a separate limitation.

## Application latency measurement follow-up

The final engine revision passed hosted architecture checks and all parser,
allocation, and facade throughput gates. The application echo-train check
failed twice: baseline/candidate p95 render-callback latency was 20.69/89.07 ms
and 18.13/33.48 ms. These results remain failures, not evidence of parity.

Investigation found that the driver timestamped output before serializing and
flushing its marker file. That filesystem work therefore counted as terminal
latency. The driver now prepares each payload, timestamps immediately before
writing and flushing stdout, and persists the original timestamps after the
measurement interval. The same correction applies to idle-burst. Trace retries
clear earlier application metrics, and CI retains raw frame and timeline events
alongside markers so future tails can be investigated. The latency threshold
and percentile calculation are unchanged. The earlier artifacts did not retain
the raw frames, so they cannot establish how much of either tail came from
marker overhead. Render-callback timing still does not measure presentation
or match a specific glyph to a displayed frame.
