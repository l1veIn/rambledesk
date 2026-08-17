# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

![RambleDesk](docs/social/ramble-banner-text2-1400x700.webp)

**把人类封装成 API，提供给 Coding Agent。** 收集你的胡言乱语，整理成提示词。

宿主发出结构化请求；你真实使用目标产品，边用边说，按需截图，然后提交一份带图的反馈包。宿主读取反馈包后继续开发，而不是从零散聊天里拼上下文。

## 为什么需要 RambleDesk？

现代 Coding 工具实现很快，但人类反馈通常仍散落在聊天里：一句临时提问、一段模糊回复、一张很晚才补上的截图，或者一次需要手动恢复上下文的续跑。

RambleDesk 把这次交接变成正式闭环：

1. 宿主创建反馈请求，附上背景和明确的操作清单。
2. RambleDesk 通知人类，并在本地持久保存请求。
3. 人类真实使用目标产品，在一次 Ramble 中记录语音、截图、文件和显式导入的剪贴板内容。
4. RambleDesk 发布不可变的 Markdown 反馈包及其附件。
5. 宿主通过 `get_feedback` 读取反馈包并继续。

请求不会因为 transport 断开或应用重启而丢失。反馈生命周期是持久的，单次 HTTP 连接只是一次交付尝试。

## 当前能力

**Ramble 内的工具** —— 你捕获的一切都会实时落入反馈包对应位置：

- **截图**：框选区域后可标注（画笔、箭头、文字），图片自动插入光标处。
- **剪贴板导入**：一键把复制的文字或图片带标签和时间戳写入文档。
- **文件导入**：通过选择器或拖放附加图片或任意文件，每个文件不超过 20 MiB。
- **图片粘贴**：直接粘贴剪贴板截图到文档。
- **语音记录 + 本地转写**：使用产品的同时开口说话，语音在本机流式转写（X-ASR、SenseVoice 或 FunASR-Nano）并写入正文。
- **附件管理**：在光标处插入、排序或移除附件，预览实时更新。

Ramble 之外：

- SQLite 持久 Inbox，支持 waiting、in-progress、completed、cancelled 状态。
- 不可变 Feedback Package：`feedback.md`、`manifest.json` 与附件。
- 带认证的本地 loopback server，提供 `/api/feedback/request|get|wait|cancel` 与 `/mcp`。
- 通用 MCP 适配器方案：工具为 `request_feedback`、`get_feedback`、`cancel_feedback`，并带消费宿主知识注册表的检测/安装引擎。
- Pi 原生 package 位于 `packages/pi-rambledesk`，使用本地 JSON API，并在 Pi tool call 内等待终态。
- DeepSeek Harness (dsh) 原生插件位于 `packages/dsh-rambledesk`，使用本地 JSON API，并在 dsh 工具调用内等待终态。
- 适配器设置：Generic MCP 宿主配置、Pi package 安装、DSH 插件安装。
- 首次使用引导：语言、数据位置、本地语音、适配器、通知和可选 Cooking；可从 **设置 → 通用** 再次启用。
- 中文/英文界面、light/dark 外观、托盘入口以及可选系统通知。

## 下载与安装

只从官方 [GitHub Releases](https://github.com/l1veIn/rambledesk/releases) 页面下载安装包。在绕过任何系统安全提示之前，请确认文件确实来自本仓库，并把下载文件的 SHA-256 与同一 Release 中的 `SHA256SUMS.txt` 对比。

### macOS（Apple Silicon）

当前 macOS 构建使用 ad-hoc 签名，但**没有经过 Apple 公证**。因此，即使 DMG 文件本身完好，Gatekeeper 也可能阻止首次启动。

1. 从同一个 Release 下载文件名包含 `aarch64.dmg` 的安装包和 `SHA256SUMS.txt`，然后在“终端”中校验：

   ```bash
   cd ~/Downloads
   grep 'aarch64.dmg' SHA256SUMS.txt | shasum -a 256 -c -
   ```

   只有结果显示 `OK` 时才继续。

2. 打开 DMG，把 **RambleDesk** 拖到安装窗口里的 **Applications（应用程序）** 文件夹，然后推出 RambleDesk 磁盘映像。
3. 在 Finder 中打开**应用程序**，按住 Control 点击或右键点击 **RambleDesk**，选择**打开**；如果系统再次询问，继续选择**打开**。
4. 如果仍被阻止，先尝试启动一次，再前往**系统设置 → 隐私与安全性**，在“安全性”区域点击**仍要打开**并完成身份验证。这个按钮通常会在启动被拦截后保留约一小时。可参阅 [Apple 官方说明](https://support.apple.com/guide/mac-help/mh40616/mac)。
5. 如果提示“RambleDesk 已损坏，无法打开”，请先确认 SHA-256 与 Release 一致，再在“终端”中只移除下载文件的 quarantine 属性，然后重新打开：

   ```bash
   xattr -dr com.apple.quarantine /Applications/RambleDesk.app
   ```

   通常不需要 `sudo`。如果系统提示应用“将损坏你的电脑”、检测到恶意软件，或者校验值不一致，请**不要**绕过警告。
6. 首次引导中，按需允许**屏幕与系统音频录制**和**麦克风**权限。如果屏幕权限授权后提示需要重启，请重启 RambleDesk 再进行截图。

卸载时先退出 RambleDesk，再把 `/Applications/RambleDesk.app` 移到废纸篓。反馈资料库和设置会被刻意保留，避免重装时丢失已有内容。

### Windows

从同一 Release 下载 `.exe` 安装器并运行。在接入 Windows Authenticode 签名前，SmartScreen 可能显示“Windows 已保护你的电脑”。确认下载来源和 SHA-256 后，选择**更多信息 → 仍要运行**。Windows 安装器及应用内更新与未公证的 macOS 分发链路彼此独立。

## 本地开发快速开始

环境要求：

- Rust 1.91.1，由 `rust-toolchain.toml` 固定
- Node.js 22.23.0
- pnpm 10.12.4
- Tauri 2 对当前平台要求的系统依赖

安装依赖并启动原生桌面应用：

```bash
pnpm install --frozen-lockfile
pnpm dev
```

在 RambleDesk 中打开 **设置 → 适配器**，可以检测支持的本机工具、一键写入 Generic MCP 配置、安装 Pi package、安装 DeepSeek Harness 插件，或复制带认证的 Streamable HTTP 配置。

## 首次使用引导

全新安装会在进入工作台前展示一段简短引导。数据位置是第一步：反馈附件、已发布包和语音模型都会写入该目录。选择其他位置后，RambleDesk 会先保存选择并重启，之后才下载模型或产生反馈。接下来可以下载本地语音模型、安装推荐的 Pi 原生适配器（同一 tool call 内自动继续）、按需配置 Generic MCP 宿主（手动继续）、开启通知以及配置可选 Feedback Cooking。随时可在 **设置 → 通用 → 再次启用新手引导** 重走。

只进行浏览器 UI 开发时：

```bash
pnpm dev:web
```

浏览器版本会对原生能力做降级处理，不能代替桌面实机验收。

## 本地语音模型

RambleDesk 支持 X-ASR 流式转写，以及由 VAD 自动分段的 SenseVoice、FunASR-Nano 非流式转写。打开 **设置 → 语音** 即可选择、下载、切换或删除模型，并调整 Silero VAD 声音阈值。模型清单位于 [`crates/rambledesk-speech/models`](crates/rambledesk-speech/models)，下载后的权重保存在所选数据存储位置，不会提交到 Git。

没有语音模型时，文字输入、截图、文件与剪贴板导入、编辑和反馈提交仍可使用。

## Feedback Cooking

可选的 Cooking 位于 **设置 → 通用**。RambleDesk 通过 Vercel AI SDK 调用 DeepSeek、OpenAI 或用户填写的 OpenAI-compatible 服务，在提交前把 uncooked Ramble 原稿整理成正式 Markdown。新反馈包始终把人类原稿保存为 `uncooked.md`，宿主默认读取的正式结果为 `feedback.md`；API Key 只留在当前设备设置中，不会写入反馈包。

## 自动化验证

运行与 CI 一致的核心门禁：

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

构建原生应用：

```bash
pnpm build
```

## 架构

```text
apps/desktop                   Tauri 2 + Svelte 5 工作台与装配根
crates/rambledesk-core         Application contract、状态机、ports、use cases
crates/rambledesk-storage      SQLite、草稿、附件、反馈包发布
crates/rambledesk-local-server Loopback listener、auth、JSON API、route mounting
crates/rambledesk-mcp          通用 MCP 适配器方案（工具面 + 宿主安装引擎）
crates/rambledesk-hosts        宿主知识注册表、profiles 与 continuation strategies
crates/rambledesk-speech       原生音频采集与本地流式转写
crates/rambledesk-cli          无界面开发入口与协议验证工具
packages/pi-rambledesk         Pi 原生适配器 package
packages/dsh-rambledesk        DeepSeek Harness (dsh) 原生适配器插件
```

`core` 持有 application contract。Storage、local server、host knowledge、speech、CLI 和 Tauri 都是基础设施层或装配层；Generic MCP、Pi 与 dsh 插件是完整的宿主适配方案。任何一层都不能成为第二套业务状态。

## 文档

| 文档 | 内容 |
| --- | --- |
| [术语表](docs/TERMINOLOGY.md) | 产品术语、协议字段和 package 边界的唯一来源。 |
| [产品宪章](docs/CONSTITUTION.md) | North Star 与不可妥协原则。 |
| [产品文档](docs/PRODUCT.md) | 范围、主流程、信息架构与恢复模型。 |
| [架构基线](docs/ARCHITECTURE.md) | 运行时拓扑、crate 边界与一致性规则。 |
| [协议](docs/PROTOCOL.md) | 工具 schema、本地 JSON API、幂等性、生命周期、错误与安全。 |
| [开发基线](docs/DEVELOPMENT.md) | 技术栈、数据模型与验收门。 |
| [适配器验证](docs/COMPATIBILITY.md) | 已测试宿主路径、协议版本、认证与执行模式。 |
| [Dogfooding 记录](docs/DOGFOODING.md) | 真实使用轮次、发现、修复与验证证据。 |
| [Kotone 复用审计](docs/KOTONE_REUSE.md) | 可复用语音组件、必要修改与许可证门禁。 |
| [macOS 分发](docs/MACOS_DISTRIBUTION.md) | 先发未公证 Apple Silicon DMG；Developer ID 以后再补。 |

## 许可证

待定。
