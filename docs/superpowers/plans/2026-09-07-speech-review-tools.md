# Speech Review Tools Implementation Plan

**Goal:** Add Tidy and inline editing to pending speech, plus optional automatic Tidy before confirmation.

**Architecture:** The main-window speech draft queue owns pending text, captured segment IDs, edit locks, Tidy status, and write acknowledgement. Overlay windows render snapshots and send commands; they do not invoke models or write feedback directly. The existing Tidy configuration and cleanup metadata are reused.

**Tech stack:** Svelte, TypeScript, existing Tauri overlay/event capabilities, Vitest.

**Spec:** User request in this conversation: four review actions; inline edit; conditional auto Tidy setting; notice about the existing automatic Tidy threshold. Continue on the current branch and preserve unrelated work.

## Behavior

- Tidy uses current Tidy credentials/model/prompt, returns text for review, and never confirms automatically.
- Auto Tidy defaults off and acts only on newly transcribed segments when confirmation is enabled.
- An edit snapshots selected IDs. New arrivals never replace the local edit buffer or get consumed by its save.
- Main owns the edit lock, suppresses global confirmation shortcuts, and rejects stale or malformed commands.
- Empty or failed Tidy keeps the original transcript and exposes the error. Explicit discard remains available.
- Previously attempted writes retain immutable text/IDs for idempotent retry.
- Cleaned text carries cleanupState='cleaned' through foreground and background insertion; raw/edited text remains pending.
- Native focus behavior stays unchanged; a textarea receives focus only after Edit. The capsule fits the existing 436px width / 480px height cap.

## Tasks

- [x] Queue: test frozen edit/Tidy selections, target isolation, new arrivals, stale completion, failure preservation, persistence and auto-mode gating; implement editing and Tidy states without delaying microphone stop.
- [x] Metadata: test cleaned speech exclusion from editor Tidy candidates and stable-ID deduplication; thread cleanupState through both insertion paths.
- [x] Commands/controller: validate review commands, suppress stale confirmation during editing, share Tidy config, and publish edit state to overlays.
- [x] Overlay: add four actions, bounded textarea with save/cancel, edit buffer preservation, busy/error state and keyboard focus.
- [x] Preferences/settings: persist opt-in speechAutoTidy; show it only with confirmation, explain Tidy threshold interaction in both locales.
- [x] Verification: run focused tests then frontend checks, browser-test the isolated real-component preview, review the final changes, and record limitations.

## Validation

Use the real queue and draft transformation functions with deterministic model/writer boundaries. Verify invalid commands and editing shortcuts cannot write stale text. Browser preview uses an in-memory writer and simulated Tidy, so no real credentials or microphone access are needed. Existing speech, draft, and editor tests remain the regression baseline. No commit or package release is part of this task.

Completed: 899 tests across 131 frontend test files; Svelte/TypeScript check with
zero errors/warnings; production web build and diff whitespace check passed.
Browser verification covered inline edit focus, later-arrival buffer preservation,
blocked confirmation during editing, save/cancel, manual and automatic Tidy,
cleaned metadata on confirmation, failed Tidy preserving original text, and the
conditional settings switch with retained preference. A 420px capsule had no
horizontal overflow; its editor uses a 160px maximum height with automatic scrolling.
Review also caught and fixed metadata loss at queue acceptance and the hidden-overlay
dock becoming inaccessible if collapsed during editing. Native microphone capture,
OS focus/shortcuts, and live model calls were not exercised in the browser preview.
