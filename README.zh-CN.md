# RambleDesk

[English](README.md) | [简体中文](README.zh-CN.md)

**面向 Coding Agent 的本地体验式反馈工作台。**

Agent 下发结构化请求；你真实使用产品，边用边说，按需截图，然后提交一份带图的
反馈包。Agent 在同一个任务中拿到结果，继续开发。

> Agent 呼叫你做真实使用反馈；你负责 ramble，RambleDesk 把它变成 Agent 可以
> 继续执行的持久产物。

## 为什么需要 RambleDesk？

Agent 已经很擅长实现功能，但人类反馈通常仍散落在聊天里：一句临时提问、一段模糊
回复、一张很晚才补上的截图，或者一次需要手动恢复上下文的续跑。

RambleDesk 把这次交接变成正式闭环：

1. Agent 创建反馈请求，附上背景和明确的操作清单。
2. RambleDesk 通知人类，并在本地持久保存请求。
3. 人类真实使用目标产品，在一次 Ramble 中记录语音、截图、文件和显式导入的剪贴板内容。
4. RambleDesk 发布不可变的 Markdown 反馈包及其附件。
5. 会话被恢复后，Agent 调用 `get_feedback` 取得完整反馈包，不再用空轮询浪费 Token。

请求不会因为 MCP 断线或应用重启而丢失。反馈生命周期是持久的，单次 HTTP 连接只是
交付方式。

## 用 RambleDesk 开发 RambleDesk

RambleDesk 从现在开始会把自身作为日常 dogfooding 对象。

完成一轮有意义的 UI 或交互改动后，Coding Agent 可以启动应用并调用
`request_feedback`。开发者在真实桌面应用里进行验收，边使用边 ramble，按需补充
截图并提交；同一个 Agent 任务随后会获得 manifest、Markdown 和附件路径，直接继续
这一轮迭代。

因此，这个仓库既是产品实现，也是对产品核心承诺的持续验证：人类判断应当增强
Agent 循环，而不是打断它的上下文。

## 当前能力

- SQLite 持久反馈 Inbox，支持 waiting、in-progress、completed、cancelled 状态。
- 统一 Ramble Session：本地流式语音转写、区域截图、文件导入、显式剪贴板导入，
  以及暂停、继续和退出。
- 位于屏幕右侧中部、不遮挡工作的紧凑纵向悬浮操作台；仅显示图标，可拖动。
- Tiptap 富文本反馈编辑器，图片直接进入文档流。
- 不可变 Feedback Package：`feedback.md`、`manifest.json` 与附件。
- 带认证的本地 loopback MCP 服务，并支持为本机 Agent 工具自动写入配置。
- 持久的 request/get/cancel，完成态 `get_feedback` 返回完整反馈包，以及带
  `RAMBLEDESK_HOST` 的 MCP 宿主自动注册。
- 中文/英文界面、light/dark 外观以及可选系统通知。

RambleDesk 仍在积极开发中。macOS 原生主流程已经完成端到端 dogfooding；Linux 和
Windows 持续由 CI 覆盖，各平台特有的实机验收会独立记录。

## MCP 工具

| 工具 | 用途 |
| --- | --- |
| `request_feedback` | 创建或重新关联持久反馈请求；创建后结束当前 turn。 |
| `get_feedback` | 读取状态；completed 时返回完整反馈包。 |
| `cancel_feedback` | 显式取消尚未结束的请求。 |

正常路径是 `request_feedback` 后结束 Agent turn（不要轮询）。人类提交并恢复会话后
再调用 `get_feedback`。自动注册会写入 `RAMBLEDESK_HOST` / `X-RambleDesk-Host` 以
标识宿主。

## 本地开发快速开始

### 环境要求

- Rust 1.91.1（由 `rust-toolchain.toml` 固定）
- Node.js 22.23.0
- pnpm 10.12.4
- Tauri 2 对当前平台要求的系统依赖

安装依赖并启动原生桌面应用：

```bash
pnpm install --frozen-lockfile
pnpm dev
```

在 RambleDesk 中打开 **设置 → MCP 接入**，可以检测支持的本机工具、一键写入配置，
或复制带认证的 Streamable HTTP 配置。修改配置后需要重启对应的 Agent 工具。

只进行浏览器 UI 开发时：

```bash
pnpm dev:web
```

浏览器版本会对原生能力做降级处理，不能代替桌面实机验收。

### 本地语音模型

流式转写使用 manifest 固定的 Sherpa X-ASR 模型，定义见
[`crates/rambledesk-speech/models/sherpa-x-asr.json`](crates/rambledesk-speech/models/sherpa-x-asr.json)。
模型二进制不会提交到 Git。请把校验后的模型目录放在 manifest 声明的平台应用数据
目录，或在本地开发时通过绝对路径指定：

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
```

构建原生应用：

```bash
pnpm build
```

## 架构

RambleDesk 是 pnpm + Cargo monorepo。Tauri/Svelte 桌面壳保持轻量，产品领域能力由
Rust 服务承载：

```text
apps/desktop                 Tauri 2 + Svelte 5 桌面应用
crates/rambledesk-core       领域模型、状态机、ports 与 use cases
crates/rambledesk-storage    SQLite、草稿、附件与 Feedback Package 发布
crates/rambledesk-mcp        带认证的 Streamable HTTP MCP 适配器
crates/rambledesk-speech     原生音频采集与本地流式转写
crates/rambledesk-cli        无界面开发入口与协议验证工具
```

`core` 是产品语义的唯一事实来源。Storage、MCP、speech、CLI 和 Tauri 都是适配层，
任何适配层都不能持有第二套业务状态。

## 文档

| 文档 | 内容 |
| --- | --- |
| [产品宪章](docs/CONSTITUTION.md) | North Star、不可妥协原则与 User_0 边界。 |
| [产品文档](docs/PRODUCT.md) | 问题、范围、主流程、信息架构与恢复模型。 |
| [架构基线](docs/ARCHITECTURE.md) | Monorepo 边界、运行时装配与一致性规则。 |
| [MCP 与反馈协议](docs/PROTOCOL.md) | 工具 schema、幂等性、生命周期、错误与安全。 |
| [开发基线](docs/DEVELOPMENT.md) | 技术栈、数据模型、里程碑与验收门。 |
| [兼容矩阵](docs/COMPATIBILITY.md) | 已测试 MCP 客户端、协议版本、认证与执行模式。 |
| [Dogfooding 记录](docs/DOGFOODING.md) | 真实使用轮次、发现、修复与验证证据。 |
| [Kotone 复用审计](docs/KOTONE_REUSE.md) | 可复用语音组件、必要修改与许可证门禁。 |
| [设计访谈](docs/INTERVIEW.md) | 历史决策上下文，不作为当前规范。 |

## 许可证

待定。
