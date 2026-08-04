# @rambledesk/pi

Pi-native RambleDesk package. It registers:

- `request_ramble_feedback`: creates a RambleDesk feedback request through the
  local JSON API and waits inside the same Pi tool call until the human submits
  or cancels.
- `resume_ramble_feedback`: reconnects to an interrupted request using the
  persisted request id or the current Pi session identity.
- `get_ramble_feedback`: reads a request by `request_id` for recovery or
  diagnostics.

`request_ramble_feedback` also accepts an optional `attachments` array so the
agent can hand the human review artifacts that render in the RambleDesk
workspace. Each attachment provides `file_name` plus exactly one content field:
`markdown` (a Markdown document, requires a `.md`/`.markdown` file name) or
`contents_base64` (a PNG/JPEG/GIF/WebP image).

When the agent has prepared its exact final summary it can send
`allow_finish: true` with `final_summary`. RambleDesk then offers “Approve and
finish”; approval terminates the tool batch without another model turn.
Request ids are persisted in Pi session entries before the network request so
a restarted Pi session can reconnect instead of creating a duplicate.

In interactive TUI/RPC sessions, startup performs one bounded local health
check and enables RambleDesk guidance when the app is available—the same effect
as running `/ramble`. A transient notification explains that `/ramble_off`
disables the guidance for the current session. `/ramble` can enable it again.
The enabled mode adds a short system-prompt reminder to request human input only
when testing, inspection, a screenshot, clarification, or a decision would be
materially useful.

The package never creates a request merely because a session or task started,
does not add a persistent status line, and does not gate agent settlement. Every
request still comes from an explicit tool call with concrete context and human
actions. Recovery, idempotent retries, and optional final-summary approval
remain available inside that tool flow. A request transport failure keeps and
reports the generated `request_id`; the next retry without an explicit ID reuses
that pending ID instead of creating a duplicate request. Print and JSON modes do not run the
automatic health check or inject guidance.

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
