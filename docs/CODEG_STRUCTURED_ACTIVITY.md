# Codeg reference: structured session activity

This slice adapts Codeg's structured message and tool lifecycle model to RambleDesk's durable session activity. Reference revision: `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`.

Sources:

- [Codeg `session_state.rs`](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/session_state.rs): `LiveContentBlock`, `ToolCallState`, `upsert_tool_call`, and `push_tool_call_ref_if_absent` inform ordered content blocks, field-preserving updates, and anchoring each tool at its first occurrence.
- [Codeg `types.rs`](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/types.rs): the `AcpEvent` tool call/update fields inform preservation of tool metadata and output content.

The RambleDesk implementation is newly written around `SessionActivityRepository`; it does not copy Codeg's transient session ownership or vendor-specific raw-input chunk heuristics. Standard ACP updates replace each supplied whole field. Missing fields retain their previous value; an explicit empty content/location collection clears that field. Raw input and output are display strings containing bounded serialized JSON. A truncated preview need not be valid complete JSON and is never used as executable tool input.

`SessionActivity.content` is optional. Migration 0017 leaves earlier text-only history unchanged. New agent message/thought rows retain ordered text, image, audio, resource, diff, and terminal references; existing `text` remains a compatibility summary. Tool activity keeps one row per tool ID in each local session and turn. Updating that row preserves its sequence and does not split a later message that is still streaming. Replay outside an active turn and events from a retired instance remain ignored by the existing runtime guard.

Display limits are explicit: raw input and raw output each have a 64 KiB limit; inline image/audio base64 has a 256 KiB limit per payload; messages and tool content share a 256 KiB content budget with at most 64 blocks. Tool locations have at most 64 entries. The complete typed activity payload has a 512 KiB string budget. Oversized text is clipped at a UTF-8 boundary. Oversized media data is omitted entirely while its metadata survives; `truncated` makes the omission visible. Resource and terminal references are recorded without opening them or claiming additional ACP client capabilities.

Verification includes upgrading a version-16 database, scoped atomic updates and reload, partial tool updates, non-text content, UTF-8/JSON/media limits, text/tool/thought interleaving, repeated tool IDs in two sessions and turns, and late-event suppression after turn completion. Existing ACP fixture coverage also verifies cancellation, two simultaneous instances, and resume without transcript replay. These tests validate the persisted model and event mapping; they do not constitute new real-backend or manual UI acceptance.
