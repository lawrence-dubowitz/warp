# crates/warpui — UI rendering & windowing

Companion crate to `warpui_core`. **MIT-licensed.** This is where the framework actually talks to the OS and the GPU.

155 Rust files. Top-level subdirs:

- `platform/` — per-platform implementations: `headless/`, `linux/`, `mac/`, `wasm/`, `windows/`
- `rendering/` — `atlas/` (texture atlas), `wgpu/` (GPU backend)
- `windowing/` — `winit/` integration

Plus 23 cargo examples in `examples/` (each with its own `main.rs`): `typed_actions`, `slider`, `animated-gradient-text`, etc. Run them with:

```bash
cargo run -p warpui --example <name>
```

These are the canonical reference for "how do I use Element X in isolation."

## GPU backends

`wgpu` selects the backend per platform:

- macOS → Metal
- Windows → DX12
- Linux → Vulkan
- WASM → WebGL/WebGPU
- Headless / debug → GLES

`crates/warpui_core/src/rendering/mod.rs` chooses the backend; the actual draw paths live here in `crates/warpui/src/rendering/wgpu/`.

WGSL shaders are formatted by `wgslfmt`. Presubmit runs:

```bash
find . -name "*.wgsl" -exec wgslfmt --check {} +
```

C / Objective-C glue (used by `mac/` and parts of platform code) is checked by `script/run-clang-format.py`.

## `dbg!` is banned (clippy)

`std::dbg!` is in `.clippy.toml`'s disallowed-macros list because it cannot ship in submitted code. Use `tracing::{trace, debug, info, warn, error}` instead.

## WASM gotchas

- `std::time::Instant` does not exist on `wasm32` — always use `instant::Instant`.
- Filesystem APIs need `local_fs` (or equivalent) gating in WASM-compiled code paths. The `fix-errors` skill describes the gating pattern.
- Spawning processes is also unavailable; the `command::blocking::Command` / `command::r#async::Command` wrappers are how native code does it.

WASM clippy in CI:

```bash
cargo clippy --locked --target wasm32-unknown-unknown --profile release-wasm-debug_assertions -- -D warnings
```

WASM dev run: `./script/wasm/run`. WASM dev server: `cargo run --release --package serve-wasm -- "$BUNDLE_DIR"`.

## Tests

```bash
cargo nextest run -p warpui
```

Integration coverage runs through `crates/integration/`; this crate's own examples double as smoke tests for the rendering pipeline.
