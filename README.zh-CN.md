# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

## 给 Agent 一个目标，想到哪说到哪

RambleDesk 将 Coding Agent 对话和人的反馈放进同一个桌面工作区。通过 ACP 连接本机 Agent，选择项目，给它一个目标，就可以开始工作。

当 Agent 需要你的判断时，它会发起 Ramble 请求。你可以直接说话、截图、粘贴内容或添加文件——不用组织成一段完美的 prompt，想到哪说到哪。检查并提交反馈，Agent 就能在原会话中继续工作。你也可以通过外部适配器，沿用自己熟悉的 Agent 应用或 CLI。

> 本文介绍 **0.4 候选版**：在 Ramble 反馈工作区的基础上，增加应用内 ACP 智能体对话。功能变化与已知限制见[发布说明](docs/CHANGELOG.md)。

### 它是怎么工作的？

1. **给 Agent 一个目标**
   在 RambleDesk 中选择 Agent 和项目目录，开始对话。在 Agent 页面查看回复和工具执行过程。

2. **Agent 请求反馈**
   遇到需要人来判断、解释或实际体验的问题时，Agent 会在 Ramble 页面创建请求。

3. **你自由表达**
   用语音讲、截张图、粘贴代码，或者直接拖入文件。哪种方式最快，就用哪种。

4. **Agent 继续工作**
   将反馈连同上下文一起提交。RambleDesk 保存反馈并排队续接原 Agent 会话，展示送达状态，必要时提供恢复操作。

<div align="center">

<img src="https://github.com/l1veIn/rambledesk/releases/download/v0.3.2/rambledesk-demo-10s.gif" alt="RambleDesk 产品演示" width="960" />

<p><em>早期版本中的 Ramble 反馈流程：通过语音和屏幕内容作出回应，再将反馈交还给 Agent。</em></p>

</div>

### 什么时候适合用？

- 你知道想要什么，但一时很难写成准确的 prompt
- Agent 需要产品判断、视觉反馈或操作确认
- 一张截图、一段口述或一个文件，比文字解释更直接
- 你希望 Agent 获得反馈后继续工作，而不是停在那里等你整理上下文

## 在同一个工作区里对话、体验和反馈

- **Agent 对话：**新建或恢复会话，查看历史。工具调用以简洁的一行呈现，需要时再展开命令详情和输出。
- **Agent 提供的选项：**选择实际连接提供的模型、模式和思考选项，在对话中处理 Agent 转发的授权请求及受支持的用户问题。
- **Ramble 反馈：**围绕请求收集语音、截图、剪贴板内容和文件。反馈保存后展示送达状态，续接中断时提供恢复入口。
- **语音写入前确认：**在悬浮窗中确定、取消、整理或编辑识别结果，再写入目标位置。可选自动整理，并提示已有的 Tidy 自动整理阈值配置。
- **适合自己的工作区：**拖动或收起会话列与请求列，记住调整后的宽度；选择明暗模式、主题配色、界面缩放、字体和工作区背景。
- **可控的诊断记录：**清除已有诊断信息，或关闭记录；待处理请求的自动预览默认关闭。

## 快速开始

从 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) 下载并安装 RambleDesk，然后按照下方对应平台的说明完成安装。

<details open>
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

### 开始第一个会话

1. 打开「**设置 → Agents**」，检测已安装的 Agent、安装受支持的连接组件，或添加自定义 ACP 入口。
2. 在 Agent 自己的工具中完成登录和 API Key 配置，再回到 RambleDesk 检查连接。配置问题阻塞连接时，RambleDesk 会给出提示，引导你到 Agent 中处理。
3. 点击「**新建会话**」，选择 Agent 和**项目目录**，输入任务。发送前必须选择项目目录；可用模型和模式会从实际连接中加载。
4. 收到 Ramble 请求后，打开请求、记录并提交反馈。通过「**查看 Agent**」回到对话，继续跟进执行结果。

## Agent 接入方式

推荐使用 **ACP 托管会话**。RambleDesk 管理对话，并将你的反馈续接到同一个 Agent 会话。关闭对话标签页会保留会话运行；停止 Agent 是独立操作。

内置目录提供 Claude Code、Codex CLI、Gemini CLI、Grok、OpenCode、Cursor、Pi、DeepSeek ACP、DeepSeek Harness 等入口。目录中有入口或连接检测成功，并不代表各项能力完全一致：模型选择、权限模式、问答和会话恢复取决于 Agent 及其 ACP 实现。社区 DeepSeek ACP 与官方 DeepSeek Harness 是能力不同的两个独立入口。

配置和恢复方式见[托管会话使用指南](docs/ACP_MANAGED_SESSIONS.md)；各 Agent 的版本、能力差异与验证范围见[能力调研矩阵](docs/research/2026-09-07-acp-agent-capabilities.md)。

**外部适配器**是沿用现有 Agent 应用或 CLI 的轻量接入方式。Agent 自行管理会话和对话，RambleDesk 负责接收反馈请求并返回你的回复。在「设置 → 外部适配器」中完成接入。反馈适配器目前支持 Claude Code、Cursor、Codex、Gemini CLI、Grok、OpenCode、Reasonix、Antigravity IDE，以及 Pi 和 DeepSeek Harness 原生适配器。

## 从源码运行

```bash
pnpm install --frozen-lockfile
pnpm dev
```

需要 Node.js、pnpm、Rust 和对应平台的 Tauri 构建依赖。项目结构与检查方式见[开发指南](docs/DEVELOPMENT.md)。

## 不要再 vibe coding 了，你只需要 Ramble。

有时候，难的不是解决问题，而是把脑子里的想法整理成 prompt。

RambleDesk 不要求你先想清楚再开口。你只管表达，它负责整理。

## 致谢

- [Codeg](https://github.com/xintaofei/codeg)，ACP 接入、Agent 对话、设置与外观设计参考
- [Snow Shot](https://github.com/mg-chao/snow-shot)，截图能力
- [RepoChan](https://github.com/l1veIn/repochan-mono)，品牌与角色资产
- [Kotone](https://github.com/l1veIn)，本地语音转写的实现基础

## 许可证

[MIT](LICENSE)

改写代码及其他第三方组件保留各自许可证，详见[第三方声明](THIRD_PARTY_NOTICES.md)。

![RambleDesk](docs/social/ramble-banner-text2-1400x700.webp)
