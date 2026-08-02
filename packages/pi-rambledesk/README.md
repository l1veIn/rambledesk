# @rambledesk/pi

Pi-native RambleDesk package. It registers:

- `request_ramble_feedback`: creates a RambleDesk feedback request through the
  local JSON API and waits inside the same Pi tool call until the human submits
  or cancels.
- `resume_ramble_feedback`: reconnects to an interrupted request using the
  persisted request id or the current Pi session identity.
- `get_ramble_feedback`: reads a request by `request_id` for recovery or
  diagnostics.

When the agent has prepared its exact final summary it can send
`allow_finish: true` with `final_summary`. RambleDesk then offers “Approve and
finish”; approval terminates the tool batch without another model turn.
Request ids are persisted in Pi session entries before the network request so
a restarted Pi session can reconnect instead of creating a duplicate.

In interactive TUI/RPC sessions, the package health-checks RambleDesk at task
start and enables a strict feedback cycle when it is available:

1. pre-open one ordinary collaboration request while Pi works;
2. wait for that request when the agent settles;
3. inject submitted feedback and continue;
4. require a finish-enabled request containing the exact final summary;
5. end only after approval or cancellation.

The settled watchdog is bounded to avoid an infinite model-reminder loop if a
model repeatedly refuses to call the final approval tool. Print and JSON modes
remain fail-open for scripts and CI.

Install from a source checkout:

```sh
pi install ./packages/pi-rambledesk
```

After package publication, the intended install form is:

```sh
pi install npm:@rambledesk/pi
```

The package does not use MCP. It talks to RambleDesk's authenticated loopback
JSON API:

- default API URL: `http://127.0.0.1:37642/api`
- override API URL: `RAMBLEDESK_LOCAL_API_URL`
- override port: `RAMBLEDESK_LOCAL_SERVER_PORT`
- token: `RAMBLEDESK_LOCAL_SERVER_TOKEN`, or the RambleDesk local-server token
  file (`RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE` can override the path)

The tool waits by holding the Pi tool execution, not by asking RambleDesk to
continue Pi later. Once it returns `completed`, the feedback markdown and
attachment paths are included directly in the model-visible tool `content`.
The full structured API response is also retained in `details` for logs and UI
rendering, but Pi does not add `details` to the model context.
