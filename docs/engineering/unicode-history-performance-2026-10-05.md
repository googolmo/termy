# Unicode, CJK rendering, and scrollback follow-up

This follow-up to the custom terminal engine keeps the existing terminal facade
and C ABI. It changes text storage/rendering and adds native presentation probes.

## Behavior

- Streaming graphemes retain their complete text in one leading cell. Emoji
  presentation selectors, skin tones, ZWJ families, regional-indicator flags,
  keycaps, combining marks, and decomposed Hangul preserve terminal column widths.
  Tests cover every input split, one-byte feeds, insert mode, erasing either wide
  half, right-margin promotion, SGR, and one-column reflow.
- CJK runs share a shaping call across wide-character spacers. The renderer maps
  shaped UTF-8 clusters back to the engine's cell columns, preserving combining
  offsets and fallback font runs. ASCII batches keep their existing shaping path.
- Live rows and sustained output use dense cells. After 250 ms without additional
  history rows, the runtime compacts at most 256 cold rows per maintenance step.
  Further steps are spaced 8 ms apart. Once settled, small output updates can
  recycle compact history storage. Large output batches return to dense storage.
  The maintenance timer sleeps when there is no work; compaction creates no
  render damage or viewport generation change.
- Compact rows contain scalar/flag arrays, style runs, and shared sparse metadata.
  Rows that would not save enough space remain dense. Public borrowed row reads
  materialize only requested history rows; output/resize/scroll releases those
  read caches. Search and full-buffer visitors reuse scratch storage. Direct
  engine embedders can call `Engine::compact_history()` after an output burst.

## Native presentation probe

Run on a visible macOS desktop with Xcode's `xctrace` available:

```sh
cargo run --release -p termy_cli --bin xtask -- benchmark-record \
  --target termy:/absolute/path/to/termy \
  --output /absolute/path/to/new-report-directory \
  --duration-secs 8
```

The default scenarios are `cjk-scroll`, `heavy-tui`, `resize`, and `graphics`.
Use repeated `--scenario` arguments to select a subset. Resize changes the native
window bounds while output continues; graphics uploads and moves a Kitty image.
Benchmark mode owns an isolated window, so an installed Termy instance can remain
open. The command refuses an existing output directory.

`presentation.json` contains application CPU/memory/callback metrics separately from
Animation Hitches' `displayed-surfaces-interval` timestamps, filtered by the
launched PID. The steady presentation window excludes one second at each end.
It reports displayed FPS, p50/p95/p99 intervals, and 60/120 Hz budget comparisons.
Budget overruns allow 1 ms of timestamp jitter. Estimated unfilled refresh slots
are derived from interval lengths, not GPU fence deadlines. A 120 Hz capture
compared with a 60 Hz budget is not a separate physical 60 Hz run. Missing frame
capture stays missing and makes `benchmark-record` fail instead of fabricating FPS.
Raw trace files remain local because they can contain other process metadata.

## Local measurements

Measured on an Apple M4 Mac with its built-in display reporting 120 Hz. The
reference is main at `b99fbc385bb314dd102affe6653d83ac489f9b8d`, with the previous
legacy-module removal preserved. Measurements use this working tree, not a released build.

### Parser throughput

Three alternating baseline/candidate pairs, 64 MiB per case, release builds,
120 × 40 cells and 1,000 history rows. Builds and native captures did not overlap
timed parsing. These are local medians, not cross-platform guarantees.

| Case | Baseline MiB/s | Candidate MiB/s | Change |
| --- | ---: | ---: | ---: |
| plain | 430.14 | 406.87 | -5.4% |
| styled | 219.89 | 225.30 | +2.5% |
| unicode | 205.84 | 192.57 | -6.4% |
| varied-unicode | 240.56 | 228.38 | -5.1% |
| combining | 159.34 | 150.34 | -5.6% |
| varied-combining | 161.59 | 156.61 | -3.1% |
| fragmented | 24.95 | 25.58 | +2.5% |

The added Unicode/state handling has a measurable cost: bulk cases are about
3–6% slower here, while styled/fragmented cases are slightly faster. This change
is not an across-the-board CPU-throughput win. CJK shaping is batched and idle
history is smaller; parsing still avoids warmed allocations.

### Retained heap

The instrumented engine benchmark feeds 8 MiB per case, then explicitly settles
history. Values include the live screen and benchmark process heap, not just
scrollback, and are not desktop RSS. Compaction was outside the parser timing.

| Case | Baseline retained KiB | Settled candidate KiB | Reduction |
| --- | ---: | ---: | ---: |
| plain scroll / 64KiB | 4009 | 900 | 77.6% |
| styled TUI / 64KiB | 219 | 219 | 0.0% |
| Unicode / 64KiB | 4009 | 759 | 81.1% |
| varied Unicode / 64KiB | 4121 | 1215 | 70.5% |
| combining / 64KiB | 4009 | 673 | 83.2% |
| plain scroll / 1 byte | 4009 | 900 | 77.6% |

All six warmed allocation workloads performed **zero allocations during parsing**.
Settling 1,000 rows took well under 1 ms in these runs. Highly decorated rows
remain dense if packing would not halve their retained cell-storage cost.

### Native presentation and application resource use

Eight-second scenarios on the built-in 120 Hz display. FPS and frame intervals
come from actual displayed surfaces, with one second trimmed from each end.
CPU is the duration-weighted Activity Monitor value; memory is its maximum
physical footprint, not RSS. CPU/memory come from a separate capture of the same
workload and include its startup/shutdown samples. They are not idle values.

| Scenario | Presented FPS | p95 ms | p99 ms | CPU % | Peak footprint MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| cjk-scroll | 119.31 | 8.33 | 8.33 | 17.22 | 177.66 |
| heavy-tui | 120.00 | 8.33 | 8.33 | 22.72 | 171.69 |
| resize | 116.75 | 8.33 | 16.67 | 21.78 | 167.88 |
| graphics | 73.17 | 16.67 | 25.00 | 21.34 | 181.94 |
| graphics (repeat) | 119.66 | 8.33 | 8.33 | 21.87 | 172.45 |

| Scenario | Intervals | Over 60 Hz budget | Over 120 Hz budget |
| --- | ---: | ---: | ---: |
| cjk-scroll | 691 | 0 | 4 |
| heavy-tui | 716 | 0 | 0 |
| resize | 719 | 0 | 20 |
| graphics | 425 | 11 | 37 |
| graphics (repeat) | 696 | 0 | 2 |

The graphics result includes a recorded 325 ms hitch and is substantially worse
than the earlier validation capture's 119.32 FPS. Application render callbacks
remained near 113 FPS in both runs, illustrating why callback rate alone cannot
establish displayed smoothness. The cause of the presentation gap is not yet
established. One repeat of the final build reached 119.66 FPS with no recorded
hitch and an 8.33 ms p99 interval. Both final-build captures are retained; this
variation does not establish consistently meeting 120 Hz.

These captures have no native before/after baseline and do not establish an FPS
improvement over Alacritty. They cover one macOS machine, not Windows/Linux or a
physical 60 Hz display. The final raw evidence is local under
`target/engine-inspection/native-final/` and
`target/engine-inspection/native-graphics-repeat/`; parser comparisons are in
`target/engine-inspection/paired-facade-final.json`.

### Validation

- Full release workspace tests passed; pre-existing ignored tests remain marked.
- All 11 explicitly enabled tmux integration cases passed.
- Strict workspace/all-target Clippy, formatting, and architecture boundaries passed.
- Native visual QA checked aligned CJK/emoji/combining columns, mixed styles,
  underlines, and a window resize.
- Public C headers/flat cell ABI are unchanged. Full cluster text is available in
  the Rust render-text API; the existing flat C cell still exposes its base scalar.
