# RambleDesk ACP client

The ACP implementation depends on `rambledesk-core`, not the database, desktop or
feedback server. It uses the official Rust SDK 2.0.0 with stable ACP protocol v1.
No experimental protocol-v2 features are enabled.

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
