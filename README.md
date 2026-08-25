# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

## Give the agent a goal. Ramble your way through the rest.

Most coding harnesses focus on giving tools to the agent. RambleDesk flips that around: it gives tools to the human.

When an agent needs your judgment, it sends the question to your desktop. Speak, take a screenshot, paste context, or drop in a file—whatever gets the idea out fastest. RambleDesk turns it into structured feedback, sends it back, and lets the agent keep working.

### How it works

1. **The agent asks**
   When a task needs human judgment, explanation, or confirmation, the agent opens a request in RambleDesk.

2. **You respond naturally**
   Talk it through, capture the screen, paste code, or attach a file. Use whichever medium makes the point best.

3. **The agent continues**
   RambleDesk organizes your input into clear feedback and returns it to the agent, with the context attached.

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk product demo" width="960" />

<p><em>The agent asks for clarification; you answer with your voice and on-screen context, and RambleDesk sends the organized feedback back.</em></p>

</div>

### When it helps

- You know what you want, but turning it into a precise prompt is slow
- The agent needs product judgment, visual feedback, or confirmation
- A screenshot, spoken explanation, or file is clearer than another paragraph
- You want the agent to continue as soon as it gets your feedback

## Quick start

1. Download RambleDesk from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases)
2. Open **Settings → Adapters** and install the adapter for your coding agent
3. Enable ramble mode in the host, then give the agent a goal
4. When it needs you, RambleDesk will knock

RambleDesk supports Claude Code, Cursor, Codex, Gemini CLI, Grok, OpenCode, Reasonix, Antigravity IDE, plus native adapters for Pi and DeepSeek Harness.

<details>
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

### From source

```bash
pnpm install --frozen-lockfile
pnpm dev
```

## Stop vibe coding. Rambling is all you need.

Sometimes the hard part is not solving the problem. It is turning what is in your head into a prompt.

RambleDesk does not ask you to organize the thought before you say it. You ramble; it makes the feedback useful.

## Thanks

- [Snow Shot](https://github.com/mg-chao/snow-shot), for the screenshot stack
- [RepoChan](https://github.com/l1veIn/repochan-mono), for brand and character assets
- [Kotone](https://github.com/l1veIn), for the local speech stack this workbench grew from

## License

[MIT](LICENSE)

![RambleDesk](docs/social/ramble-banner-en-1400x700.webp)
