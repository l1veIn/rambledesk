# Typed prompt attachments

The managed workspace supplies the composer's real chooser and clipboard providers.
Only explicitly selected/pasted `File` objects are read. There is no project search
or guessed filesystem path. Image and embedded-context controls follow the active
Agent's negotiated prompt capabilities; unsupported media produces a visible error.

- PNG, JPEG, GIF and WebP headers are checked; decoded images are capped at 1.5 MiB.
- Text files must decode as UTF-8 without binary control bytes. Prompt text and all
  embedded text together are capped at 256 KiB.
- The message is capped at 16 total blocks, including its text block, and 4 MiB of
  encoded content. Bounds are checked before clearing the draft or sending.
- Browser text uploads use `ramble-attachment://<uuid>/<encoded filename>` as the
  identity of content already embedded in the request. This is not a local file
  path, external link or instruction for the Agent to fetch anything.

`SessionPromptDrafts` owns text and attachments by local session ID. Beginning a
submission clears both immediately. A failed submission restores both only if no
new edit has occurred since that clear. Later file reads append only to their
captured session; deletion prevents them from recreating a draft. Drafts stay in
memory and are not written to logs, local storage or user files.
