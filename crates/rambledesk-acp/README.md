# RambleDesk ACP client

The ACP implementation depends on `rambledesk-core`, not the database, desktop or
feedback server. It uses the official Rust SDK 2.0.0 with stable ACP protocol v1.
No experimental protocol-v2 features are enabled.

## Application boundary

Production callers construct `AcpSessionDriver` and use the application-owned
`AgentSessionDriver` / `AgentSessionConnection` interfaces from core. SDK request
types and notifications are not part of that interface. Low-level SDK access is
explicitly under `rambledesk_acp::probe`, for diagnostic examples and protocol
tests; application integration should not use it.

Session configuration exposes only an `options` collection and a single
`{config_id, value}` change. IDs are opaque session-local handles. Standard
config options, legacy mode/model catalogs and private metadata are normalized
inside ACP, which retains the original setter route. Neither the UI nor core
chooses a protocol method. Agent-confirmed values and runtime updates remain
authoritative; category names do not imply permission or YOLO semantics.

Pending user work uses `SessionInteraction`: `permission`, `question` or `plan`.
Snapshots expose `interactions`, runtime state is `waiting_input`, and callers
submit a matching tagged `SessionInteractionResponse` through
`respondManagedInteraction`. A mismatched response does not consume the request.
These are live application contracts; stored session IDs, history and database
formats are unchanged. A frontend and backend from this development branch must
be upgraded together.

Private interaction modules own their RPC methods, parsing, response envelopes
and cancellation semantics. Shared code handles bounded form schemas, attribution,
pending requests and lifecycle cleanup. Adding a protocol must not introduce
provider checks into the core session state machine or UI components.

This crate remains RambleDesk's ACP integration: installation discovery and
managed-feedback injection are separate internal responsibilities. It does not
attempt to replace the official SDK with a general-purpose plugin framework.

## Probe and verification

Run the probe against an installed agent:

```powershell
cargo run -p rambledesk-acp --example smoke -- C:/temp/launch.json "Reply with OK"
```

The JSON file contains separate command/arguments, an absolute working directory,
optional environment overrides, and optional ACP `mcp_servers` declarations:

```json
{"command":"deepseek-acp","args":[],"cwd":"C:/projects/example"}
```

A third argument loads/resumes an existing remote session ID. The probe prints
negotiated identity/capabilities, assistant text and the stop reason; it does not print raw protocol messages,
credentials or stderr. Permissions are declined by the probe. No client filesystem
or terminal capability is advertised. Supported `session/close` is awaited before
EOF so the agent can flush its conversation. A bounded shutdown wait is followed
by child termination/reaping if needed.

`cargo test -p rambledesk-acp` uses a local Node fixture (no network or API key) to
verify stdio initialization, message updates, explicit permission cancellation,
prompt cancellation, original-ID load/resume, unsupported recovery and launch validation.
Real backend evidence is recorded in `docs/ACP_BACKEND_PROBE.md` in the repository.

Managed sessions advertise ACP form elicitation and route session-scoped
`elicitation/create`, Grok's `_x.ai/ask_user_question` / `_x.ai/exit_plan_mode`,
and Cursor's `cursor/ask_question` / `cursor/create_plan` through the same pending
request lifecycle as permissions. Answers return on the original JSON-RPC request;
they never become approval option IDs or ordinary chat messages. Cursor extensions
use the owning connection's session binding because their parameters omit a session ID.

Supported forms contain flat string, boolean, numeric, or choice-array fields.
The backend validates required fields, original choice values, types, bounds and
uniqueness before consuming a reply. Unknown constraints, including string patterns
and formats, keep a visible card with decline/cancel available; acceptance stays
disabled. URL and pre-session authentication elicitation are outside this managed
session surface. Cancellation, disconnect and late requests release parked replies.

Grok's keep-planning reply ignores feedback. The plan card explains that revision
notes belong in the next message; nonempty feedback on a non-approval answer is
rejected rather than silently lost. Plan approvals require an explicit decision.
`tests/user_input.rs` verifies the actual provider response envelopes, invalid and
duplicate answers, session isolation, unsupported forms and late cancellation.
