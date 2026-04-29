# app/ — Main binary

This is the main Warp binary crate. ~1940 Rust files. Five channel binaries point at the shared `warp::run()` library entry point.

## Binaries

`app/src/bin/` selects the channel:

| File | Cargo bin | Channel |
|---|---|---|
| `local.rs` | `warp` | Local (developer-internal, requires `warp-channel-config` on PATH) |
| `oss.rs` | `warp-oss` | Open source build |
| `dev.rs` | `dev` | Dev channel |
| `preview.rs` | `preview` | Preview channel |
| `stable.rs` | `stable` | Stable channel |

`./script/run` picks `warp` vs `warp-oss` automatically based on whether the `warp-channel-config` shim is on `PATH`.

## Startup path

```
src/bin/<channel>.rs  →  warp::run()  (src/lib.rs)
                         ├─ platform init
                         ├─ feature flags
                         ├─ App builder + AppContext
                         ├─ settings load
                         ├─ telemetry transport
                         └─ root_view::RootView (src/root_view.rs)
```

## Module layout (`app/src/`)

Top-level subsystems:

- **AI / agents** — `ai/` (deep tree; see `app/src/ai/AGENTS.md`), `ai_assistant/`
- **Auth & billing** — `auth/`, `billing/`, `pricing/`, `usage/`
- **Cloud / sync** — `drive/`, `cloud_object/`, `external_secrets/`, `crash_reporting/`
- **Editor / code** — `editor/`, `code/`, `code_review/`, `notebooks/`, `completer/`
- **Search & navigation** — `search/`, `coding_entrypoints/`, `command_palette.rs`, `suggestions/`
- **Server / telemetry / IPC** — `server/` (telemetry transport + GraphQL), `remote_server/`, `plugin/`
- **Settings & themes** — `settings/` (see its AGENTS.md), `settings_view/`, `themes/`, `user_config/`
- **Terminal** — `terminal/` (see its AGENTS.md), `default_terminal/`, `env_vars/`
- **UI scaffolding** — `view_components/`, `ui_components/`, `pane_group/`, `tab_configs/`, `workspace/`, `workspaces/`, `banner/`, `chip_configurator/`, `context_chips/`, `resource_center/`, `tips/`, `quit_warning/`, `undo_close/`
- **Persistence** — `persistence/` (see its AGENTS.md)
- **Platform / system** — `platform/`, `system/`, `app_services/` (linux + windows), `antivirus/`, `autoupdate/`, `login_item/`, `prevent_sleep/` (via crate)
- **Test infra** — `integration_testing/`, `test_util/`

Plus `examples/`, `tests/`, `assets/`, `channels/`, `resources/`, `DockTilePlugin/` (macOS native), `build.rs`.

## `app/src/lib.rs` guardrail (CRITICAL)

```
// PLEASE DO NOT ADD MORE PUBLIC MODULES!
```

The comment near line 101-106 forbids adding new public modules. New subsystems live as private modules and expose minimal public APIs. If you genuinely need a public module, raise it in review first; don't ninja it through.

## Other landmines specific to `app/`

- **`app/src/workspace/global_actions.rs:68-69`** — do not add new global actions; use **typed actions** instead.
- **`app/src/root_view.rs:134-137`** — do not use `unthemed_window_border()` for themed views.
- **`app/src/menu.rs:1270-1271`** — `MenuItem::submenu` is currently disabled / not ready; do not use.
- **`app/src/server/server_api.rs:93-100`** — eval-user list is intentional; do not change or remove entries.
- **`app/src/server/telemetry/events.rs:5032-5035`** — do not introduce wildcard `_` arms when matching on telemetry discriminants; enumerate every variant and feature-flag explicitly. New telemetry events must be added through `register_telemetry_event!` (see `crates/warp_core/AGENTS.md`).
- **`app/src/auth/auth_manager/user_persistence.rs:25-29`** — do not reintroduce a top-level `refresh_token` field; use `auth_tokens.refresh_token`.
- **`app/src/settings_view/mod.rs:387-388`** — legacy SSH wrapper flag is gone; use `SSH_TMUX_WRAPPER_CONTEXT_FLAG`.

## Cargo.toml

`app/Cargo.toml` is large (~30K) with many feature flags. Common ones:
- `gui` (default for `./script/run`)
- `with_local_server` (binds to local backend; sets `WITH_LOCAL_SERVER=1` env when invoked via `./script/run`)
- `with_local_session_sharing_server`
- `with_sandbox_telemetry`
- `test-util` / `integration_tests` (test-only surfaces)

## Tests

- **Unit tests**: source-adjacent `*_tests.rs` / `mod_test.rs`, included via `#[cfg(test)] #[path = "..."] mod tests;`. Use `App::test` for UI/model logic.
- **Integration**: tests live in `crates/integration/` (see its AGENTS.md). The `app/src/integration_testing/` module exposes test hooks.
- **Reference / golden**: `app/src/terminal/ref_tests/`, `app/src/persistence/sqlite_tests.rs`.
- Helpers: `app/src/test_util/mod.rs`, `app/src/test_util/terminal.rs`.

Run from repo root: `cargo nextest run -p warp` (or `--workspace --exclude command-signatures-v2`).
