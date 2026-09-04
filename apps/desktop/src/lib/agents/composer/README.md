# Agent composer

Svelte port of the plain-text composer in [Codeg](https://github.com/xintaofei/codeg/tree/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src/components/chat/composer), pinned to `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1` (Apache-2.0). Derived files identify their source and modifications. Upstream's React components, state hooks, routing URIs, and installation behavior are not imported.

`AgentComposer.svelte` accepts controlled `value`, `draftKey` (the local session ID), `onchange(text)`, and `onsubmit(text)`. The parent owns draft persistence, submit and restore transitions. The managed workspace clears a submission immediately so the user can write the next message while the agent works; a failed submission is restored only if no newer edit has occurred. Prefer a keyed component per session as well as passing `draftKey`.

`disabled` makes the editor read-only. `busy` allows editing but prevents submission. `sendDisabled` blocks submission while retaining editing, for example before an offline agent has connected; it does not imply that the agent is running. Pass `oncancel` only when the active turn really can be cancelled. Submission and provider errors retain the draft, and late callbacks from another `draftKey` cannot change the visible operation state. `submitShortcut` is `enter` (default) or `mod+enter`; composition flags and the legacy IME 229 key are always guarded.

The imperative handle exposes `focus()`, `getText()`, `insertText(text)`, and `insertQuote(text)`. Quoting appends literal Markdown quote markers with paragraph separation; decoration changes only their appearance, never the bytes sent.

`referenceSearch(query, { signal })` is optional. Its results are real `ComposerReference` file URIs supplied by a host capability. Only then does `@` autocomplete and its toolbar button appear. Results are aborted and discarded when stale. A selected reference is an inline atom serialized to an escaped file link; persisted string drafts restore that link literally, preserving the send text without inventing access to the file.

The optional `attachments`, `onAddAttachments`, `onRemoveAttachment`, and `onPasteFiles` hooks delegate all attachment ownership, reading, and encoding to the host. Without these capabilities, no attachment button appears and file paste/drop reports unsupported input. Do not supply attachment providers to a text-only backend. The named `footer` slot is available for real host controls such as a negotiated model selector.

Shared modules keep the live editor and headless tests on the same Tiptap schema. `to-prompt-blocks.ts` and `from-prompt-blocks.ts` serialize the editor's text only; the managed workspace separately combines that text with typed attachment content. Unsupported editor block kinds fail explicitly rather than being silently dropped. No Markdown parser or formatting input rules are loaded.

Upstream mapping:

- `editor-config`, `plain-text-content`, `to-prompt-blocks`, `from-prompt-blocks`: reduced to existing RambleDesk text input capabilities and installed Tiptap packages.
- `quote-decoration`, `inactive-selection`, `message-quote`, `ime-composition`: framework-free logic retained with local imports and scoped CSS names.
- `submit-key` and its tests: retained; keyboard matching is limited to the supported Enter chords.
- `reference-node`, `reference-search`: native DOM and provider-based file references replace React node views and Codeg service calls.
- `AgentComposer`: Svelte integration, controlled draft lifecycle, capability hooks, and localized chrome.

Validation: `pnpm --filter rambledesk-desktop test -- src/lib/agents/composer` uses real headless Tiptap editors for serialization, quote decorations, clipboard slices, history reset, and reference atoms. IME and asynchronous provider races are tested independently because a simulated DOM cannot faithfully reproduce native composition behavior.
