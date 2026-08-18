---
name: ramble
description: Run a human-feedback loop through RambleDesk. Use when the user needs real human judgment, review, approval, or a hands-on check of the work — persist a request via request_ramble_feedback, wait for the human to ramble in the RambleDesk desktop workbench, then read the immutable feedback package returned by the tool and implement it. Do not scatter these requests across chat.
---

# ramble — run a human-feedback loop through RambleDesk

Route anything that needs a real human's judgment, review, approval, or hands-on
experience through the RambleDesk feedback loop instead of asking in chat.

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
     directly. Do not call `ask_user_question` while waiting.
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
- Use `request_ramble_feedback` only when human testing, visual inspection,
  screenshots, clarification, or a decision would materially improve the
  result. Do not create a generic request merely because a task started or the
  agent is about to finish.
- Never set `allow_finish` for requests that need detailed feedback (experience,
  checks, opinions); the human should submit a feedback body, not a shortcut.
- One topic per request; split multiple topics into multiple requests.
- Restate your understanding before acting on feedback; if it is ambiguous,
  clarify with another ramble instead of guessing.
