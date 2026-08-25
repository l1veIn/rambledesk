# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

## 给 Agent 一个目标，想到哪说到哪

现在的 coding harness，大多在给 Agent 配工具。RambleDesk 反过来，给「人」配工具。

当 Agent 需要你的判断时，它会把问题送到桌面。你可以直接说话、截图、粘贴内容或拖入文件——不用组织成一段完美的 prompt，想到哪说到哪。RambleDesk 会自动整理成结构化反馈，再交还给 Agent，让它继续工作。

### 它是怎么工作的？

1. **Agent 发起请求**
   遇到需要人来判断、解释或确认的问题时，Agent 会唤起 RambleDesk。

2. **你自由表达**
   用语音讲、截张图、粘贴代码，或者直接拖入文件。哪种方式最快，就用哪种。

3. **Agent 继续工作**
   RambleDesk 将这些内容整理成清晰的反馈，连同上下文一起返回给 Agent。

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk 产品演示" width="960" />

<p><em>Agent 请求补充信息；你通过语音和屏幕内容作出回应，RambleDesk 会将整理后的反馈发送回去。</em></p>

</div>

### 什么时候适合用？

- 你知道想要什么，但一时很难写成准确的 prompt
- Agent 需要产品判断、视觉反馈或操作确认
- 一张截图、一段口述或一个文件，比文字解释更直接
- 你希望 Agent 获得反馈后继续工作，而不是停在那里等你整理上下文

## 快速开始

1. 从 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) 下载并安装 RambleDesk
2. 打开 **设置 → 适配器**，安装你正在使用的 Agent 适配器
3. 在对应的 coding harness 中启用 ramble mode，然后给 Agent 一个目标
4. 当它需要你时，RambleDesk 会来敲门

目前支持 Claude Code、Cursor、Codex、Gemini CLI、Grok、OpenCode、Reasonix、Antigravity IDE，以及 Pi 和 DeepSeek Harness。

<details>
<summary><strong>Windows 和 macOS 安装说明</strong></summary>

### Windows

运行 `x64-setup.exe`。未加入 Authenticode 签名时，SmartScreen 可能会拦截首次启动。确认安装包来自本仓库，然后选择 **更多信息 → 仍要运行**。

### macOS（Apple Silicon）

打开 DMG，将 RambleDesk 拖入“应用程序”。当前版本采用 ad-hoc 签名且尚未公证。首次启动时，请右键点击应用并选择“打开”。

如果系统仍然阻止启动，请前往 **系统设置 → 隐私与安全性**，在“安全性”区域找到 RambleDesk，点击 **仍要打开**，然后确认“打开”。

如果 macOS 提示应用已损坏，请先确认文件来自本仓库，然后运行：

```bash
xattr -dr com.apple.quarantine /Applications/RambleDesk.app
```

</details>

### 从源码运行

```bash
pnpm install --frozen-lockfile
pnpm dev
```

## 不要再 vibe coding 了，你只需要 Ramble。

有时候，难的不是解决问题，而是把脑子里的想法整理成 prompt。

RambleDesk 不要求你先想清楚再开口。你只管表达，它负责整理。

## 致谢

- [Snow Shot](https://github.com/mg-chao/snow-shot)，截图能力
- [RepoChan](https://github.com/l1veIn/repochan-mono)，品牌与角色资产
- [Kotone](https://github.com/l1veIn)，本地语音转写的实现基础

## 许可证

[MIT](LICENSE)

![RambleDesk](docs/social/ramble-banner-text2-1400x700.webp)
