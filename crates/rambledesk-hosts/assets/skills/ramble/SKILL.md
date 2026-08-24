---
name: ramble
description: >-
  Run a human-feedback loop through RambleDesk. Hard trigger: when the user
  invokes /ramble, $ramble, [$ramble], or says to use RambleDesk/ramble
  feedback, create a RambleDesk request before answering the substance unless
  they explicitly say not to send one. Also use when real human judgment,
  review, approval, or hands-on checking is needed.
---

# ramble — run a human-feedback loop through RambleDesk

Route anything that needs a real human's judgment, review, approval, or hands-on
experience through the RambleDesk feedback loop instead of asking in chat.

## Invocation contract

- Treat explicit invocation as a command, not a hint. If the user invokes
  `/ramble`, `$ramble`, `[$ramble]`, names this skill as the way to handle the
  task, or asks for RambleDesk feedback, create the first RambleDesk request
  before substantive advice, design, review, or implementation.
- Treat `/ramble [task]` as a task-scoped loop. Guarantee the first request,
  then create later serialized requests only when the same task needs another
  clarification, review, or final confirmation. Do not carry the loop into an
  unrelated future task.
- Treat an empty argument or a generic starter such as "start this Ramble" or
  "Let's work on something together" as no meaningful task:
  - If the conversation already contains one clear active task, summarize the
    current understanding in the kickoff request and ask the human to confirm
    or correct it while supplying any missing context, constraints, desired
    output, priorities, and completion criteria.
  - Otherwise create a kickoff request asking the human for the goal, relevant
    context and materials, constraints, desired output, priorities, and
    completion criteria.
  - Ask for this task brief in RambleDesk, never in the host chat.
- Treat `/ramble_on` and `/ramble_off`, when the host provides them, as
  persistent-mode switches rather than one-task skill invocations.
- Use this skill implicitly when human testing, visual inspection, screenshots,
  clarification, or a decision would materially improve the result.
- Do not treat a meta request to inspect, edit, or debug this skill as a request
  to create feedback unless the user also explicitly asks to run the loop.

## Select the available flow

Prefer the native adapter flow when a tool equivalent to
`request_ramble_feedback` is available. Otherwise use the Generic MCP flow with
a tool equivalent to `request_feedback`. Discover renamed RambleDesk tools by
capability before deciding that no request tool exists.

### Native adapter flow

1. Call `request_ramble_feedback` with `wait: true` (the default). Omit
   `host_id` and `host_session_id`; the adapter derives its host identity and
   current session.
2. Let the tool call wait until the human submits or cancels. Do not start a
   separate host confirmation while this native wait is active.
3. Read the returned feedback markdown and attachment paths directly from the
   completed tool result.
4. If the wait is interrupted, call `resume_ramble_feedback` to reconnect to
   the durable request. Use `get_ramble_feedback(request_id)` only for a
   non-blocking state read and `cancel_ramble_feedback` for explicit
   cancellation.

### Generic MCP flow

1. Call `request_feedback` with:
   - `host_id`: the current host family (claude, codex, cursor, reasonix, …).
   - `host_session_id`: the real host session id when available; otherwise
     generate a stable identifier once for this session and reuse it, for
     example `<host>-<YYYYMMDD-HHMMSS>-<random hex>`.
2. After creation, do not poll. Discover an interactive host tool whose schema
   can block until the human finishes. When it supports choices, offer exactly
   "Completed the ramble" and "Cancel the ramble". If no blocking host tool is
   available, end the turn and resume when notified.
3. After completion, call `get_feedback(request_id)`. Treat `request_id` as the
   durable lookup key across MCP disconnects; never create a replacement merely
   because the transport reports "Session not found".
4. Call `cancel_feedback` only when the human explicitly gives up.

## Build every request

- Use a short, specific `title` that the human can scan in the Inbox.
- Explain in `what_happened` what the agent understands, why feedback is needed,
  and what perspective the human should bring.
- Provide an ordered, executable `actions` checklist with one action per item.
- Attach files only when the human needs to review them. Prefer an absolute
  local `attachments[].path` for existing images and Markdown. Use inline
  `markdown` only for short text and `contents_base64` only for a small image
  that is not on disk.
- Omit `request_id`; the server generates it. Reuse the returned id for recovery.
- Omit `allow_finish` by default. Set it with `final_summary` only for a simple
  approve/reject decision that needs no detailed feedback body.
- Keep one active request per host session. Never create requests concurrently,
  in parallel tool calls, or from multiple subagents for the same session.
  Finish, read, or explicitly cancel the active request before creating another.

## Continue the task

1. Form a working brief from the returned feedback before acting.
2. Apply the feedback item by item.
3. If the same task needs another clarification, review, or final confirmation,
   create the next request through the same selected flow.
4. Once the task is complete, leave the task-scoped loop. Do not affect the next
   unrelated task unless persistent mode is enabled.

## Principles

- Persist first: a created request survives disconnects and restarts.
- Keep transport state disposable and durable request state authoritative.
- Serialize requests and keep one topic per request.
- Do not set `allow_finish` for experience reports, checks, opinions, or other
  requests that need a feedback body.
- Clarify ambiguous feedback with another RambleDesk request instead of
  guessing or moving the clarification into host chat.
- Consider an explicit invocation complete only after the request and returned
  feedback have been handled, the host requires a later manual continuation,
  the human cancelled, or no RambleDesk request tool exists after discovery.
