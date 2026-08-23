# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

![RambleDesk](docs/social/ramble-banner-text2-1400x700.webp)

[![RambleDesk 演示：Drive Development with Ramble](docs/social/rambledesk-demo-poster.jpg)](docs/social/rambledesk-demo.mp4)

[观看 28 秒演示](docs/social/rambledesk-demo.mp4)

## 不要再 vibe coding 了，你只需要 Ramble。

有时候我觉得写提示词很难。我只能输出胡言乱语。所以我写了个专门接收胡言乱语的工作台。

RambleDesk 已经能适配大部分常见 harness。开启 ramble 模式，然后给 AI 一个目标。它会在需要你的时候通过 RambleDesk 通知你。你可以用各种现成的 ramble 工具：语音转录、截图、粘贴代码、上传文件。完成 ramble 之后，AI 会自动继续。

RambleDesk 就像太阳眼镜。你会知道什么时候该使用它。

## 安装和使用

从 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) 下载。

**Windows：** 运行 `x64-setup.exe`。未做 Authenticode 签名时，SmartScreen 可能拦一下，确认来源后选「更多信息 → 仍要运行」。

**macOS（Apple Silicon）：** 打开 DMG，把 RambleDesk 拖进 Applications。当前是 ad-hoc 签名、未公证。第一次请右键 → 打开；如果提示「已损坏」，先确认下载来自本仓库，再执行：

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

打开软件，走完首次引导。在 **设置 → 适配器** 里装上你用的宿主：Claude Code、Cursor、Codex、Gemini CLI、Grok、OpenCode、Reasonix、Antigravity IDE，以及 Pi / DeepSeek Harness 原生适配器。

然后在宿主里开启 ramble 模式，给 AI 一个目标。它需要你的时候会来敲门。

从源码跑：

```bash
pnpm install --frozen-lockfile
pnpm dev
```

## 致谢

- [Snow Shot](https://github.com/mg-chao/snow-shot)，截图能力
- [RepoChan](https://github.com/l1veIn/repochan-mono)，品牌与角色资产
- [Kotone](https://github.com/l1veIn)，本地语音转写的实现基础

## 许可证

[MIT](LICENSE)
