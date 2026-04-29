# app/src/terminal — Terminal UI

The view layer for the terminal. Wraps `crates/warp_terminal`'s `TerminalModel`. This directory is huge and full of UI state machines; some files are >100K.

## TerminalModel locking — the #1 cause of UI deadlocks

The terminal model is locked behind a mutex (`model.lock()`). **Do not call `lock()` if any caller higher in the stack already holds it.**

Symptoms: macOS beach ball / unresponsive UI when scrolling, resizing, or executing commands.

How to avoid:

1. Before adding a `model.lock()` call, walk the call chain. If a parent already locked, accept a `&MutexGuard<…>` (or the inner ref) as a parameter instead.
2. Keep the lock scope **short**. Drop the guard before doing async work or rendering.
3. Prefer pure helpers operating on the inner data; lock once at the top and pass the guard down.

Reference: `WARP.md → Terminal Model Locking`.

## Subdirs

- `model/` — view-side model adapters around `TerminalModel`
- `view/` — the rendered terminal view (`view.rs`, `TerminalView::render`)
- `grid_renderer/` — grid drawing
- `block_filter.rs`, `blockgrid_*.rs`, `block_list_*.rs` — block list rendering (one of the largest files in the codebase: `block_list_element.rs` ~204K, `block_list_viewport.rs` ~89K)
- `local_shell/`, `local_tty/`, `remote_tty/`, `writeable_pty/`, `ssh/`, `wsl/` — shell/PTY transports
- `shared_session/` — shared / collaborative sessions
- `cli_agent_sessions/`
- `find/` — in-terminal find
- `history/`
- `input/`
- `prompt/`
- `event_listener/`
- `alt_screen/`, `alt_screen_reporting.rs`
- `audible_bell/`
- `session_settings/`
- `warpify/` — block extraction / shell integration
- `ref_tests/` — reference tests (golden output)

## Editing `block_list_element.rs` and friends

Files this large drift if you don't constrain yourself:

- Make minimal, surgical edits.
- Don't reorder unrelated chunks.
- Don't strip comments — they're often the only documentation for old quirks.
- If you really need a refactor, propose it explicitly first.

## Tests

- Unit tests are file-adjacent: `*_tests.rs`, `mod_test.rs`.
- **Reference tests** for terminal rendering live in `ref_tests/`. Update goldens deliberately, not as a side effect — review the diff before committing.
- `bootstrap_test.rs`, `available_shells_test.rs`, `cli_agent_tests.rs` are good entry points if you're new to this directory.

```bash
cargo nextest run -p warp terminal::
```

End-to-end terminal scenarios are exercised by `crates/integration/tests/integration/shell_integration_tests/*` (a separate CI slice).

## Other landmines in this tree

- `available_shells.rs` is ~38K and lists every shell Warp knows how to bootstrap. Be careful adding entries; each one tends to come with shell-specific quirks.
- `command_corrections_denylist.rs` is intentionally small — entries here suppress corrections globally.
- `buy_credits_banner.rs` (~35K) is a banner; it's not part of terminal emulation despite being in this folder.

## Style reminders

- `ctx` is last and named `ctx`.
- No path qualifiers; imports at the top.
- Inline format args.
- Exhaustive matches.
