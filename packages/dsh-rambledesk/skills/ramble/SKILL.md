---
name: ramble
description: Run a human-feedback loop through RambleDesk. Hard trigger: when the user invokes /ramble, $ramble, [$ramble], or says to use RambleDesk/ramble feedback, create a RambleDesk request before answering the substance unless they explicitly say not to send one. Also use when real human judgment, review, approval, or hands-on checking is needed.
---

# ramble — run a human-feedback loop through RambleDesk

Route anything that needs a real human's judgment, review, approval, or hands-on
experience through the RambleDesk feedback loop instead of asking in chat.

## Invocation contract

- Explicit invocation is a command, not a hint. If the user invokes `/ramble`,
  `$ramble`, `[$ramble]`, names this skill as the way to handle the task, or
  asks for RambleDesk feedback, create the RambleDesk request first.
- Do not answer with advice, a review, a design proposal, or implementation
  before creating the request just because you can infer a reasonable answer
  from screenshots, files, or local context.
- For implicit use, invoke this skill when human testing, visual inspection,
  screenshots, clarification, or a decision would materially improve the
  result.
- A meta request to inspect, edit, or debug this skill is not itself a request
  to create a RambleDesk feedback item unless the user also explicitly asks to
  run the feedback loop for that meta task.
- If the expected RambleDesk tool name is unavailable, discover the available
  RambleDesk equivalent and follow its schema. Do not silently degrade an
  explicit invocation into ordinary chat.
- Do not hard-code host confirmation tool names from examples. If a RambleDesk
  request tool returns before the human finishes, search the available tools for
  terms such as `ask`, `question`, `input`, `confirm`, and `confirmation`, then
  use any discovered host tool whose schema can block until the human completes
  or cancels the request.

## Workflow

1. **Request** — call `request_ramble_feedback`:
   - `title`: a short title the human can read at a glance in their Inbox.
   - `what_happened`: background — what you are doing, why you need this
     feedback, and what lens the human should bring.
   - `actions`: an explicit, executable checklist — one action per item.
   - `attachments` / `context_refs`: only when there is material for the human
     to review (a long document or a generated image). By default pass no
     attachments. When attaching a local file, pass `attachments[].path` as an
     absolute filesystem path and `file_name` — do not Read the file into
     `contents_base64`. Use `markdown` only for short inline Markdown. Use
     `contents_base64` only for a small image that is not on disk.
   - `allow_finish`: omit by default. Set `true` only when the request is a
     simple approve/reject decision that needs no feedback body, and then also
     provide `final_summary`.
   - `request_id`: omit — the server generates it.
   - `host_session_id`: omit — the plugin derives it from the current dsh
     session, so concurrent sessions stay separate.
   - `wait`: keep `true` (the default). The tool call blocks until the human
     submits or cancels in RambleDesk, then returns the feedback package
     directly. Do not call a separate host confirmation tool while this native
     wait is active.
2. **Read** — when the tool returns `completed`, read the feedback markdown
   from the tool content plus any attachment paths.
3. **Implement** — apply the feedback item by item; when you need another
   confirmation or review, go back to step 1.
4. **Recover** — if a wait was interrupted (cancelled turn or restart), call
   `resume_ramble_feedback` to reconnect to the durable request instead of
   creating a new one. `get_ramble_feedback(request_id)` reads state without
   waiting.
5. **Cancel** — if the human explicitly gives up, call `cancel_ramble_feedback`.

## Principles

- Persist first: a created request is durable and survives disconnect/restart.
- An explicit invocation is not complete until one of these is true: the
  request was created and the feedback package was read, the host requires a
  manual continuation after request creation, the user cancelled, or no
  RambleDesk request tool is available after discovery.
- Never set `allow_finish` for requests that need detailed feedback (experience,
  checks, opinions); the human should submit a feedback body, not a shortcut.
- One topic per request; split multiple topics into multiple requests.
- Restate your understanding before acting on feedback; if it is ambiguous,
  clarify with another ramble instead of guessing.
