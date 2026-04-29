# crates/ — Workspace crate index

63 crates. The workspace `Cargo.toml` lists `crates/*` plus `app`. Default-members narrow the set built/tested by default and **exclude `serve-wasm` and `integration`** (their build/run is driven explicitly via scripts and the integration binary).

## Sub-AGENTS.md (read these first if you're working in them)

| Crate | Why it has its own AGENTS.md |
|---|---|
| `warp_core/` | Telemetry / error registration macros, channels, paths, platform |
| `warp_features/` | `FeatureFlag` enum + rollout sets; canonical place to add flags |
| `warpui_core/` | Custom UI framework: Entity-Component-Handle, Elements, scene/presenter |
| `warpui/` | wgpu rendering + winit windowing + per-platform impls (mac/linux/windows/wasm) |
| `warp_terminal/` | Terminal emulation crate (separate from `app/src/terminal/`) |
| `ai/` | Shared AI types, agent action models, indexing, skills runtime |
| `integration/` | Builder/TestStep framework — registration in two places is mandatory |
| `persistence/` | Diesel migrations directory (~110 migrations) |

## Crate categories

### Core platform / infra
- `warp_core` — IDs, channels, paths, telemetry, errors, settings re-exports, OS info
- `warp_features` — `FeatureFlag` enum + rollout sets
- `warp_util` — small utilities
- `warp_logging` — `tracing` configuration
- `warp_files` — file abstractions
- `simple_logger`
- `channel_versions` — channel version metadata
- `app-installation-detection`

### UI framework
- `warpui_core` — framework primitives (entity, view, model, scene, elements, fonts, keymap)
- `warpui` — rendering + windowing implementation; 23 examples in `warpui/examples/`
- `warpui_extras` — auxiliary widgets
- `ui_components` — shared higher-level components

### Terminal & shell
- `warp_terminal` — terminal model + shell adapters
- `command` — `command::blocking::Command` and `command::r#async::Command` (use these instead of `std::process::Command`)
- `editor` — text editor crate (block list, blocks, grid)
- `vim` — vim mode
- `command-signatures-v2` — checked-in JS build artefacts; nested `js/` package, not pure Rust

### AI / agents
- `ai` — shared agent types, action/action-result models, indexing (file outline, embeddings), project context, skills
- `computer_use` — computer-use agent surface
- `warp_completer` — command/code completion engine; **excluded from main clippy/test**, has WIP `v2` feature, runs separately
- `input_classifier` — natural-language vs command classification

### Server / network / IPC
- `graphql` — generated GraphQL bindings (Cynic) + schema mirror
- `warp_graphql_schema`
- `warp_server_client`
- `remote_server` — remote agent server
- `serve-wasm` — WASM dev server (excluded from default-members)
- `http_client`, `http_server`, `websocket`, `jsonrpc`
- `ipc`
- `firebase`
- `field_mask`
- `warp_web_event_bus`

### Persistence / storage
- `persistence` — Diesel migrations live here (`migrations/`)
- `settings` — settings framework; app-side schema is in `app/src/settings/`
- `settings_value`, `settings_value_derive`
- `asset_cache`, `asset_macro`
- `virtual_fs`
- `managed_secrets`, `managed_secrets_wasm`

### Tooling / dev
- `integration` — integration-test binary crate (Builder/TestStep)
- `onboarding`
- `warp_cli`
- `repo_metadata` — repo introspection
- `warp_ripgrep` — ripgrep wrapper
- `node_runtime`
- `lsp` — LSP integration; ships a `rust-lsp` example binary
- `languages` — language-grammar/registry
- `markdown_parser`
- `syntax_tree`
- `string-offset`
- `sum_tree`
- `fuzzy_match`
- `handlebars`
- `voice_input`
- `prevent_sleep`
- `isolation_platform`
- `natural_language_detection`
- `warp_js`
- `watcher`

## Adding a new crate

1. `cargo new --lib crates/<name>` (or `--bin`).
2. Add to workspace `Cargo.toml`. The wildcard `crates/*` membership picks it up automatically; if you also want it in default-members, add it explicitly to that list.
3. License: AGPL-3.0-only by default. UI crates (`warpui_core`, `warpui`) are MIT — keep `license = "MIT"` if you're under the UI tree.
4. Reuse workspace dependencies via `<dep>.workspace = true` rather than re-pinning versions.
5. If the crate needs a custom toolchain target (WASM, etc.), follow patterns in `warpui` and `serve-wasm`.

## Running tests for one crate

```bash
cargo nextest run -p <crate> --locked
cargo nextest run -p warp_completer --features v2     # special-cased: WIP feature path
```

Workspace-wide tests intentionally exclude `command-signatures-v2`; respect that exclusion when adding test commands.
