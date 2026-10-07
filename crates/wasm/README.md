# termy_wasm

WebAssembly bindings for Termy's headless terminal engine, consumed by the
`@termysh/*` npm packages in `packages/`.

## Owner

This crate owns the wasm-bindgen surface over `termy_core` built with
`default-features = false`: byte feeding, resize, damage, flat cell reads,
protocol replies, events, keyboard/mouse/paste encoding and built-in themes.

It has no PTY, filesystem or DOM access. Rendering, input capture and transport
belong to the TypeScript packages; terminal semantics belong to `termy_core`.

## Validation

```sh
cargo test -p termy_wasm
cargo build -p termy_wasm --target wasm32-unknown-unknown --release
```

## Forbidden Dependencies

- `gpui`
- `termy` / `crates/desktop_app`
- `web-sys` (DOM access belongs to `@termysh/web`)
