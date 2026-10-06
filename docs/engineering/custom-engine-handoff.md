# Custom terminal engine: cloud handoff and completion

Continue on `feat/custom-terminal-engine` in
[PR #400](https://github.com/lassejlv/termy/pull/400). The user requested this
checkpoint so implementation could continue in the cloud. The replacement is
complete; the cloud results and measurement limits are recorded below.

## Scope and constraints

- Replace Alacritty everywhere with `crates/core/src/terminal_engine`, including
  native PTYs, display terminals, desktop tmux panes and persistent sessions.
- Keep performance and bounded memory central to the implementation.
- The former experimental display implementation, including its tests and
  tooling, was removed after the engine replacement.
- The user explicitly waived the repository's Grok-only model requirement for
  this work. Do not ask for that approval again.
- Do not record videos. Do not merge the PR unless the user requests it.

## Current implementation

The replacement is integrated throughout the application. Alacritty's Cargo
dependencies, runtime adapters, desktop bridge, conversion helpers, comparison
examples and engine-selection switches are removed. A boundary check rejects
reintroducing the dependency. Historical documentation still records the former
engines and their performance comparisons.

The new engine has streaming VT/UTF-8 parsing, Unicode and combining text,
primary/alternate grids, bounded scrollback, reflow, protocol queries, clipboard
handling, synchronized output, Kitty graphics and Unix/Windows PTY transport.
Performance work includes ASCII runs, recycled rows, ordered incremental scroll
damage, borrowed viewport access, one-pass clipboard parsing, bounded resize
buffers and event payloads, and allocation-free image revision polling.

See the [engine design](../../crates/core/src/terminal_engine/README.md) and
[measured performance report](custom-engine-2026-10-05.md) for implementation
limits and measured regressions as well as improvements.

## Validation at code checkpoint `427677cb`

- All Linux, macOS and Windows core checks, both desktop platform checks,
  workspace tests, tmux integration, strict Clippy, formatting and architecture
  boundaries passed in GitHub Actions.
- Local full suites passed: 717 core unit tests and 46 core integration tests,
  878 desktop tests including 17 pane tests. The final engine overload regression
  then passed with all 158 engine-filtered tests, bringing core unit coverage to
  718 passing tests. Existing ignored tests remain ignored.
- Manual native application checks passed for shell input, styles, wide and
  combining Unicode, synchronized Kitty layout/scrolling/animation, resize and
  a real two-pane tmux session. The isolated QA app and tmux server were closed.
- The parser/grid allocation benchmark passed with zero warmed allocations in
  each of its five workloads. This is a feed-only benchmark, not rendering proof.
- CI idle-blink and idle-burst performance jobs passed. Echo-train and
  steady-scroll failed because baseline `xctrace` exceeded its 63-second outer
  timeout before candidate measurement. This handoff adds a separate 60-second
  startup allowance while retaining the existing 45-second finalization budget,
  workload durations, retries and performance thresholds. All 24 CLI benchmark
  tests and the formatting check pass with that change.

## Cloud completion

- Audited the workspace, Cargo.lock, native/display construction, tmux panes and
  persistent-session construction: no active Alacritty dependency or adapter
  remains. Historical reports retain the former display backend comparisons.
- Strengthened the dependency boundary to reject Alacritty in every workspace
  package, feature, platform and normal/build/dev dependency section. A long
  dependency-output fixture verifies that an early search exit cannot hide it.
- All checks at `b651c7bd` passed, including all four macOS performance scenarios.
  Echo-train and steady-scroll now finish with the bounded startup allowance.
  See [the tracing run](https://github.com/lassejlv/termy/actions/runs/37296537917)
  and the PR for final-head checks; earlier runs do not prove a later revision.
- Implemented and evaluated the proposed 2 KiB width cache, including exhaustive
  scalar/hit/miss tests and collisions. Rejected it after six alternating pairs:
  repeated Unicode improved only 1.6%, while varied Unicode fell 8.6% and
  combining fell 6.4% by median. Cloud timings varied substantially. The final
  engine retains its original width lookup and memory footprint.
- Added a 16,384-distinct-scalar Unicode workload to the committed allocation
  benchmark and its existing CI gate. Both repeated and varied Unicode must be
  measured for future proposals. All six workloads retained zero warmed
  allocations in the cloud experiment.
- Local Linux core tests passed (710 unit tests and 46 integration tests;
  existing ignores retained), as did CLI tests, formatting and architecture
  boundaries. The prototype's 160 engine tests included two exhaustive/cache
  regressions; the retained engine still has its original 158 engine tests.
- Updated the performance report with the rejected experiment and hosted echo
  evidence, preserving the former experimental display comparison and its
  substantial retained-memory/throughput tradeoffs.

## Performance follow-up

The subsequent performance work replaces repeated Unicode dispatch, redundant
wide-cell writes, full-row recycling clears and tiny combining-key library
comparisons. The [throughput follow-up](custom-engine-throughput-2026-10-05.md)
records six alternating Linux pairs against Alacritty: all seven workloads are
ahead by median, with 1.36–4.80× feed throughput and 1.20–4.95× with damage
consumption. All six warmed allocation gates still allocate zero times, and
six paired real-PTY runs show no median latency regression in that probe.

Identical-source, uninstrumented facade comparisons now run in PR CI on Linux
and macOS. Preserve the raw measurements and inspect the actual baseline engine;
future custom-to-custom comparisons do not prove an Alacritty comparison.
The PR description records the final verified CI revision and results.

## Measurement limits and future performance work

The replacement does not establish a universal performance improvement. The
measured Unicode throughput gap is addressed; compact history remains an
optimization opportunity, and the tested width cache is still rejected. Native
Alacritty gains must not be presented as gains over the former experimental
display facade.

The hosted echo run recorded 40 samples and zero missed echoes per binary, but
its displayed-frame samples are unavailable. Render callbacks do not establish
presented-frame or key-to-photon latency. This cloud executor is Linux and cannot
perform paired focused native macOS window measurements. For future measurement,
preserve `HOME`, isolate `XDG_CONFIG_HOME` and `TERMY_INSTANCE_HOME`, focus the
window, then release an explicit workload start gate. Do not claim those
measurements are complete or replace them with a virtual-display smoke test.

The PR must remain unmerged unless the user asks to merge it. Check final-head
CI before review/merge; the PR description records the latest verified status.

Local raw measurements, saved binaries and experimental scripts under
`target/custom-engine-2026-10-05/` are ignored build artifacts and are not part of
this Git checkpoint. The committed report preserves the completed results and
their limits; committed benchmark examples provide the reproducible core probes.
