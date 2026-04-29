# crates/warp_terminal — Terminal emulation

Backend terminal emulation. Separate from `app/src/terminal/`, which is the **UI/view layer** wrapping this crate's `TerminalModel`.

Layout:

- `src/lib.rs` — small re-export
- `src/model/` — terminal model (grid, cursor, parsing state)
- `src/shell/` — shell adapters (per-shell quirks, prompt parsing)
- `src/shared_session.rs`

## TerminalModel locking — READ THIS

This is the single most common source of macOS-beach-ball bugs in the app.

- The terminal model is wrapped in a lock. **Never call `model.lock()` if any caller already holds the lock**, directly or via a parent on the call stack.
- Symptom of getting it wrong: deadlock and a UI freeze.
- Fix pattern: **pass the locked reference down** instead of re-locking. Keep lock scope as short as possible — drop it before doing UI work.
- See `WARP.md → Terminal Model Locking` and `app/src/terminal/AGENTS.md` for call-site guidance.

If you change ownership semantics (who locks, how long the guard lives), you almost certainly need a code review — these bugs do not surface in unit tests.

## Coding style

- `ctx` last and named `ctx`.
- Imports at the file top, not path-qualified at call sites.
- Inline format args.
- Exhaustive matches: do not add `_ => …` arms when the variant set is meant to be enumerated; new control sequences should be a build error you handle.

## Tests

```bash
cargo nextest run -p warp_terminal
```

End-to-end terminal behaviour is exercised by the `crates/integration/` Builder tests and by reference tests in `app/src/terminal/ref_tests/`.
