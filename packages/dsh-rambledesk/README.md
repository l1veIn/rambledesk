# @rambledesk/dsh

RambleDesk native adapter for the DeepSeek Harness (dsh). A Cordis plugin that
registers:

- `request_ramble_feedback`: creates a RambleDesk feedback request through the
  local JSON API and waits inside the same dsh tool call until the human submits
  or cancels.
- `resume_ramble_feedback`: reconnects to an interrupted request using the
  persisted request id and dsh session identity.
- `get_ramble_feedback`: reads a request by `request_id` for recovery or
  diagnostics.
- `cancel_ramble_feedback`: explicitly cancels a waiting or in-progress request.

`request_ramble_feedback` also accepts an optional `attachments` array so the
agent can hand the human review artifacts that render in the RambleDesk
workspace. Each attachment provides `file_name` plus exactly one content field:
prefer `path` (an absolute local file) for images and Markdown already on disk;
use `markdown` for a short inline Markdown document (`.md`/`.markdown` file
name); use `contents_base64` only for a small PNG/JPEG/GIF/WebP image that is
not on disk.

When the agent has prepared its exact final summary it can send
`allow_finish: true` with `final_summary`. RambleDesk then offers "Approve and
finish"; the returned tool content instructs the agent to end the flow without
another model turn.

The tool waits by holding the dsh tool execution, not by asking RambleDesk to
continue dsh later. The tool declares no `timeoutMs`, so the wait is not cut
short by the dsh tool-call timeout policy; the only interruption is the
execution signal (a cancelled turn), which aborts the wait without touching the
durable request. Once it returns `completed`, the feedback markdown and
attachment paths are included directly in the model-visible tool `content`.

Request ids are persisted in `state.json` next to the plugin before the network
request, together with the session's `host_session_id`, so a restarted dsh
session can reconnect with `resume_ramble_feedback` instead of creating a
duplicate. The `host_session_id` is derived from the calling dsh session
(per-session `dsh-<session-id>`); when no session identity is available it
falls back to a stable per-machine id (`dsh-<uuid>`). Pending request state in
`state.json` is keyed by that session id, so concurrent sessions never see or
resume each other's requests. Request transport failures keep and report the
generated `request_id`.

## RambleDesk-only mode

The plugin also implements a persistent ramble-mode switch. When on, every
session's system prompt carries the "RambleDesk-only mode" constraint: the
agent communicates with the human exclusively through RambleDesk feedback
requests (open the workbench, minimize the chat, issue instructions and review
through feedback packages) and never asks or waits in the chat.

- Enter with the `/ramble_on` slash command (it also health-checks the local
  RambleDesk server and reports availability), leave with `/ramble_off`.
- Or configure `mode: on` in the plugin config so new sessions start in ramble
  mode automatically.
- The mode is persisted in `state.json`, so it survives restarts and applies to
  new sessions; `state.json` wins over the config value (a later `/ramble_off`
  stays off).

The mode is injected through `systemPrompt.context` with a provider evaluated
per assembly, so flipping the mode changes the model's behaviour on the next
turn without a plugin reload.

The desktop installer writes the shared canonical `ramble` skill into dsh's
global skill directory. That skill owns `/ramble [task]` and starts a task-scoped
native feedback loop without enabling persistent mode. With no meaningful task
text, the skill first asks in RambleDesk for the goal, context and constraints,
desired output, and completion criteria. A task may use later serialized
requests for clarification, review, or final confirmation; unrelated future
tasks are unaffected unless `/ramble_on` is active.

The plugin has no npm dependencies. It registers plain tool definitions on
`ctx.tools` and talks to RambleDesk's authenticated loopback JSON API:

- default API URL: `http://127.0.0.1:37642/api`
- override API URL: `RAMBLEDESK_LOCAL_API_URL`
- override port: `RAMBLEDESK_LOCAL_SERVER_PORT`
- token: `RAMBLEDESK_LOCAL_SERVER_TOKEN`, or the RambleDesk local-server token
  file (`RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE` can override the path)

## Install

From a dsh profile (`~/.dsh/profiles/<profile>/`), either install the package
into the profile's dependencies, or copy this directory next to the profile and
mount it by relative path in `cordis.patch.yml`:

```yaml
- insert:
    - id: rambledesk
      name: './plugins/rambledesk/index.js'
      config:
        hostId: dsh
```

Restart dsh after changing `cordis.patch.yml` (the web profile disables HMR).

## Test

```sh
pnpm test:dsh
```
