# Agent composer

Svelte port of the plain-text composer in [Codeg](https://github.com/xintaofei/codeg/tree/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src/components/chat/composer), pinned to `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1` (Apache-2.0). Derived files identify their source and modifications. Upstream's React components, state hooks, routing URIs, and installation behavior are not imported.

`AgentComposer.svelte` accepts controlled `value`, `draftKey` (the local session ID), `onchange(text)`, and `onsubmit(text)`. The parent owns draft persistence, submit and restore transitions. The managed workspace clears a submission immediately so the user can write the next message while the agent works; a failed submission is restored only if no newer edit has occurred. Prefer a keyed component per session as well as passing `draftKey`.

`disabled` makes the editor read-only. `busy` allows editing but prevents submission. `sendDisabled` blocks submission while retaining editing, for example before an offline agent has connected; it does not imply that the agent is running. Pass `oncancel` only when the active turn really can be cancelled. Submission errors retain the draft, and late callbacks from another `draftKey` cannot change the visible operation state. `submitShortcut` is `enter` (default) or `mod+enter`; composition flags and the legacy IME 229 key are always guarded.

Literal Markdown quote markers typed or pasted into the editor retain their bytes when sent. The shared schema continues to read file-reference atoms from native editor clipboard slices and serializes them as escaped file links. There is no file-search provider in the application, so no autocomplete menu or unused provider lifecycle is mounted.

Attachments belong to the Ramble request workflow. The Agent composer accepts text and file references; file paste/drop directs the user to the Ramble request. The named `footer` slot holds the agent identity and negotiated session controls.

Shared modules keep the live editor and headless tests on the same Tiptap schema. `to-prompt-blocks.ts` contains the text and clipboard serializers; `plain-text-content.ts` restores controlled string drafts. No Markdown parser or formatting input rules are loaded.

Upstream mapping:

- `editor-config`, `plain-text-content`, `to-prompt-blocks`: reduced to existing RambleDesk text input capabilities and installed Tiptap packages. The unused typed-block conversion wrappers were removed.
- `quote-decoration`, `inactive-selection`, `message-quote`, `ime-composition`: framework-free logic retained with local imports and scoped CSS names.
- `submit-key` and its tests: retained; keyboard matching is limited to the supported Enter chords.
- `reference-node`: native DOM representation and safe file-link serialization replace React node views. The unused `reference-search` provider and menu were removed.
- `AgentComposer`: Svelte integration, controlled draft lifecycle and localized chrome. The earlier attachment hooks and transcript quote action were removed when those interactions moved to Ramble requests or native text selection.

Validation: `pnpm --filter rambledesk-desktop test -- src/lib/agents/composer` uses real headless Tiptap editors for serialization, quote decorations, clipboard slices, history reset, and reference atoms. IME handling is tested independently because a simulated DOM cannot faithfully reproduce native composition behavior.
