# Custom terminal engine: cloud handoff

Continue on `feat/custom-terminal-engine` in
[PR #400](https://github.com/lassejlv/termy/pull/400). The PR is intentionally a
draft. The user requested this checkpoint so implementation can continue in the
cloud; the remaining validation below is not complete.

## Scope and constraints

- Replace Alacritty everywhere with `crates/core/src/terminal_engine`, including
  native PTYs, display terminals, desktop tmux panes and persistent sessions.
- Keep performance and bounded memory central to the implementation.
- Leave the legacy `crates/core/src/tmon` implementation untouched. Only two
  legacy test files changed to remove their obsolete Alacritty oracle while
  retaining explicit expected-byte assertions. Tmon is outside the module tree.
- The user explicitly waived the repository's Grok-only model requirement for
  this work. Do not ask for that approval again.
- Do not record videos. Do not merge the PR unless the user requests it.

## Current implementation

The replacement is integrated throughout the application. Alacritty's Cargo
dependencies, runtime adapters, desktop bridge, conversion helpers, comparison
examples and engine-selection switches are removed. A boundary check rejects
reintroducing the dependency. Historical documentation and untouched legacy
Tmon comments still mention the former engine.

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

## Remaining work

1. Follow all checks on the newly pushed PR head to completion. Diagnose actual
   failures; do not treat the previous head's successful checks as proof for a
   later code revision. Confirm the tracing-timeout fix on hosted macOS.
2. Evaluate the measured mixed-Unicode throughput regression. An unimplemented
   proposal is a per-grid boxed cache of 512 `u32` entries (2 KiB), packing the
   full scalar and two width bits. Hash with wrapping multiplication by
   `0x9e3779b1`, use the top nine bits for the slot, and validate the complete
   scalar tag on every hit. Bypass ASCII and preserve the pinned width lookup on
   misses, including width 3 for U+17D8. Reflow would remain unchanged. This is
   only a hypothesis: compare repeated and cache-miss-heavy Unicode, ASCII,
   styled and combining workloads before adopting it. Exhaustive scalar and
   collision checks should verify identical width results.
3. Complete paired focused-app output/echo measurements if the cloud environment
   supports a real macOS window. Preserve `HOME`; isolate `XDG_CONFIG_HOME` and
   `TERMY_INSTANCE_HOME`, focus the window, then release an explicit workload
   start gate. Render callbacks do not establish presented-frame or
   key-to-photon latency. The current CI workflow disables Animation Hitches
   tracing and permits zero displayed-frame samples.
4. Retain the report's comparison with the former Tmon display facade. It has
   substantial throughput and retained-memory differences from the former
   native Alacritty backend; do not describe native gains as universal gains.
5. Update the report and PR description around the final measured implementation.
   Mark the PR ready when the remaining work and final checks are complete.

Local raw measurements, saved binaries and experimental scripts under
`target/custom-engine-2026-10-05/` are ignored build artifacts and are not part of
this Git checkpoint. The committed report preserves the completed results and
their limits; committed benchmark examples provide the reproducible core probes.
