# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**In 2026, the most expensive part of vibe coding is human attention.**

Working with a general-purpose agent often means doing three things:

1. Read a long, scattered account of what the model did.
2. Figure out what it actually finished—and what it needs from you.
3. Turn your thoughts into a well-written prompt for the next round.

RambleDesk turns the first two steps into a **review request**, and the third into **say it first, tidy it if you want**.

The agent must explain two things: **what just happened** and **what you should try or confirm**. You speak, take screenshots, and edit directly in a desktop workbench, without writing a prompt first. On submission, your original feedback, an optional refined version, and attachments become an immutable feedback package for the agent to continue from. Keep your attention on hands-on review and decisions.

Starting with **0.4.0**, RambleDesk connects to coding agents such as Claude Code, Codex CLI, and Gemini CLI through ACP. Use an agent that already works on your machine, follow the prompts to install any required connection component, and choose a project to begin.

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk product demo" width="960" />

<p><em>The feedback loop in an earlier release: the agent asks for a review → you speak and capture → submit feedback.</em></p>

</div>

## Quick start

Download **0.4.0** from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases), available for Windows x64 and macOS Apple Silicon. See the [release notes](docs/CHANGELOG.md) for changes.

1. **Connect an agent.** Make sure it works locally, then open **Settings → Agents** and follow the connection prompts. Sign-in and model access come from the agent itself.
2. **Give it a task.** Start a new session, choose an agent and project folder, and enter your goal.
3. **Try it, then respond.** When a Ramble request arrives, follow its review steps and record your feedback with voice, screenshots, or text. Submit it to queue continuation in the original agent session; delivery status and recovery actions stay visible in the workbench.

For voice input, download a local transcription model in **Settings → Voice** and allow microphone access. If you want a model to tidy your wording, configure a model service under **Settings → Post-processing**. This is off by default, and your original feedback is preserved.

<details>
<summary><strong>First-install notes</strong></summary>

**Windows:** Run `x64-setup.exe`. The installer is not yet Authenticode-signed. If SmartScreen blocks it, confirm it came from this repository, then choose **More info → Run anyway**.

**macOS:** Open the DMG and drag RambleDesk into Applications. The build is ad-hoc signed and not notarized. On first launch, right-click and choose **Open**. If it is still blocked, go to **System Settings → Privacy & Security**, find RambleDesk in the **Security** section, choose **Open Anyway**, and confirm **Open**.

If macOS reports the app as damaged, confirm the download source first, then run:

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

</details>

## Keep your agent workflow

The recommended path is to start conversations directly in RambleDesk through **ACP**. Model options, permissions, and session recovery depend on the connected agent. See the [ACP guide](docs/ACP_MANAGED_SESSIONS.md) for setup and recovery.

You can also stay in your usual agent app or CLI and connect the feedback workbench through **Settings → External adapters**. Continuation varies by adapter; generic MCP requires returning to the agent to continue.

## From source

With Node.js, pnpm, Rust, and your platform's Tauri build dependencies installed:

```bash
pnpm install --frozen-lockfile
pnpm dev
```

See the [development guide](docs/DEVELOPMENT.md) for running and checking the project, or the [documentation index](docs/README.md) for product, integration, and architecture guides.

## Thanks

- [Codeg](https://github.com/xintaofei/codeg): ACP integration, Agent conversations, settings, and appearance references
- [Snow Shot](https://github.com/mg-chao/snow-shot): the screenshot stack
- [RepoChan](https://github.com/l1veIn/repochan-mono): brand and character assets
- [Kotone](https://github.com/l1veIn/kotone): the local speech stack this workbench grew from

## License

[MIT](LICENSE). Adapted code and other third-party components retain their respective licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

<p align="center">
  <img src="docs/social/rambelle-chibi-footer.webp" alt="Rambelle hands over a feedback folio" width="800" />
</p>
