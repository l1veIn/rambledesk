# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

## Give the agent a goal. Ramble your way through the rest.

RambleDesk brings coding-agent conversations and human feedback into one desktop workspace. Connect a local agent through ACP, choose a project, and give it a goal.

When the agent needs your judgment, it opens a Ramble request. Speak, take a screenshot, paste context, or attach a file—whatever gets the idea out fastest. Review your feedback, send it back, and continue in the same agent session. You can also keep using your usual agent app or CLI through an external adapter.

> This README describes the **0.4 release candidate**, which adds in-app ACP conversations alongside the Ramble feedback workspace. See the [release notes](docs/CHANGELOG.md) for changes and known limitations.

### How it works

1. **Give the agent a goal**
   Start a conversation in RambleDesk with an agent and a project folder. Follow its responses and tool activity in the Agent view.

2. **The agent asks for feedback**
   When it needs your judgment, explanation, or a hands-on check, it opens a request in the Ramble view.

3. **You respond naturally**
   Talk it through, capture the screen, paste code, or attach a file. Use whichever medium makes the point best.

4. **The agent continues**
   Submit the feedback with its context attached. RambleDesk saves it and queues continuation in the original agent session, with delivery status and recovery actions when needed.

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk product demo" width="960" />

<p><em>The Ramble feedback loop in an earlier release: answer with your voice and on-screen context, then send the feedback back.</em></p>

</div>

### When it helps

- You know what you want, but turning it into a precise prompt is slow
- The agent needs product judgment, visual feedback, or confirmation
- A screenshot, spoken explanation, or file is clearer than another paragraph
- You want the agent to continue as soon as it gets your feedback

## One workspace for conversation and feedback

- **Agent conversations:** start and resume sessions, read history, and expand compact tool-call rows when you need command details or output.
- **Agent-provided controls:** choose the models, modes, and reasoning options exposed by the connected agent. Handle forwarded permission requests and supported questions directly in the conversation.
- **Ramble feedback:** collect voice, screenshots, clipboard content, and files around a request. Saved feedback has a visible delivery state, including recovery after an interrupted continuation.
- **Speech review:** confirm, cancel, Tidy, or edit recognized speech in the floating window before writing it. Optional automatic Tidy works alongside the existing Tidy threshold settings.
- **A workspace that fits:** resize or collapse the session and request columns, with widths remembered. Choose light or dark mode, theme colors, interface scaling, fonts, and workspace backgrounds.
- **Diagnostics you control:** clear recorded diagnostics or turn recording off. Automatic preview of waiting requests is off by default.

## Quick start

Download and install RambleDesk from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases), then follow the instructions for your platform below.

<details open>
<summary><strong>Windows and macOS installation notes</strong></summary>

### Windows

Run `x64-setup.exe`. Until Authenticode is added, SmartScreen may block the first launch. Confirm the download came from this repository, then select **More info → Run anyway**.

### macOS (Apple Silicon)

Open the DMG and drag RambleDesk into Applications. The build is ad-hoc signed and not notarized. On first launch, right-click the app and select **Open**.

If macOS still blocks it, open **System Settings → Privacy & Security**, scroll to **Security**, find RambleDesk, click **Open Anyway**, then confirm **Open**.

If macOS says the app is damaged, first confirm the file came from this repository, then run:

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

</details>

### Start your first session

1. Open **Settings → Agents**. Detect an installed agent, install a supported connection component, or add a custom ACP entry.
2. Complete sign-in and API-key setup in the agent's own tools, then check its connection in RambleDesk. If setup blocks the connection, RambleDesk shows guidance for returning to the agent and fixing it.
3. Choose **New session**, select an agent and a **project folder**, and enter your task. A project folder is required before sending; available model and mode choices load from the connection.
4. When a Ramble request arrives, open it, record your feedback, and submit. Use **View Agent** to return to the conversation and follow the next step.

## Agent connections

**ACP managed sessions are the recommended path.** RambleDesk manages the conversation and feeds your replies back into the same session. Closing a conversation tab keeps the session running; stopping the agent is a separate action.

The built-in catalog includes entries for Claude Code, Codex CLI, Gemini CLI, Grok, OpenCode, Cursor, Pi, DeepSeek ACP, DeepSeek Harness, and more. A catalog entry or successful connection check does not imply identical capabilities: model selection, permission modes, questions, and session recovery depend on the agent and its ACP implementation. DeepSeek ACP and official DeepSeek Harness are separate entries with different capabilities.

See the [managed session guide](docs/ACP_MANAGED_SESSIONS.md) for setup and recovery, and the [agent capability matrix](docs/research/2026-09-07-acp-agent-capabilities.md) for versioned research and verification limits.

**External adapters** offer a lightweight way to keep working in your usual agent app or CLI. Your agent manages its own sessions and conversations; RambleDesk receives feedback requests and returns your replies. Set them up under **Settings → External adapters**. Feedback adapters support Claude Code, Cursor, Codex, Gemini CLI, Grok, OpenCode, Reasonix, Antigravity IDE, plus native adapters for Pi and DeepSeek Harness.

## From source

```bash
pnpm install --frozen-lockfile
pnpm dev
```

Requires Node.js, pnpm, Rust, and the platform's Tauri build dependencies. See the [development guide](docs/DEVELOPMENT.md) for the project structure and checks.

## Stop vibe coding. Rambling is all you need.

Sometimes the hard part is not solving the problem. It is turning what is in your head into a prompt.

RambleDesk does not ask you to organize the thought before you say it. You ramble; it makes the feedback useful.

## Thanks

- [Codeg](https://github.com/xintaofei/codeg), for ACP integration, Agent conversations, settings, and appearance references
- [Snow Shot](https://github.com/mg-chao/snow-shot), for the screenshot stack
- [RepoChan](https://github.com/l1veIn/repochan-mono), for brand and character assets
- [Kotone](https://github.com/l1veIn), for the local speech stack this workbench grew from

## License

[MIT](LICENSE)

Adapted code and other third-party components retain their respective licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

![RambleDesk](docs/social/ramble-banner-en-1400x700.webp)
