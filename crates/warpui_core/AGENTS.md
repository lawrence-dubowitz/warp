# crates/warpui_core — UI framework core

The Entity-Component-Handle UI framework that powers Warp. This crate is **MIT-licensed** (the rest of the workspace is AGPL-3.0-only). Keep that boundary clean: nothing in `warpui_core` should depend on AGPL crates.

187 Rust files across these areas:

- `assets/` — fonts, icons, asset loading
- `async/` — runtime adapters (`native/`, `wasm/`)
- `core/` — autotracking, model, view machinery
- `debug/` — frame inspector / debug overlays
- `elements/` — Element implementations (Flutter-inspired): `flex`, `stack`, `drag`, `new_scrollable`, `shimmering_text`, `table`
- `fonts/`
- `integration/` — hooks for integration tests (`crates/integration/` calls into here)
- `keymap/`
- `platform/test/` — test platform stubs
- `rendering/` — backend selection, scene assembly
- `telemetry/` — UI-side telemetry hooks
- `text/`
- `ui_components/` — primitive widgets
- `windowing/`

Plus top-level files: `presenter.rs` (views → Scene per frame), `scene.rs` (scene/layer primitives), `lib.rs`, etc.

## Mental model

Read the **Architecture Overview** in `WARP.md` first. Two-line summary:

- **Entity** owns state. **View** owns rendering of an entity. **Handle<T>** is a typed reference into the entity registry.
- During a render or event tick the framework gives you a `&mut AppContext` (commonly named `ctx`). `ctx` is **always the last parameter** of methods that take it, except for closure-taking functions where the closure is last. Never rename `ctx`.

## UI guidelines

If you are writing or reviewing UI code anywhere in the repo, read the **`warp-ui-guidelines`** skill (`.agents/skills/warp-ui-guidelines/`) up front. It catalogues conventions for theming, hit-testing, scrolling, drag-and-drop, etc., and many of those rules originate in this crate.

## Theming rules

- **Reuse shared button themes**; do not mutate shared theme implementations.
- The `unthemed_window_border()` helper in `app/src/root_view.rs:134-137` is for unthemed surfaces only — do **not** apply it to themed views.

## Tests

`integration/` exposes an in-process integration harness used by `app/src/integration_testing/` and the `crates/integration/` test binary. Don't break that surface lightly.

```bash
cargo nextest run -p warpui_core
```

Examples and end-to-end UI tests live in the sibling `warpui` crate.

## Coding style reminders

- Do not introduce path qualifiers (`module::Foo::call()`); import `Foo` at the top of the file.
- Inline format args: `format!("{value}")` not `format!("{}", value)`.
- Match exhaustively; do not add catch-all `_` arms when the framework is meant to enumerate every variant.
