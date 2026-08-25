# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

![RambleDesk](docs/social/ramble-banner-en-1400x700.webp)

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk product demo" width="960" />

</div>

## Stop vibe coding. Rambling is all you need.

Sometimes writing prompts is hard. All I can put out is rambling. So I built a workbench that takes rambling.

RambleDesk already fits most common harnesses. Turn on ramble mode, give the AI a goal. When it needs you, RambleDesk will call. Use the tools that are already there: speech, screenshots, pasted code, files. After you finish the ramble, the AI continues.

RambleDesk is like sunglasses. You'll know when to put them on.

## Install and use

Download from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases).

**Windows:** run `x64-setup.exe`. Until Authenticode is added, SmartScreen may block the first launch. Confirm the download, then **More info → Run anyway**.

**macOS (Apple Silicon):** open the DMG and drag RambleDesk into Applications. The build is ad-hoc signed and not notarized. First launch: right-click → Open. If macOS blocks it as unsafe or from an unidentified developer, open **System Settings → Privacy & Security**, scroll to the **Security** section, find RambleDesk, click **Open Anyway**, then confirm **Open**. If macOS says the app is damaged, confirm the file came from this repo, then:

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

Open the app and finish first-run setup. In **Settings → Adapters**, install the host you use: Claude Code, Cursor, Codex, Gemini CLI, Grok, OpenCode, Reasonix, Antigravity IDE, plus native adapters for Pi and DeepSeek Harness.

Then enable ramble mode in the host and give the AI a goal. It will knock when it needs you.

From source:

```bash
pnpm install --frozen-lockfile
pnpm dev
```

## Thanks

- [Snow Shot](https://github.com/mg-chao/snow-shot), for the screenshot stack
- [RepoChan](https://github.com/l1veIn/repochan-mono), for brand and character assets
- [Kotone](https://github.com/l1veIn), for the local speech stack this workbench grew from

## License

[MIT](LICENSE)
