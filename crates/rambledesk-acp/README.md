# RambleDesk ACP client

This crate depends on `rambledesk-core`, not SQLite, Tauri or the feedback server.
It uses the official Rust SDK 2.0.0 and ACP protocol v1, with the SDK's
`unstable_elicitation` feature enabled for form interactions. It is the ACP
integration, not a general-purpose agent runtime or plugin framework.

## Application boundary

Production code constructs `AcpSessionDriver` through core's
`AgentSessionDriver` / `AgentSessionConnection` contracts. SDK requests and
notifications stay inside ACP. Raw access under `rambledesk_acp::probe` is for
protocol tests and diagnostic examples, not application integration. Discovery,
installation and feedback workflow injection remain separate internal concerns.

Session configuration exposes `SessionConfiguration.options` and one
`{config_id, value}` change. IDs are opaque, connection-local handles: an option
can disappear and return on the same connection without changing its handle,
but reconnecting requires fresh negotiation. ACP normalizes standard options,
legacy model/mode catalogs and private metadata while retaining the original
setter route. Core and UI do not select wire methods or infer permission/YOLO
semantics from categories. Agent-confirmed values and runtime updates remain
authoritative; catalog labels and cached checks are not capability guarantees.

Pending input is `SessionInteraction`: `permission`, `question` or `plan`.
Snapshots expose `interactions`, runtime activity becomes `waiting_input`, and
`respondManagedInteraction` takes a matching tagged response. Wrong types or
invalid answers do not consume the pending request. Frontend and backend must
use the same application contract; session identity and history are separate
from these live interaction/configuration handles.

## Interaction support

Managed sessions advertise ACP form elicitation and support session-scoped
`elicitation/create`, Grok `_x.ai/ask_user_question` / `_x.ai/exit_plan_mode`, and
Cursor `cursor/ask_question` / `cursor/create_plan`. Replies return on the
original JSON-RPC request, never as chat text or a permission option ID. Cursor
requests use the owning connection's session binding because their parameters
omit a session ID.

Forms support flat strings, booleans, numbers and choice arrays. Required fields,
original choice values, types, bounds and uniqueness are validated before a
reply is consumed. Unsupported constraints, including patterns and formats,
keep a visible decline/cancel path but disable acceptance. URL and pre-session
authentication elicitation are outside this managed-session surface. Grok's
keep-planning reply cannot carry revision notes: nonempty feedback on a
non-approval response is rejected, and notes belong in the next message.
Plan approval always requires an explicit decision.

Private modules own method names, parsing, response envelopes and cancellation
semantics; shared code owns schemas, attribution and pending-request cleanup.
Cancellation, disconnect, turn completion and late requests release parked
replies. New provider support must not add provider branches to core or UI.
No ACP client filesystem or terminal capability is advertised; managed feedback
uses the Agent's own command execution through the private IPC workflow.

## Probes and evidence

From the repository root, `smoke` accepts a launch file, an optional prompt, and
an optional existing remote session ID after the prompt:

```sh
cargo run -p rambledesk-acp --example smoke -- /absolute/path/launch.json
cargo run -p rambledesk-acp --example smoke -- /absolute/path/launch.json "Reply with OK" "existing-remote-id"
```

```json
{"command":"deepseek-acp","args":[],"cwd":"/absolute/project/path"}
```

Use the installed bridge's actual command and ACP arguments. `cwd` is required
here, unlike `managed_loop`. Optional `env` and `mcp_servers` are low-level launch
inputs; keep credentials in existing Agent authentication or inherited
configuration, not in the JSON. Without a prompt, smoke still launches the Agent
and creates/restores a session; it does not verify model output. A prompt invokes
the model and may incur charges, so authorize the run explicitly.

The example prints negotiated session information, assistant text and stop
reason. It declines permissions and provides no approval UI. Shutdown awaits
supported `session/close`, then EOF, with bounded termination/reaping if needed.
It does not exercise the full Ramble feedback application.

`cargo test -p rambledesk-acp` uses local Node fixtures without network or API
keys. Tests cover protocol traffic, configuration handles, recovery and pending
interaction lifecycles; `tests/user_input.rs` checks provider envelopes, invalid
and duplicate replies, session isolation and unsupported forms. Fixture success
is not real backend or model certification.

Use the [managed backend probe](../../docs/ACP_BACKEND_PROBE.md) for real
feedback, two-session isolation and original-ID recovery. Current results and
missing acceptance live in the [quality checklist](../../docs/quality/README.md).
See the [managed-session guide](../../docs/ACP_MANAGED_SESSIONS.md) for user
behavior and [CODEG_PORTS.md](../../docs/CODEG_PORTS.md) for source attribution
and adopted boundaries.
