# termy_core::search_engine

Reusable terminal search primitives.

## Owner

This module owns text matching and search state that can be shared by the desktop app, headless runtime, and tests. Keep GPUI rendering, selection visuals, and command-palette behavior outside this module.

Use this module when changing search matching semantics or reusable search state.

Terminal adapters use `SearchLineMapping` to map matched bytes back to physical
cells. Preserve each cell's full combining suffix, omit trailing wide spacers,
and record its actual column span. This handles hidden text, joined emoji,
separately rendered cells, and wide glyphs clipped to a one-column grid without
reconstructing layout from the resulting string. Ordinary ASCII cells need no
mapping entries. Plain-string callers can continue using the grapheme-width
fallback through `search_line` and `search`.

## Validation

```sh
cargo test -p termy_core
```

## Boundaries

GPUI and desktop application state. Shared helpers are sibling core modules.
