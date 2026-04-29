# app/src/settings — Warp app settings

The app-side settings tree. The framework lives in `crates/settings/`; this directory provides the actual schema, defaults, sync, validation, and onboarding logic for Warp.

## DO NOT add fields to `Settings` directly

`mod.rs:246-249` is the canonical place where the rule is documented. **Always go through the macros in `app/src/settings/macros.rs`**. The macros wire:

- Default values
- Schema validation (`schema_validation_tests.rs` will fail if you skip them)
- Cloud sync hooks (`cloud_preferences_syncer.rs`)
- Per-channel overrides
- Telemetry redaction for sensitive entries

If you add a field by hand to the `Settings` struct, you'll bypass all of the above and break sync silently.

## Notable files

- `mod.rs` — top-level Settings type, public API
- `macros.rs` — **the only sanctioned way to declare a setting**
- `init.rs`, `initializer.rs` — startup loading & migration of older settings shapes
- `manager.rs`
- `schema_validation_tests.rs` — verifies schema integrity
- `cloud_preferences.rs`, `cloud_preferences_syncer.rs` — Warp Drive sync (note: `cloud_preferences_syncer_tests.rs` is ~66K, treat sync as a critical path)
- `ai.rs`, `editor.rs`, `font.rs`, `gpu.rs`, `input.rs`, `theme.rs`, `privacy.rs`, `onboarding.rs`, `code.rs` — domain-specific setting groups
- `import/` — settings import from other terminals
- `ssh.rs`, `emacs_bindings.rs`, `vim_banner.rs` — small specific surfaces

## Onboarding & first-run

`onboarding.rs` is paired with the `onboarding` crate. Changes to first-run defaults usually need to be reflected in both.

## Privacy / GDPR

`privacy.rs` is ~35K; redaction and consent toggles live there. Anything that adds a setting controlling telemetry needs to be threaded through there and through `app/src/server/telemetry_ext.rs`.

## Cross-references

- Cross-channel concerns: `crates/warp_features/` for feature flags; `crates/warp_core/src/channel/` for channel detection.
- Settings view UI is in `app/src/settings_view/` (a different directory). The legacy SSH wrapper toggle lives there at `mod.rs:387-388` — use `SSH_TMUX_WRAPPER_CONTEXT_FLAG`, not the old flag name.

## Tests

```bash
cargo nextest run -p warp settings::
```

`schema_validation_tests.rs` and `init_tests.rs` are the most important to run after any schema change.
