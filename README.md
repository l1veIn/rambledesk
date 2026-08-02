# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**A local, experiential feedback desk for coding hosts.**

A coding host sends a structured request. You use the target product for real, talk through what you notice, capture the screen, and submit an image-rich feedback package. The host reads that package and continues with durable evidence instead of scattered chat.

## Why RambleDesk?

Modern coding tools can implement quickly, but human feedback is still usually trapped in chat: a short question, a vague reply, a screenshot pasted later, or a manual reminder to continue.

RambleDesk makes that handoff a first-class loop:

1. A host creates a feedback request with context and concrete actions.
2. RambleDesk notifies the human and persists the request locally.
3. The human uses the target product while recording voice, screenshots, files, and explicit clipboard imports in one Ramble.
4. RambleDesk publishes an immutable Markdown feedback package with attachments.
5. The host reads the package through `get_feedback` and continues.

Requests survive transport disconnects and application restarts. The feedback lifecycle is durable; a single HTTP connection is only a delivery attempt.

## Current Capabilities

- Durable Inbox backed by SQLite, with waiting, in-progress, completed, and cancelled states.
- Request Workspace with rich-text editing, screenshots, attachments, clipboard capture, and local streaming speech transcription.
- Immutable Feedback Packages containing `feedback.md`, `manifest.json`, and attachments.
- Authenticated loopback local server with `/api/feedback/request|get|wait|cancel` and `/mcp`.
- Thin Generic MCP Adapter with `request_feedback`, `get_feedback`, and `cancel_feedback`.
- Pi native package at `packages/pi-rambledesk`, using the local JSON API and blocking inside the Pi tool call.
- Adapter settings for Generic MCP hosts and Pi package installation.
- Chinese and English UI, light and dark appearance modes, tray entry points, and optional system notifications.

## Development Quick Start

Prerequisites:

- Rust 1.91.1, pinned by `rust-toolchain.toml`
- Node.js 22.23.0
- pnpm 10.12.4
- Platform prerequisites required by Tauri 2

Install dependencies and launch the native desktop app:

```bash
pnpm install --frozen-lockfile
pnpm dev
```

Open **Settings → Adapters** in RambleDesk to detect supported local tools, install Generic MCP configuration, install the Pi package, or copy the authenticated Streamable HTTP configuration.

For browser-only UI development:

```bash
pnpm dev:web
```

The browser build degrades native-only behavior and is not a substitute for desktop acceptance.

## Local Speech Model

Streaming transcription uses the manifest-pinned Sherpa X-ASR model described in [`crates/rambledesk-speech/models/sherpa-x-asr.json`](crates/rambledesk-speech/models/sherpa-x-asr.json). The model binary is intentionally kept out of Git. Place the verified model directory at the platform application-data location declared by the manifest, or point a local development run at it with an absolute path:

```bash
RAMBLEDESK_SHERPA_MODEL_DIR=/absolute/path/to/sherpa-x-asr pnpm dev
```

Text, capture, import, editing, and feedback submission remain usable without speech transcription.

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
pnpm test:pi
```

Build the native application with:

```bash
pnpm build
```

## Architecture

```text
apps/desktop                   Tauri 2 + Svelte 5 workbench and composition root
crates/rambledesk-core         Application contract, state machine, ports, use cases
crates/rambledesk-storage      SQLite, drafts, attachments, package publication
crates/rambledesk-local-server Loopback listener, auth, JSON API, route mounting
crates/rambledesk-mcp          Generic MCP Adapter thin layer
crates/rambledesk-hosts        Host profiles and continuation strategies
crates/rambledesk-speech       Native audio capture and local streaming transcription
crates/rambledesk-cli          Headless development and protocol verification entrypoint
packages/pi-rambledesk         Pi native adapter package
```

`core` owns the application contract. Storage, local server, host profiles, speech, CLI, and Tauri are infrastructure or composition layers; the Generic MCP and Pi packages participate in complete host-facing adapters. None of these layers may become a second source of business state.

## Documentation

| Document | Contents |
| --- | --- |
| [Terminology](docs/TERMINOLOGY.md) | The only source for product terms, protocol fields, and package boundaries. |
| [Product constitution](docs/CONSTITUTION.md) | North star and non-negotiable principles. |
| [Product specification](docs/PRODUCT.md) | Scope, primary flows, information architecture, and recovery model. |
| [Architecture](docs/ARCHITECTURE.md) | Runtime topology, crate boundaries, and consistency rules. |
| [Protocol](docs/PROTOCOL.md) | Tool schemas, local JSON API, idempotency, lifecycle, errors, and security. |
| [Development baseline](docs/DEVELOPMENT.md) | Stack decisions, data model, and acceptance gates. |
| [Adapter verification](docs/COMPATIBILITY.md) | Tested host paths, protocol versions, authentication, and execution modes. |
| [Dogfooding log](docs/DOGFOODING.md) | Real operator rounds, findings, fixes, and validation evidence. |
| [Kotone reuse audit](docs/KOTONE_REUSE.md) | Reusable speech components, required changes, and license gates. |

## License

To be decided.
