# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**A local, experiential feedback desk for coding agents.**

An agent sends a structured request. You use the product for real, talk through what
you notice, capture the screen, and submit an image-rich feedback package. The agent
receives that package in the same task and keeps working.

> The agent asks for real-world feedback. You ramble. RambleDesk turns it into a
> durable artifact the agent can act on.

## Why RambleDesk?

Agent workflows are good at implementation, but human feedback is still usually
buried in chat: a short question, a vague reply, a screenshot pasted much later, or a
manual request to resume the task.

RambleDesk makes that handoff a first-class loop:

1. The agent creates a feedback request with context and concrete actions.
2. RambleDesk notifies the human and preserves the request locally.
3. The human uses the target product while recording voice, screenshots, files, and
   explicit clipboard imports in one Ramble.
4. RambleDesk publishes an immutable Markdown feedback package with its attachments.
5. When the session is resumed, `get_feedback` returns the complete package to the
   agent without token-burning polling.

Requests survive MCP disconnects and application restarts. The feedback lifecycle is
durable; a single HTTP connection is only a delivery mechanism.

## Building RambleDesk with RambleDesk

RambleDesk is now dogfooded as part of its own development process.

For a meaningful UI or interaction change, the coding agent can implement the change,
launch the app, and call `request_feedback`. The developer then tests the real desktop
build, rambles while using it, adds captures where useful, and submits. The same agent
task wakes with the resulting manifest, Markdown, and attachment paths and can continue
the iteration immediately.

This repository therefore serves both as the product implementation and as an ongoing
test of the product's core promise: human judgment should strengthen an agent loop
without breaking its context.

## Current capabilities

- Durable feedback inbox backed by SQLite, with waiting, in-progress, completed, and
  cancelled request states.
- Unified Ramble session with local streaming speech transcription, region capture,
  file import, explicit clipboard import, and pause/resume/exit controls.
- Compact, draggable, icon-only floating console positioned out of the way at the
  right-center of the screen.
- Tiptap rich-text feedback editor with images embedded in the document flow.
- Immutable Feedback Packages containing `feedback.md`, `manifest.json`, and
  attachments.
- Authenticated loopback MCP server with automatic configuration support for local
  agent tools.
- Durable request create/get/cancel, full-package return on completed `get_feedback`,
  and automatic MCP host registration with `RAMBLEDESK_HOST`.
- Chinese and English UI, light and dark appearance modes, and optional system
  notifications.

RambleDesk is under active development. The native macOS workflow has been dogfooded
end to end; Linux and Windows are continuously covered by CI, with platform-specific
acceptance tracked separately.

## MCP tools

| Tool | Purpose |
| --- | --- |
| `request_feedback` | Create or reconnect to a durable feedback request; end the turn after. |
| `get_feedback` | Read status; when completed, return the full feedback package. |
| `cancel_feedback` | Explicitly cancel an unfinished request. |

The normal path is `request_feedback`, then end the agent turn (do not poll). After the
human submits and the session is resumed, call `get_feedback`. Auto-install writes
`RAMBLEDESK_HOST` / `X-RambleDesk-Host` so each host identity is known.

## Development quick start

### Prerequisites

- Rust 1.91.1 (pinned by `rust-toolchain.toml`)
- Node.js 22.23.0
- pnpm 10.12.4
- The platform prerequisites required by Tauri 2

Install dependencies and launch the native desktop app:

```bash
pnpm install --frozen-lockfile
pnpm dev
```

Open **Settings → MCP** in RambleDesk to detect supported local tools, install the
configuration in one click, or copy the authenticated Streamable HTTP configuration.
Restart the target agent tool after its configuration changes.

For a browser-only UI development session:

```bash
pnpm dev:web
```

The browser build deliberately degrades native-only behavior and is not a substitute
for desktop acceptance.

### Local speech model

Streaming transcription uses the manifest-pinned Sherpa X-ASR model described in
[`crates/rambledesk-speech/models/sherpa-x-asr.json`](crates/rambledesk-speech/models/sherpa-x-asr.json).
The model binary is intentionally kept out of Git. Place the verified model directory
at the platform application-data location declared by the manifest, or point a local
development run at it with an absolute path:

```bash
RAMBLEDESK_SHERPA_MODEL_DIR=/absolute/path/to/sherpa-x-asr pnpm dev
```

Text, capture, import, editing, and feedback submission remain usable without speech
transcription.

## Verification

Run the same core gates used by CI:

```bash
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
pnpm check
pnpm test
pnpm build:web
pnpm contracts:check
pnpm mcp:self-test
pnpm mcp:inspector-smoke
```

Build the native application with:

```bash
pnpm build
```

## Architecture

RambleDesk is a pnpm and Cargo monorepo with a thin Tauri/Svelte desktop shell around
Rust domain services:

```text
apps/desktop                 Tauri 2 + Svelte 5 desktop application
crates/rambledesk-core       Domain model, state machine, ports, and use cases
crates/rambledesk-storage    SQLite, drafts, attachments, and package publication
crates/rambledesk-mcp        Authenticated Streamable HTTP MCP adapter
crates/rambledesk-speech     Native audio capture and local streaming transcription
crates/rambledesk-cli        Headless development and protocol verification entrypoint
```

The core owns product semantics. Storage, MCP, speech, CLI, and Tauri are adapters; no
adapter may become a second source of business state.

## Documentation

| Document | Contents |
| --- | --- |
| [Product constitution](docs/CONSTITUTION.md) | North star, non-negotiable principles, and the User_0 boundary. |
| [Product specification](docs/PRODUCT.md) | Problem, scope, primary flow, information architecture, and recovery model. |
| [Architecture](docs/ARCHITECTURE.md) | Monorepo boundaries, runtime composition, and consistency rules. |
| [MCP and feedback protocol](docs/PROTOCOL.md) | Tool schemas, idempotency, lifecycle, errors, and security. |
| [Development baseline](docs/DEVELOPMENT.md) | Stack decisions, data model, milestones, and acceptance gates. |
| [Compatibility matrix](docs/COMPATIBILITY.md) | Tested MCP clients, protocol versions, authentication, and execution modes. |
| [Dogfooding log](docs/DOGFOODING.md) | Real operator rounds, findings, fixes, and validation evidence. |
| [Kotone reuse audit](docs/KOTONE_REUSE.md) | Reusable speech components, required changes, and license gates. |
| [Design interview](docs/INTERVIEW.md) | Historical decision context; not a current specification. |

## License

To be decided.
