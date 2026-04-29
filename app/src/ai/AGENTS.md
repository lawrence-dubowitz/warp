# app/src/ai — App-side AI surface

This is the largest, deepest tree in `app/`. It's where agents, conversations, ambient agents, MCP integration, skills, and the AI document UI all live. Shared types are in `crates/ai/`; this directory is the **app-bound** runtime, persistence, and view code.

## High-level subdirs

- `agent/` — agent runtime: action execution, tool dispatch, schema
- `agent_events/` — event types between agent and UI
- `agent_management/` — agent lifecycle, scheduling, filters
- `agent_sdk/` — public SDK surface
- `ambient_agents/` — long-running, background-running agents
- `artifacts/` — agent-produced files, downloads
- `blocklist/`
- `cloud_agent_config/` — config for cloud-hosted agents
- `cloud_environments/`
- `conversation_navigation/` — UI for moving between turns/messages
- `document/` — AI document panes
- `execution_profiles/` — runtime profiles applied to agent execution
- `facts/` — long-lived agent memory primitives
- `generate_block_title/`
- `generate_code_review_content/`
- `get_relevant_files/`
- `loading/` — progressive loading UI for streamed agent output
- `mcp/` — MCP (Model Context Protocol) servers, tools, resources
- `outline/`
- `predict/` — predictive completions
- `skills/` — skill registry / runtime (paired with `.agents/skills/` on disk)
- `voice/` — voice agent

Top-level files include `agent_conversations_model.rs` (~76K), `conversation_details_panel.rs` (~72K), `ai_document_view.rs` (~49K), and the corresponding `*_tests.rs` siblings.

## Hard rule: MCP servers are grouped, not flat

`app/src/ai/agent/mod.rs:1945-1951` is explicit: **do not** add new MCP resources/tools as flat top-level surfaces. They belong on grouped servers. Search for that comment if you're tempted.

## Conversation models are big — change them carefully

`agent_conversations_model.rs` and `conversation_details_panel.rs` each carry years of accumulated state machinery. When extending them:

- Land changes behind a `FeatureFlag` first (`crates/warp_features/`). Flip the flag in `DOGFOOD_FLAGS` once you're confident.
- Keep tests next to the file (`*_tests.rs`) and use `App::test` so you exercise the real entity/view machinery, not a mock.
- Treat schema changes as persistence migrations — see `app/src/persistence/AGENTS.md`.

## Ambient agents

`ambient_agents/` and the recent `ambient_agent_panes` migration (`crates/persistence/migrations/.../add_ambient_agent_panes`) imply this is an active area. Coordinate via beads (`bd ready`) before stacking concurrent changes.

## Telemetry

Use `register_telemetry_event!`. App-event conversion / redaction goes through `app/src/server/telemetry_ext.rs`. The events match in `app/src/server/telemetry/events.rs` is **exhaustive** — no `_` arm allowed; every variant must be handled and feature-flagged explicitly (see line 5032).

## Tests

- Unit tests in `*_tests.rs` files at this level — many of them are huge by intent.
- Reference tests for terminal output go through `app/src/terminal/ref_tests/`.
- Integration coverage: `crates/integration/`.

```bash
cargo nextest run -p warp ai::                # rough filter; many tests live in this tree
```
