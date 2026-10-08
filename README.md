# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**RambleDesk is a desktop app for reviewing your coding agent's work. Give feedback with text, voice, and annotations, then continue the task.**

The agent explains what it did and what it needs you to try or decide. You review the result and give feedback where it matters. In sessions started inside RambleDesk, submitting feedback queues the next turn in the original conversation.

[Download](https://github.com/l1veIn/rambledesk/releases/latest) · [Get started](#get-started) · [Documentation](docs/README.md)

- **Comment on the material itself.** Point to an image region, a changed line, or a table cell instead of describing its location in a long prompt.
- **Say what you think.** Write, speak, or attach evidence. Optional AI cleanup helps with wording while preserving your original feedback.
- **Keep the task moving.** Drafts retain your work, and submitted feedback keeps your comments and attachments together for the agent.

## See it in use

Screenshots show the **0.5.0-rc.1 development version**, with demo data. These new workbenches are **not included in stable 0.4.0**.

**Point out a visual problem.** Mark an area and explain what should change.

![Visual feedback with a marked image region and a comment](docs/screenshots/en/visual-feedback.webp)

**Review a code change.** Put an opinion next to the line it concerns.

![Diff review with feedback attached to a changed line](docs/screenshots/en/diff-review.webp)

**Suggest a table correction.** Record a proposed value and explain it in context.

![Table review with a suggested cell value and a comment](docs/screenshots/en/table-review.webp)

These actions record feedback and suggestions; they do not directly edit your project's files.

## Get started

Download the stable **0.4.0** release for **Windows x64** or **macOS Apple Silicon** from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases). To try the new workbenches pictured above, run the current source below.

1. **Connect an agent.** Open **Settings → Agents**. Follow the prompts to install supported components or connect an existing agent such as Claude Code, Codex CLI, or Gemini CLI. Sign-in and model access belong to the agent itself.
2. **Give it a task.** Start a session, choose an agent and project folder, and describe your goal.
3. **Try it and respond.** When the agent requests feedback, follow its review steps and submit your comments. In an in-app session, feedback returns to the original conversation; delivery status stays visible.

Text feedback needs no voice setup. For speech, download a local transcription model in **Settings → Voice** and allow microphone access. AI cleanup is optional and needs a separately configured model service.

The Windows installer is not Authenticode-signed; macOS builds are not notarized. See the [installation guide](docs/INSTALLATION.md). Before upgrading to 0.5.0, make a complete backup: 0.4.0 cannot open the upgraded database. See [upgrade and rollback guidance](docs/DATA_COMPATIBILITY.md).

Prefer your existing agent app or CLI? Use **Settings → External adapters** and follow the [integration guide](docs/COMPATIBILITY.md). Continuation depends on the adapter; Generic MCP requires returning to the agent manually.

## Run from source

Install Git, Node.js, pnpm, Rust, and your platform's Tauri dependencies, then run:

```bash
git clone https://github.com/l1veIn/rambledesk.git
cd rambledesk
pnpm install --frozen-lockfile
pnpm dev
```

See [development setup](docs/DEVELOPMENT.md), [agent setup and recovery](docs/ACP_MANAGED_SESSIONS.md), the [workbench playground](playground/workbenches/README.md), or the [documentation index](docs/README.md).

## Thanks

- [Codeg](https://github.com/xintaofei/codeg): ACP integration, agent conversations, settings, and appearance references
- [Snow Shot](https://github.com/mg-chao/snow-shot): the screenshot stack
- [RepoChan](https://github.com/l1veIn/repochan-mono): brand and character assets
- [Kotone](https://github.com/l1veIn/kotone): the local speech stack this workbench grew from

## License

[MIT](LICENSE). Adapted code and other third-party components retain their respective licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

<p align="center">
  <img src="docs/social/rambelle-chibi-footer.webp" alt="Rambelle hands over a feedback folio" width="400" />
</p>
