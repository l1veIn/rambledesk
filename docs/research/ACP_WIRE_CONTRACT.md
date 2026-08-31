# ACP wire contract used by `rambledesk-acp-client`

> Research snapshot: 2026-08-30. This note records primary-source evidence for the implementation; product terms remain defined only by [TERMINOLOGY.md](../TERMINOLOGY.md).

## Version choice

RambleDesk currently speaks stable ACP v1 to Codex. The published `@agentclientprotocol/codex-acp` source is version 1.7.0 and depends on `@agentclientprotocol/sdk` 1.4.x; its server registers `session/new`, `session/load`, `session/resume`, `session/cancel`, `session/set_config_option`, `session/request_permission` and `elicitation/create` on the v1 connection. Sources: [codex-acp package](https://github.com/agentclientprotocol/codex-acp/blob/69ca755d9878238aecf0737c0e4568b3bab37be2/package.json), [codex-acp method registration](https://github.com/agentclientprotocol/codex-acp/blob/69ca755d9878238aecf0737c0e4568b3bab37be2/src/index.ts).

ACP v2 now has a different lifecycle: `session/resume` can request replay and `session/load` is removed. It is not used until Codex exposes the v2 contract and RambleDesk can switch as one negotiated implementation, rather than mixing v1 and v2 methods. Source: [official v2 migration](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/docs/protocol/v2/migration.mdx).

## Session lifecycle

- `initialize` is the first request. Its response selects a protocol version and returns `agentCapabilities`.
- v1 `session/new` requires an absolute `cwd` and `mcpServers`; it returns an Agent-owned `sessionId` and may return `configOptions`.
- `session/load` requires `loadSession: true` and replays history as `session/update` notifications before its response.
- `session/resume` requires `sessionCapabilities.resume` and reconnects without history replay.
- Every new/load/resume request carries the complete MCP server list again. RambleDesk therefore creates a new authenticated Session Toolset endpoint for each Agent Run and reinjects it on every lifecycle path.
- Codex ACP must launch with `DISABLE_MCP_CONFIG_FILTERING=true`; otherwise a same-named MCP entry in user/project configuration can silently filter the per-Run RambleDesk toolset.
- `session/cancel` is a notification. Pending Permission Requests must be answered with the `cancelled` outcome when the turn is cancelled.
- `session/close` unloads an active Session and cancels its turn; it does not delete the Agent-owned history. Codex 1.7 implements it as thread unsubscribe, so a later process can still `session/resume` the same id. RambleDesk calls close during clean shutdown, then tears down the process tree.

Sources: [official v1 session setup](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/docs/protocol/v1/session-setup.mdx), [official v1 schema](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/schema/v1/schema.json), [official Rust schema types](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/agent-client-protocol-schema/src/v1/agent.rs).

`configOptions` are Agent-returned live configuration. A client changes one with `session/set_config_option`; it must not invent model, reasoning or access choices that the Agent did not return. Source: [official session configuration](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/docs/protocol/v1/session-config-options.mdx).

Codex ACP 1.7 currently exposes three mode values whose wire names are not product semantics: `read-only` is actually a workspace-write sandbox with user-reviewed escalation, `agent` uses auto-review, and `agent-full-access` is danger-full-access without approvals. Therefore the pinned Launch Profile maps RambleDesk Workspace Write to wire `read-only`, YOLO to `agent-full-access`, and rejects product Read Only as unsupported. Preflight exposes only Workspace Write and YOLO. Launch applies model, reasoning effort and Access Mode only after `session/new`; resume/load keep the Agent-returned live configuration. Grouped select options are flattened for the UI projection while their groups are preserved. Sources: [Codex AgentMode](https://github.com/agentclientprotocol/codex-acp/blob/69ca755d9878238aecf0737c0e4568b3bab37be2/src/AgentMode.ts), [Codex config option handler](https://github.com/agentclientprotocol/codex-acp/blob/69ca755d9878238aecf0737c0e4568b3bab37be2/src/CodexAcpServer.ts).

## Permission Request

The Agent calls `session/request_permission` with `sessionId`, a `toolCall`, and the complete list of options. The response is either `selected { optionId }` or `cancelled`; the client must return one of the option ids supplied by that request. Each JSON-RPC id is an independent responder, so concurrent requests cannot share a single replaceable slot.

Codex 1.7 puts its user-facing permission reason in request-level `_meta.permission`, not necessarily in `toolCall.title`. The live projection therefore preserves both `toolCall` and the complete request `_meta`.

RambleDesk keeps responder and FIFO display state under one lock, rejects answers to a non-front request, and drains all responders on turn cancellation or disconnect. This follows both the official wire contract and the failure already fixed in codeg, where a single visible slot could orphan earlier concurrent responders. Sources: [official tool-call permission contract](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/docs/protocol/v1/tool-calls.mdx), [codeg queue implementation](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs).

## Ask Question and elicitation

ACP `elicitation/create` is a direct Agent-to-Client JSON-RPC request. Form mode carries `sessionId`, optional `toolCallId`, `message`, and a restricted flat object `requestedSchema`. The response action is `accept`, `decline`, or `cancel`; accepted form content should conform to the requested schema. The live JSON-RPC request may remain pending for human input, but it is tied to the connection and turn rather than persisted as RambleDesk history.

RambleDesk advertises form mode but its first UI slice queues Ask Question only for exactly one single-select or multi-select field. Other form shapes receive an immediate `decline`, so no JSON-RPC responder is orphaned. It maps Codex MCP approval forms (`_meta.codex_approval_kind == "mcp_tool_call"`) and message-only confirms to the Permission queue. codeg uses the same approval/question split so advertising form elicitation does not accidentally turn tool consent into a generic question. Sources: [official ACP elicitation contract](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/docs/protocol/v1/elicitation.mdx), [codeg elicitation classifier](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/question.rs), [codeg responder routing](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs).

## Session Toolset and Feedback

ACP v1 session setup accepts HTTP MCP server declarations only when the Agent advertises `mcpCapabilities.http`; stdio remains the universal baseline. Codex ACP 1.7 translates both stdio and HTTP declarations into Codex MCP configuration. Sources: [official MCP server declarations](https://github.com/agentclientprotocol/agent-client-protocol/blob/cc6855fd71086145f0d37af82d43c39da55f9398/docs/protocol/v1/session-setup.mdx), [Codex ACP MCP mapping](https://github.com/agentclientprotocol/codex-acp/blob/69ca755d9878238aecf0737c0e4568b3bab37be2/src/CodexAcpClient.ts).

The v3 Adapter is a per-Agent-Run authenticated loopback HTTP MCP server that calls only `rambledesk-core::kernel`:

- `request_feedback` is a short durable write. It injects the trusted RambleDesk `session_id` and current ACP Session Link, returns `request_id`, and instructs the Agent to end the turn.
- Waiting for a human occurs in the durable Feedback Request, never in the MCP request.
- Later `reconcile` sends a Feedback Resume Prompt. The Agent calls the short `get_feedback(request_id)` operation and receives a location-independent envelope with a stable `delivery_id`.
- Agent work is marked delivered only after a completed `get_feedback` tool update and a normal Prompt Turn completion are both observed.

This preserves unlimited human waiting without depending on any Agent's MCP timeout.

## Live verification

The non-default smoke tests use the pinned `npx -y @agentclientprotocol/codex-acp@1.7.0` command and the user's normal Codex authentication. On 2026-08-30 they verified the real mode/model/reasoning option shapes, a real network Permission Request including `_meta.permission`, a rejected Agent-provided option, clean `session/close`, and subsequent `session/resume`. The broken global `/opt/homebrew/bin/codex` wrapper was neither used nor modified.

## SDK check

The official Rust SDK 2.0 provides typed v1 schema handling, JSON-RPC correlation, subprocess launching and process-group teardown. RambleDesk keeps its own internal actor because its external Interface must own multiple durable Sessions, Core work claims, live FIFO requests and per-Run toolset lifetimes as one Module; no SDK type crosses that Interface. The implementation follows the same bounded-frame, correlation and process-group invariants and is tested with an in-memory programmable ACP Agent. Sources: [official Rust SDK](https://github.com/agentclientprotocol/rust-sdk/tree/754d5aa1ce2cfa54ba2c2a6d3edc7e7b6bce28eb), [SDK one-shot client](https://github.com/agentclientprotocol/rust-sdk/blob/754d5aa1ce2cfa54ba2c2a6d3edc7e7b6bce28eb/src/agent-client-protocol/examples/yolo_one_shot_client.rs), [SDK process owner](https://github.com/agentclientprotocol/rust-sdk/blob/754d5aa1ce2cfa54ba2c2a6d3edc7e7b6bce28eb/src/agent-client-protocol/src/acp_agent.rs).
