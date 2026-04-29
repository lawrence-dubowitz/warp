# crates/warp_features — Feature flag registry

Single source of truth for feature flags. The `FeatureFlag` enum lives in `src/lib.rs` (~40K). Every flag the app reads at runtime is a variant here.

(Older comments in `WARP.md` reference `crates/warp_core/src/features.rs` — that's stale. The current home is this crate.)

## Adding a new flag

1. Add a variant to `FeatureFlag` (kebab-cased docstring describing what it gates). The enum derives `Sequence` (from `enum_iterator`), so new variants are automatically iterated by tooling — no separate registration.
2. Pick a rollout list. The crate exposes four sets:
   - `DEBUG_FLAGS` — on in debug builds only
   - `DOGFOOD_FLAGS` — on by default for the Local / dogfood channel
   - `PREVIEW_FLAGS` — on for Preview
   - `RELEASE_FLAGS` — on for everyone (Stable)
3. Gate code at the call site: `FeatureFlag::YourFlag.is_enabled()`. **Prefer runtime checks over `#[cfg]`.**
4. Hide UI behind the same flag. If a setting/menu/keybinding shows up while the flag is off, the rollout isn't actually gated.
5. Once a flag is in `RELEASE_FLAGS` and stable, schedule its removal — flags are not permanent. The `.agents/skills/remove-feature-flag` skill walks through cleanup; the `.agents/skills/promote-feature` skill walks through advancing through the rollout sets.

## Test override hook

When the `test-util` feature is on, the crate exposes:

```rust
pub use overrides::{get_overrides, set_overrides};
```

Use this in tests to flip flags deterministically. Don't poke `AtomicBool` / `AtomicU8` internals directly.

## Use the skills

- `.agents/skills/add-feature-flag` — adding a new flag end-to-end.
- `.agents/skills/promote-feature` — moving Local → Dogfood → Preview → Stable, including any compile-time/runtime bridge needed before flag cleanup.
- `.agents/skills/remove-feature-flag` — removing a stabilised flag everywhere.

## Tests

Unit tests live next to `src/lib.rs` in `features_test.rs`. Run:

```bash
cargo nextest run -p warp_features
```
