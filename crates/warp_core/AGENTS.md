# crates/warp_core — Foundational types & registration macros

Foundational crate: re-exported widely. Touch with care — changes here ripple through `app/` and most other crates.

`lib.rs` exports: `app_id`, `assertions`, `channel`, `command`, `context_flag`, `errors`, `execution_mode`, `features`, `interval_timer`, `macos` (cfg-gated), `operating_system_info`, `paths`, `platform`, `safe_log`, `semantic_selection`, `host_id`, `session_id`, `sync_queue`, `telemetry`, `ui`, `user_preferences`. Also re-exports the `settings` framework and its macros.

## DO NOT implement these traits directly — use the macros

Two registration traits are **deliberately gated** behind macros so that registries stay consistent and discoverable.

### `RegisteredTelemetryEvent` — use `register_telemetry_event!`

File: `crates/warp_core/src/telemetry.rs:78`.

```rust
// WRONG
impl RegisteredTelemetryEvent for MyEvent { /* ... */ }

// RIGHT
register_telemetry_event!(MyEvent { /* fields */ });
```

The macro wires the event into the global registry, ensures redaction hooks are evaluated, and prevents the `app/src/server/telemetry/events.rs` discriminant match from going stale (that match is **exhaustive — no `_` arm allowed**, see line 5032).

### `RegisteredError` — use `register_error!`

File: `crates/warp_core/src/errors/registration.rs:20`.

Mechanically the same pattern: register error variants through the macro so they get IDs, redaction, and Sentry classification.

## Feature flags — DO NOT add to this crate

Feature flags live in `crates/warp_features/`, **not** here. Older docs (and even WARP.md prose) sometimes claim features.rs lives in `crates/warp_core/src/features.rs`; the actual `FeatureFlag` enum is in `crates/warp_features/src/lib.rs`. See `crates/warp_features/AGENTS.md`.

## Channels (`channel/mod.rs`)

`channel/mod.rs:16-19` enumerates allowed Warp channels. **Do not add internal-only channel variants**; that file is shared with the OSS build.

## Errors (`errors.rs`)

`errors.rs:10` notes: do not import `#[doc(hidden)]` internals from this module from outside the crate. Use the public surface or a `register_error!` invocation.

## Paths / platform / `instant`

- `paths` — channel-aware data and config directories.
- `platform::SessionPlatform::default_line_ending()` — use this instead of `line_ending::LineEnding::from_current_platform`, which mishandles Unix-like Windows subsystems (banned in `.clippy.toml`).
- `instant::Instant` (re-exported indirectly) — never use `std::time::Instant` because the WASM target lacks it.

## Tests

Unit tests are file-adjacent (`*_tests.rs`). Run:

```bash
cargo nextest run -p warp_core
```

If you change a registration macro, also build `app/` to confirm the registry stays consistent.
