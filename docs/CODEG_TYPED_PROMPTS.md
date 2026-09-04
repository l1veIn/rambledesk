# Codeg reference: typed prompt content

The prompt-block mapping follows [Codeg `map_prompt_blocks`](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs) at revision `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`. RambleDesk uses its own validated domain input and existing turn lifecycle. It does not adopt Codeg's vendor-specific capability overrides or binary-resource fallback.

`send_managed_prompt_content` accepts optional leading text followed by ordered text, image, resource-link, or embedded-text-resource blocks. The earlier `send_managed_prompt` entry point and feedback continuation remain compatible. Both entry points share turn serialization, cancellation, recovery checkpoints, and activity persistence.

Image, audio, and embedded-context capability facts come from the Agent's initialize response. Resource links are part of the negotiated ACP baseline. The current input contract supports images and text resources; it does not offer audio blocks. Unsupported inputs fail before starting a turn or saving a user message.

The core input limit is 16 blocks and 4 MiB of content fields in total. All text and embedded resource text together have a 256 KiB limit. Each image has a 2 MiB **encoded base64** limit, corresponding to at most 1,572,864 decoded bytes. PNG, JPEG, GIF, and WebP inputs must contain valid base64 and a matching file signature. These are format-boundary checks, not a full image decode. URI references are limited to `file`, `http`, and `https` and are never automatically opened by this mapping. Embedded browser uploads may instead use `ramble-attachment://<uuid>/<encoded filename>`; this scheme requires a UUID authority and one valid encoded filename, forbids user information/query/fragment, and is not accepted for resource links.

Accepted content reaches the Agent without clipping. Durable history stores one structured user activity with the existing bounded display preview: oversized inline media is omitted from the preview while its metadata and the user's text remain visible, with `truncated: true`. Such a history preview is not a replay payload.

Tests cover input bounds and capability checks, exact small-image transfer, ordered text/resource mapping, large-image transfer with bounded history, rejection before turn creation, cancellation, plain-text compatibility, and reopening persisted history. These stdio fixtures do not assert additional real-backend or manual UI acceptance.
