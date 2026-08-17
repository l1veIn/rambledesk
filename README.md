# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

![RambleDesk](docs/social/ramble-banner-en-1400x700.webp)

**Humans, as an API, for coding agents.** Collect your rambling, shape it into prompts.

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

**Tools inside a Ramble** — everything you capture lands in the feedback package, in place:

- **Screen capture** — select a region and annotate it (draw, arrows, text); the image is inserted at your cursor.
- **Clipboard import** — pull copied text or images into the document in one click, with a label and timestamp.
- **File import** — attach images or any file via picker or drag-and-drop, up to 20 MiB each.
- **Paste** — paste a screenshot straight into the document.
- **Voice with on-device transcription** — talk while you use the product; speech is transcribed locally (X-ASR, SenseVoice, or FunASR-Nano) into the body.
- **Attachment management** — insert at the cursor, reorder, or remove; previews update in place.

Beyond the Ramble:

- Durable Inbox backed by SQLite, with waiting, in-progress, completed, and cancelled states.
- Immutable Feedback Packages containing `feedback.md`, `manifest.json`, and attachments.
- Authenticated loopback local server with `/api/feedback/request|get|wait|cancel` and `/mcp`.
- Generic MCP Adapter scheme with `request_feedback`, `get_feedback`, and `cancel_feedback`, plus a host detect/install engine that consumes the host knowledge registry.
- Pi native package at `packages/pi-rambledesk`, using the local JSON API and blocking inside the Pi tool call.
- DeepSeek Harness (dsh) native plugin at `packages/dsh-rambledesk`, using the local JSON API and blocking inside the dsh tool call.
- Adapter settings for Generic MCP hosts, Pi package installation, and dsh plugin installation.
- First-run setup for language, data location, local speech, adapters, notifications, and optional Cooking; it can be rerun from **Settings → General**.
- Chinese and English UI, light and dark appearance modes, tray entry points, and optional system notifications.

## Download and install

Download release builds only from the official [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) page. Before overriding an operating-system warning, confirm that the file came from this repository and compare its SHA-256 digest with `SHA256SUMS.txt` from the same release.

### macOS (Apple Silicon)

The current macOS build is ad-hoc signed but is **not notarized by Apple**. Gatekeeper may therefore block the first launch even when the downloaded DMG is intact.

1. Download the `aarch64.dmg` file and `SHA256SUMS.txt` from the same release, then verify the DMG in Terminal:

   ```bash
   cd ~/Downloads
   grep 'aarch64.dmg' SHA256SUMS.txt | shasum -a 256 -c -
   ```

   Continue only when the result says `OK`.

2. Open the DMG, then drag **RambleDesk** onto the **Applications** folder shown in the installer window. Eject the RambleDesk disk image afterward.
3. Open **Applications** in Finder, Control-click or right-click **RambleDesk**, choose **Open**, then confirm **Open** if macOS offers it.
4. If macOS still blocks the app, try to open it once, then go to **System Settings → Privacy & Security**. In the Security section choose **Open Anyway** and authenticate. Apple normally keeps this button available for about an hour after the blocked launch. See [Apple's official instructions](https://support.apple.com/guide/mac-help/mh40616/mac).
5. If macOS instead reports that RambleDesk “is damaged and can't be opened,” first make sure the SHA-256 digest matches the release. Then remove only the downloaded-file quarantine attribute and open the app again:

   ```bash
   xattr -dr com.apple.quarantine /Applications/RambleDesk.app
   ```

   `sudo` is not normally required. Do **not** bypass a “will damage your computer” or malware warning, and do not continue when the checksum differs.
6. During first-run setup, allow **Screen & System Audio Recording** and **Microphone** when you want capture and voice transcription. If RambleDesk says a restart is required after screen permission is granted, restart it before taking a capture.

To uninstall the app, quit RambleDesk and move `/Applications/RambleDesk.app` to the Trash. This intentionally leaves your local feedback library and settings in place so reinstalling does not destroy your work.

### Windows

Download the `.exe` installer from the same release and run it. Until Windows Authenticode signing is added, SmartScreen may show **Windows protected your PC**. Only after checking the download and SHA-256 digest, choose **More info → Run anyway**. The installer and in-app updater are separate from the unsigned macOS delivery path.

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

Open **Settings → Adapters** in RambleDesk to detect supported local tools, install Generic MCP configuration, install the Pi package, install the DeepSeek Harness plugin, or copy the authenticated Streamable HTTP configuration.

## First-run setup

A fresh install opens a short setup flow before the workbench. Choose the data location first: feedback attachments, published packages, and speech models are written there. Choosing a different location saves it and restarts RambleDesk before later setup steps download a model or create feedback. The flow then offers a local speech model, a recommended Pi native adapter (automatic continuation in the same tool call), optional Generic MCP hosts (manual continuation), notification permissions, and optional Feedback Cooking. Run it again anytime from **Settings → General → Run getting started again**.

For browser-only UI development:

```bash
pnpm dev:web
```

The browser build degrades native-only behavior and is not a substitute for desktop acceptance.

## Local Speech Model

RambleDesk supports X-ASR streaming transcription plus VAD-segmented SenseVoice and FunASR-Nano transcription. Open **Settings → Voice** to choose, download, switch, or remove a model and tune the Silero VAD threshold. Model manifests live under [`crates/rambledesk-speech/models`](crates/rambledesk-speech/models); downloaded model weights remain outside Git in the configured data storage location.

Text, capture, import, editing, and feedback submission remain usable without speech transcription.

## Feedback Cooking

Optional Cooking is configured under **Settings → General**. RambleDesk uses the Vercel AI SDK with DeepSeek, OpenAI, or a user-supplied OpenAI-compatible endpoint to turn the uncooked Ramble draft into formal Markdown before submission. Every new feedback package preserves the human source as `uncooked.md`; `feedback.md` is the canonical result read by the host. API keys remain in local device settings and are never written to packages.

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
crates/rambledesk-mcp          Generic MCP Adapter scheme (tool surface + host installer)
crates/rambledesk-hosts        Host knowledge registry, profiles, continuation strategies
crates/rambledesk-speech       Native audio capture and local streaming transcription
crates/rambledesk-cli          Headless development and protocol verification entrypoint
packages/pi-rambledesk         Pi native adapter package
packages/dsh-rambledesk        DeepSeek Harness (dsh) native adapter plugin
```

`core` owns the application contract. Storage, local server, host knowledge, speech, CLI, and Tauri are infrastructure or composition layers; the Generic MCP, Pi, and dsh packages participate in complete host-facing adapters. None of these layers may become a second source of business state.

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
| [macOS distribution](docs/MACOS_DISTRIBUTION.md) | Unsigned Apple Silicon DMG first; Developer ID later. |

## License

To be decided.
