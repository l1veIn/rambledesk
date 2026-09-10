# Live session configuration

Controls are derived from `SessionRuntime.configuration` and connected through
`setManagedSessionConfig`. They never invent choices or update launch profiles.
Modern options with the `model` or `mode` category replace the corresponding
legacy catalog. Unknown current values remain visible rather than silently
selecting the first supported choice.

Only confirmed snapshot values are displayed. Native select changes are reset
before awaiting the mutation; refusal therefore keeps the confirmed value.
Requests are serialized and disabled during a prompt, permission wait, lifecycle
operation or deletion. The session controller refreshes after either outcome.

Presentation is adapted from Codeg
`src/components/chat/session-config-selector.tsx`, commit
`3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`, Apache-2.0. The Svelte port uses native
grouped selects and boolean chips, retains opaque option IDs, and uses actual
JSON booleans from RambleDesk's generated contract. The projection/fallback
adapter and tests are new RambleDesk code.
