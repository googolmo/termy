# xtask

Repository automation binary.

## Owner

This crate owns maintainer commands that generate or verify repository
artifacts, including generated documentation and desktop benchmark reports.

Keep product runtime code out of this crate. If an automation command needs
shared domain data, depend on the smallest domain crate that owns that data.

## Validation

```sh
cargo test -p termy_cli --bin xtask
cargo run -p termy_cli --bin xtask -- generate-keybindings-doc --check
cargo run -p termy_cli --bin xtask -- generate-config-doc --check
cargo run -p termy_cli --bin xtask -- check-dependency-policy
```

## Forbidden Dependencies

- `termy_core::ffi`
- `termy` / `crates/desktop_app`
- product runtime workflows

The headless terminal engine benchmark lives in `crates/core/examples/terminal_engine_bench.rs`
and runs with `just benchmark-terminal-engine`. It exercises the same engine as
the native and display runtimes without depending on another emulator.
