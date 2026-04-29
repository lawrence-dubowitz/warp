# crates/ai — Shared AI types & indexing

Shared AI primitives used by the app surface in `app/src/ai/`. ~69 Rust files.

Subdirs:

- `agent/` — agent action / action-result types (`action`, `action_result`)
- `diff_validation/` — validation of agent-produced diffs
- `index/` — codebase indexing
  - `file_outline/` — outline-level summaries
  - `full_source_code_embedding/` — embedding pipeline
- `project_context/` — project-context model
- `skills/` — skill runtime types

## Where to put what

- **Reusable types / models** (anything `app/`-side AI code imports) → here.
- **Agent runtime, conversation state, MCP servers, ambient agents, view code** → `app/src/ai/` (see its AGENTS.md). That tree is much larger and tightly coupled to the app's persistence + UI.

If you find yourself adding view code or panel state to this crate, you're in the wrong place.

## MCP / tool surfaces

The grouped-server pattern is the long-term shape for MCP resources/tools — see `app/src/ai/agent/mod.rs:1945-1951`. Avoid adding new flat surfaces.

## Tests

```bash
cargo nextest run -p ai
```

Integration coverage is in `crates/integration/`. The agent SDK / conversations / MCP wiring lives in `app/` and is tested there.
