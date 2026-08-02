# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**面向 Coding 宿主的本地体验式反馈工作台。**

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

- SQLite 持久 Inbox，支持 waiting、in-progress、completed、cancelled 状态。
- Request Workspace：富文本编辑、截图、附件、剪贴板捕获、本地流式语音转写。
- 不可变 Feedback Package：`feedback.md`、`manifest.json` 与附件。
- 带认证的本地 loopback server，提供 `/api/feedback/request|get|wait|cancel` 与 `/mcp`。
- 通用 MCP 适配器薄层，工具为 `request_feedback`、`get_feedback`、`cancel_feedback`。
- Pi 原生 package 位于 `packages/pi-rambledesk`，使用本地 JSON API，并在 Pi tool call 内等待终态。
- 适配器设置：Generic MCP 宿主配置、Pi package 安装。
- 中文/英文界面、light/dark 外观、托盘入口以及可选系统通知。

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

在 RambleDesk 中打开 **设置 → 适配器**，可以检测支持的本机工具、一键写入 Generic MCP 配置、安装 Pi package，或复制带认证的 Streamable HTTP 配置。

只进行浏览器 UI 开发时：

```bash
pnpm dev:web
```

浏览器版本会对原生能力做降级处理，不能代替桌面实机验收。

## 本地语音模型

流式转写使用 manifest 固定的 Sherpa X-ASR 模型，定义见 [`crates/rambledesk-speech/models/sherpa-x-asr.json`](crates/rambledesk-speech/models/sherpa-x-asr.json)。模型二进制不会提交到 Git。请把校验后的模型目录放在 manifest 声明的平台应用数据目录，或在本地开发时通过绝对路径指定：

```bash
RAMBLEDESK_SHERPA_MODEL_DIR=/absolute/path/to/sherpa-x-asr pnpm dev
```

没有语音模型时，文字输入、截图、文件与剪贴板导入、编辑和反馈提交仍可使用。

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
crates/rambledesk-mcp          通用 MCP 适配器薄层
crates/rambledesk-hosts        Host profiles 与 continuation strategies
crates/rambledesk-speech       原生音频采集与本地流式转写
crates/rambledesk-cli          无界面开发入口与协议验证工具
packages/pi-rambledesk         Pi 原生适配器 package
```

`core` 持有 application contract。Storage、local server、MCP、hosts、speech、CLI 和 Tauri 都是适配层或装配层，不能持有第二套业务状态。

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

## 许可证

待定。
