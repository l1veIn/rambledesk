---
name: ramble
description: Run a human-feedback loop through RambleDesk. Use when the user needs real human judgment, review, approval, or a hands-on check of the work — persist a request via request_feedback, wait for the human to ramble in the RambleDesk desktop workbench, then read the immutable feedback package with get_feedback and implement it. Do not scatter these requests across chat.
---

# ramble — run a human-feedback loop through RambleDesk

Route anything that needs a real human's judgment, review, approval, or hands-on
experience through the RambleDesk feedback loop instead of asking in chat.

## Workflow

1. **Request** — call `request_feedback`:
   - `host_id`: your host family (reasonix / claude / codex / …).
   - `host_session_id`: a stable, self-generated identifier for the current
     session, unique enough not to collide across sessions. Prefer the host's
     real session id when available; otherwise generate one once per session
     and reuse it, e.g. `<host>-<YYYYMMDD-HHMMSS>-<random hex>`
     (`reasonix-20260813-050001-a1b2c3`). Keep it constant for the whole
     session — do not mint a new one per request.
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
2. **Wait** — do not poll after creation. If the host has an interactive
   confirmation tool (ask / ask_choice), use it to wait for the human and offer
   exactly two options: "Completed the ramble" and "Cancel the ramble" — do not
   add more options. If there is no such tool, end the turn and resume when
   notified.
3. **Read** — once the human submits, call `get_feedback(request_id)` and read
   `feedback.md` plus any attachments.
4. **Implement** — apply the feedback item by item; when you need another
   confirmation or review, go back to step 1.
5. **Cancel** — if the human explicitly gives up, call `cancel_feedback`.

## Principles

- Persist first: a created request is durable and survives disconnect/restart.
- Never set `allow_finish` for requests that need detailed feedback (experience,
  checks, opinions); the human should submit a feedback body, not a shortcut.
- One topic per request; split multiple topics into multiple requests.
- Restate your understanding before acting on feedback; if it is ambiguous,
  clarify with another ramble instead of guessing.
