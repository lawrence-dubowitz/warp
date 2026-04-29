# Agent Instructions — Warp

Warp is a Rust-based terminal with an in-house GPU UI framework, deep AI integration, and a custom Cargo workspace covering 60+ crates plus the main `app/` binary. This file is the canonical entry point for AI agents working in this repo.

For deeper engineering context, also read [`WARP.md`](./WARP.md) (architecture, build/test/lint commands, coding style, terminal-model-locking, feature-flag rollout, exhaustive-match rule). Do not duplicate WARP.md here — link to it.

## rtk policy

Use `rtk` for all high-volume shell commands. `rtk` is a CLI proxy that filters and compresses noisy output before it reaches the agent context window.

- Prefix with `rtk` for: `git status`, `git diff`, `git log`, `gh`, `find`, `grep`, `tree`, `ls`, `docker`, `kubectl`, `cargo test`, `cargo nextest`, log reads on large files.
- Do **not** use `rtk` when exact unfiltered output is required (e.g. parsing a config, debugging a tool that depends on byte-exact output).
- Do **not** stack `rtk` with another output reducer.

See `/home/lawrence/AGENTS.md` for the cross-project policy.

## Map of AGENTS.md files

Read the most-specific AGENTS.md for the directory you are touching. Each scoped file documents only what is unique to that subtree.

- `/AGENTS.md` (this file) — repo-wide rules, commands, conventions
- `/app/AGENTS.md` — main binary, module layout, lib.rs guardrails
- `/app/src/ai/AGENTS.md` — agent runtime, MCP, conversations, ambient agents
- `/app/src/settings/AGENTS.md` — settings macros, NEVER add to `Settings`
- `/app/src/terminal/AGENTS.md` — TerminalModel locking, blocks, ref tests
- `/app/src/persistence/AGENTS.md` — SQLite, Diesel, schema.rs + schema.patch
- `/crates/AGENTS.md` — workspace map and per-crate ownership index
- `/crates/warp_core/AGENTS.md` — telemetry/error registration macros, channels
- `/crates/warp_features/AGENTS.md` — `FeatureFlag` enum, rollout sets
- `/crates/warpui_core/AGENTS.md` — Entity-Component-Handle, Elements, scene
- `/crates/warpui/AGENTS.md` — wgpu rendering, winit, platform impls
- `/crates/warp_terminal/AGENTS.md` — terminal emulation crate
- `/crates/ai/AGENTS.md` — shared AI types, indexing, skills
- `/crates/integration/AGENTS.md` — Builder/TestStep framework, registration

## Stack at a glance

- **Language**: Rust pinned to `1.92.0` (see `rust-toolchain.toml`). 2018 edition (`.rustfmt.toml`). Resolver 2.
- **Workspace**: `app` + `crates/*` (~63 crates). Default-members narrowed (excludes `serve-wasm`, `integration`).
- **UI framework**: `warpui_core` + `warpui` (custom; Entity-Component-Handle; wgpu via metal/vulkan/dx12/gles; winit).
- **Async**: Tokio everywhere; async-broadcast; never use `std::time::Instant`, use `instant::Instant` for WASM compatibility.
- **Process spawning**: Forbidden — `std::process::Command`, `async_process::Command`. Use `command::blocking::Command` or `command::r#async::Command` (flashes a terminal on Windows otherwise). See `.clippy.toml`.
- **Logging**: `tracing` (NOT `dbg!()`; banned in `.clippy.toml`).
- **HTTP**: `reqwest` + `rustls`. **DB**: Diesel + SQLite. **GraphQL client**: `cynic`.
- **Targets**: macOS, Linux, Windows, WASM (`wasm32-unknown-unknown`). The WASM build is a real target — gate filesystem and other native APIs.

## Local Environment Issues

- **`command-signatures-v2` build failure**: The build script for this crate requires Node 18.14.1 and yarn/corepack. On some Linux dev boxes, this causes `cargo clippy --workspace` to panic.
  - **Workaround**: Use scoped clippy: `cargo clippy -p warp -p <other_crates> ...` or exclude the crate explicitly.

## Canonical commands

There is no top-level `Makefile`/`Justfile`. Canonical workflow is `./script/*` plus `cargo nextest`.

### Setup
```bash
./script/bootstrap                 # First-time setup
./script/install_cargo_build_deps  # Build deps
./script/install_cargo_test_deps   # Test deps
```

### Run
```bash
./script/run                                              # Cross-platform launcher (handles macOS bundling/codesign)
cargo run --features with_local_server                    # Run against local server
SERVER_ROOT_URL=http://localhost:8082 \
  WS_SERVER_URL=ws://localhost:8082/graphql/v2 \
  cargo run --features with_local_server                  # Local server explicitly
./script/wasm/run                                         # WASM dev run
```

### Test
```bash
./script/presubmit                                                            # Full local pre-PR check (must pass before opening/updating a PR)
cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2  # Default test runner
cargo nextest run -p warp_completer --features v2                             # Completer v2 path (excluded from main clippy/test)
cargo test --doc                                                              # Doctests
```

CI runs the integration package in two slices (`shell_integration_tests` vs the rest) — see `.github/workflows/ci.yml`.

### Lint / format (matches `script/presubmit`)
```bash
cargo fmt -- --check
cargo clippy --workspace --exclude warp_completer --all-targets --tests -- -D warnings
cargo clippy -p warp_completer --all-targets --tests -- -D warnings
./script/run-clang-format.py -r --extensions 'c,h,cpp,m' ./crates/warpui/src/ ./app/src/
find . -name "*.wgsl" -exec wgslfmt --check {} +
./script/lint_powershell -ci         # required when pwsh is on PATH
cargo clippy --locked --target wasm32-unknown-unknown --profile release-wasm-debug_assertions -- -D warnings
```

`warp_completer` is excluded from the workspace clippy because its `v2` feature gates work-in-progress code; it is linted/tested separately.

### Bundle / package
```bash
cargo bundle --bin warp                                          # macOS .app
./script/bundle --channel oss --nouniversal --check-only         # Native bundle dry-run
./script/wasm/bundle --channel oss --nouniversal --check-only    # WASM bundle dry-run
```

## Coding style (project-specific)

See `WARP.md` for the full list. Highlights agents repeatedly trip on:

- **`ctx` parameter goes last** and is named `ctx` (exception: closure-taking functions put the closure last).
- **Remove unused parameters** — never prefix with `_`.
- **Inline format args**: `format!("{message}")`, not `format!("{}", message)`. Clippy enforces `uninlined_format_args`.
- **Imports at file top**, no path qualifiers, except in `#[cfg(...)]`-guarded blocks where absolute or scope-local imports may be needed.
- **Exhaustive matches**: avoid `_ => …`. Enumerate every variant so adding one is a build error you must address.
- **Don't strip existing comments** when the logic is unchanged.

## Hard "do not" list (project-wide)

Agents lose hours to these. Do not violate them.

- **Do not implement `RegisteredTelemetryEvent` directly** — use the `register_telemetry_event!` macro (`crates/warp_core/src/telemetry.rs`).
- **Do not implement `RegisteredError` directly** — use `register_error!` (`crates/warp_core/src/errors/registration.rs`).
- **Do not add public modules in `app/src/lib.rs`** — see the comment near line 101.
- **Do not add fields to `Settings` directly** — go through the macros in `app/src/settings/macros.rs`.
- **Do not add new global actions** in `app/src/workspace/global_actions.rs` — use typed actions.
- **Do not use `std::time::Instant`, `std::process::Command`, `async_process::Command`, `std::dbg!`, `async_channel::Sender::send_blocking`, or `LineEnding::from_current_platform`** — see `.clippy.toml` for replacements.
- **Do not call `model.lock()` on `TerminalModel` if a parent in the call chain already holds the lock** — duplicate locks cause UI deadlock (macOS beach ball). Pass locked refs down; keep lock scope short. See `WARP.md → Terminal Model Locking`.
- **Do not use real shell dotfiles in integration tests** — the framework provides hermetic temp HOME/rc files. See `crates/integration/AGENTS.md`.
- **Do not retry to mask deterministic failures** — fix the failure.
- **Do not assert before bootstrap** in integration tests — call `wait_until_bootstrapped_single_pane_for_tab(0)` first.
- **Do not introduce filesystem APIs in WASM code** without `local_fs` (or equivalent) gating.
- **Do not use `unthemed_window_border()` for themed views** — see `app/src/root_view.rs:134`.
- **Do not change/remove eval users** in `app/src/server/server_api.rs:93-100`.
- **Do not use `MenuItem::submenu`** — currently disabled (`app/src/menu.rs:1270`).
- **Do not invent triage labels** — only use those declared in `.github/issue-triage/config.json`.

## Where things live

- **Binaries**: `app/src/bin/{local,oss,dev,preview,stable}.rs`. Channel selection picks `warp` (Local) vs `warp-oss` based on `warp-channel-config` on `PATH`.
- **App startup**: `app::run()` in `app/src/lib.rs`. Top-level views in `app/src/root_view.rs`.
- **Feature flags**: `crates/warp_features/src/lib.rs` — `FeatureFlag` enum + `DEBUG_FLAGS` / `DOGFOOD_FLAGS` / `PREVIEW_FLAGS` / `RELEASE_FLAGS` + `FeatureFlag::X.is_enabled()`. Prefer runtime checks over `#[cfg]`.
- **Telemetry**: definitions in `crates/warp_core/src/telemetry.rs`; transport in `app/src/server/telemetry/`; app-event conversion/redaction in `app/src/server/telemetry_ext.rs`.
- **Settings**: framework in `crates/settings`; app-side schema in `app/src/settings/`.
- **Terminal emulation**: `crates/warp_terminal`. Terminal UI in `app/src/terminal/`.
- **AI/agents**: shared types in `crates/ai`; full app surface in `app/src/ai/` (agents, MCP, ambient, skills, conversation).
- **Persistence**: `crates/persistence/migrations/` (Diesel; ~110 migrations from 2021 to today). Schema in `app/src/persistence/schema.rs` + `schema.patch`. **Never edit `schema.rs` manually** — see `app/src/persistence/README.md`.
- **GraphQL**: schema in `graphql/api/schema.graphql`; generated Rust in `crates/graphql`.
- **Integration tests**: `crates/integration/`. Custom Builder/TestStep framework. Tests must be registered in TWO places — see `crates/integration/AGENTS.md`.

## Skills (project-specific workflows)

Project skills live in `.agents/skills/` and override built-in defaults. Use them over generic approaches:

- `add-feature-flag` / `promote-feature` / `remove-feature-flag`
- `add-telemetry`
- `create-pr` / `review-pr` / `review-pr-local` / `diagnose-ci-failures`
- `fix-errors` (compile, clippy, fmt, WASM, presubmit)
- `implement-specs` / `spec-driven-implementation` / `write-product-spec` / `write-tech-spec`
- `resolve-merge-conflicts`
- `rust-unit-tests`
- `triage-issue-local` / `dedupe-issue-local`
- `warp-integration-test`, `warp-ui-guidelines`
- `update-skill`

## Quick Reference (legacy beads block kept verbatim below)

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd dolt push          # Push beads data to remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
Use 'bd' for task tracking
