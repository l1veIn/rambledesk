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
- Do not hard-code host confirmation tool names from examples. Before deciding
  the host cannot wait for the human, search the available tools for terms such
  as `ask`, `question`, `input`, `confirm`, and `confirmation`, then use any
  discovered host tool whose schema can block until the human completes or
  cancels the request.

## Workflow

1. **Request** — call `request_feedback`:
   - `host_id`: your host family (reasonix / claude / codex / …).
   - `host_session_id`: a stable, self-generated identifier for the current
     session, unique enough not to collide across sessions. Prefer the host's
     real session id when available; otherwise generate one once per session
     and reuse it, e.g. `<host>-<YYYYMMDD-HHMMSS>-<random hex>`
     (`reasonix-20260813-050001-a1b2c3`). Keep it constant for the whole
     session — do not mint a new one per request. This is only an application
     correlation id; it is not an MCP transport session and is not needed to
     retrieve feedback.
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
   - **Single active request**: before creating a request, make sure this host
     session has no earlier request still waiting or in progress. Never call
     `request_feedback` concurrently, in parallel tool calls, or from multiple
     subagents for the same session. Finish and read (or explicitly cancel) the
     active request before creating the next one. If several topics need human
     input, queue them and send them sequentially.
2. **Wait** — do not poll after creation. Discover the host's interactive
   confirmation tool instead of relying on hard-coded names such as `ask`,
   `ask_choice`, or `ask_user_question`. If a current tool can block until the
   human finishes, use it to wait and, when the schema supports choices, offer
   exactly two options: "Completed the ramble" and "Cancel the ramble" — do not
   add more options. If there is no such tool available in the current mode, end
   the turn and resume when notified.
3. **Read** — once the human submits, call `get_feedback(request_id)` and read
   `feedback.md` plus any attachments. `request_id` is the only durable lookup
   key. After any MCP disconnect or reconnect, call `get_feedback` with that
   same id; never create a replacement request because a transport reports
   "Session not found".
4. **Implement** — apply the feedback item by item; when you need another
   confirmation or review, go back to step 1.
5. **Cancel** — if the human explicitly gives up, call `cancel_feedback`.

## Principles

- Persist first: a created request is durable and survives disconnect/restart.
- MCP transport state is disposable. Losing it never means the durable feedback
  request was lost; reconnect and read the original `request_id`.
- An explicit invocation is not complete until one of these is true: the
  request was created and the feedback package was read, the host requires a
  manual continuation after request creation, the user cancelled, or no
  RambleDesk request tool is available after discovery.
- One active request per host session: serialize RambleDesk requests even when
  the host can issue parallel MCP calls. A terminal or explicitly cancelled
  request releases the session for the next request.
- Never set `allow_finish` for requests that need detailed feedback (experience,
  checks, opinions); the human should submit a feedback body, not a shortcut.
- One topic per request; split multiple topics into multiple requests.
- Restate your understanding before acting on feedback; if it is ambiguous,
  clarify with another ramble instead of guessing.
