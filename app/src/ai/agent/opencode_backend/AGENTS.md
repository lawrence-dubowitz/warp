# Agent Instructions — Opencode Backend

This module implements the backend glue for the OpenCode agent integration. It manages the lifecycle of the `opencode serve` subprocess and routes communication between Warp and the OpenCode server.

## Architecture Overview

The backend follows a three-stream implementation strategy defined in `specs/opencode-as-primary-agent/TECH.md`.

### Implementation Streams
1. **Infrastructure (Complete)**: Transport crate (`opencode_transport`), skeleton modules, and the selector branch in `api/impl.rs`.
2. **Execution (In Progress)**: Bun plugin, JSON-RPC server framing, and the `ParkedToolCall` state machine for asynchronous tool execution.
3. **Persistence (Pending)**: Storage of session IDs and backend kinds in `AgentConversationData` and `bd prime` memory injection.

## Module Map

- `backend.rs`: Primary handle (`OpencodeBackend`) coordinating the supervisor, demuxer, and HTTP client.
- `session_router.rs`: Maps Warp conversation IDs to OpenCode session IDs; handles lazy session creation.
- `event_to_client_action.rs`: Converts SSE events from OpenCode into Warp `ClientAction` batches.
- `parked_call.rs`: Manages the lifecycle of tool calls that are "parked" awaiting plugin execution/approval.
- `rpc_handlers.rs`: Implements the JSON-RPC handlers for the plugin-to-Warp communication channel.
- `action_adapter.rs`: Maps OpenCode tool requests into `AIAgentActionType` variants.
- `prompt_builder.rs`: Constructs the prompt sent to OpenCode (includes context and system prompts).
- `persistence_glue.rs`: Handles persistence of session mapping and backend preferences.
- `beads_glue.rs`: Injects read-only `bd prime` context into the agent's prompt.

## Key Invariants

- **Subprocess Lifecycle**: One `opencode serve` process per Warp instance, supervised with backoff and killed on shutdown.
- **Event Routing**: A single global SSE subscription is demuxed by `sessionID` into per-conversation channels.
- **Tool Execution**: Tool calls are tracked via `ParkedToolCall` to handle the async gap between the RPC request and the SSE result.
- **Feature Gating**: The entire backend is gated by `FeatureFlag::UseOpencodeAsPrimaryAgent`.
