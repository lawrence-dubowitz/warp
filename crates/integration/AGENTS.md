# crates/integration — Integration test framework

Custom Builder/TestStep harness. **Excluded from `default-members`** — built/run via the `integration` binary or via the dedicated nextest filter.

Layout:

- `src/bin/integration.rs` — manual runner binary; **every test must be registered here**
- `src/builder.rs` — test Builder
- `src/step.rs` / `src/test.rs` — TestStep + the registered test bodies (this file is huge: ~290K)
- `src/util.rs`, `src/user_defaults.rs`
- `tests/` — nextest-discovered tests
  - `tests/common/mod.rs` — shared harness helpers
  - `tests/integration/*.rs` — `register_test!` macro lists (the **second** registration site)
  - `tests/data/` — fixture SQLite/YAML/markdown files
  - `tests/INTEGRATION_TESTING.md` — start here when adding a test

## Read the skill first

`.agents/skills/warp-integration-test/` is the authoritative how-to. It covers the Builder API, registration in two places, hermetic temp HOME, bootstrap waits, and rerun-on-precondition-fail semantics.

## Two-place registration (CRITICAL)

A new integration test must be registered in **both**:

1. The manual runner: `crates/integration/src/bin/integration.rs`.
2. A `register_test!` invocation in `crates/integration/tests/integration/*.rs`.

Forgetting either side will silently exclude the test from one of the two harnesses (manual run vs nextest CI).

## Hermetic environment — DO NOT touch real shell dotfiles

The framework constructs a temp HOME with synthetic rc files for each test. **Do not** read or write `~/.zshrc`, `~/.bashrc`, etc. from a test or a fixture. See `.agents/skills/warp-integration-test/SKILL.md:57`.

## Other rules from the skill

- `wait_until_bootstrapped_single_pane_for_tab(0)` **before** any assertions — asserting before bootstrap is a flake source. (Skill, line 249.)
- **Never retry to mask deterministic failures.** Fix the failure. (Skill, line 233.)
- Use the helper macros — `async_assert!`, `async_assert_eq!`, `integration_assert!`, `assert_eventually!`, `new_step_with_default_assertions`, `execute_command_for_single_terminal_in_tab` — instead of rolling your own polling loops.

## Running

CI splits this package into two slices:

```bash
# Everything except shell integration
cargo nextest run --workspace --locked --exclude command-signatures-v2 -E "package(integration) and not test(shell_integration_tests)"
# Just shell integration
cargo nextest run --workspace --locked --exclude command-signatures-v2 -E "package(integration) and test(shell_integration_tests)"
```

Locally you can also drive the manual binary:

```bash
cargo run -p integration --bin integration -- <test-name>
```

## Fixtures

- `tests/data/` — the framework's primary fixture root.
- `crates/editor/test_fixtures/` — rendering/image fixtures for editor reference tests.
- App-side reference fixtures: `app/src/terminal/ref_tests/`.
- SQLite round-trip: `app/src/persistence/sqlite_tests.rs`.
