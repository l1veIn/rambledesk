# RambleDesk 架构基线

> 状态：v2 当前基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文若与术语表冲突，以术语表为准。

本文描述 RambleDesk 当前应遵循的结构边界。

## 运行时拓扑

```text
┌────────────────────┐     MCP transport      ┌──────────────────────────────┐
│ Generic MCP hosts  │ ─────────────────────→ │ RambleDesk local server      │
│ Claude/Codex/...   │     /mcp               │                              │
└────────────────────┘                         │  auth / listener / guards    │
                                               │  route mounting              │
┌────────────────────┐     Local JSON API      │                              │
│ Pi package         │ ─────────────────────→ │  /api/feedback/*             │
│ packages/pi-*      │     /api               │                              │
└────────────────────┘                         └──────────────┬───────────────┘
                                                              │
┌────────────────────┐     Tauri commands/events              │
│ Workbench UI       │ ←──────────────────────────────────────┘
└────────────────────┘
                                                              │
                                                              ▼
                                      ┌──────────────────────────────┐
                                      │ rambledesk-core              │
                                      │ application contracts         │
                                      └──────────────┬───────────────┘
                                                     │
                                                     ▼
                                      ┌──────────────────────────────┐
                                      │ rambledesk-storage           │
                                      │ SQLite + feedback packages   │
                                      └──────────────────────────────┘
```

`apps/desktop` 是装配根：它装配 storage、core application、本地服务、host knowledge、desktop-only capabilities。CLI 和测试可以复用同一套 crate，但不能成为第二套业务实现。

## Feedback Draft 所有权

工作台最多只有一个可编辑 `RichFeedbackEditor`。当前 request 通过 Editor transaction 写入；后台 Active Ramble 通过 TipTap JSON transformation 写入。数据库以版本化 `document_json` 为真源，保存时同时生成 `body_markdown`。详见 [ADR 004](adr/004-single-editor-structured-draft.md)。

禁止 per-request Editor、hidden Editor、session 持有 editor handle，以及自动 Tidy。

## Package 边界

```text
rambledesk/
├── apps/
│   └── desktop/                  # Workbench UI + Tauri composition root
├── crates/
│   ├── rambledesk-core/          # application contract
│   ├── rambledesk-storage/       # SQLite + feedback package publication
│   ├── rambledesk-local-server/  # loopback HTTP server + JSON API
│   ├── rambledesk-mcp/           # Generic MCP Adapter (tool surface + installer engine)
│   ├── rambledesk-hosts/         # Host knowledge registry + profiles + continuation strategy
│   ├── rambledesk-speech/
│   └── rambledesk-cli/
├── packages/
│   └── pi-rambledesk/            # Pi Native Adapter
└── docs/
```

### `rambledesk-core`

持有：

- 反馈请求 use cases：request/get/wait/cancel/list；
- 反馈草稿、附件和提交 use cases；
- 反馈包输出合同；
- 稳定 DTO、错误码、状态机；
- Repository、PackagePublisher、Clock、IdGenerator 等 ports。

不得持有：

- HTTP、JSON、MCP、Pi package、Tauri command；
- 本地服务 listener、token path、Host/Origin guard；
- 宿主安装逻辑、host profile、continuation strategy；
- 源码 checkout 模型或路径依赖。

### `rambledesk-storage`

持有：

- SQLite schema 与 migrations；
- core repository ports 的实现；
- request、draft、attachment metadata 持久化；
- 跨请求宿主会话关联；
- 不可变反馈包发布和恢复对账。

不得持有：

- 宿主协议；
- 适配器安装；
- 源码 checkout runtime 语义；
- UI 或 transport 细节。

### `rambledesk-local-server`

持有：

- loopback HTTP listener；
- bearer token 生成、读取、默认路径；
- Host/Origin guard；
- `/api/feedback/request|get|wait|cancel`；
- `/mcp` route mounting；
- server handle、endpoint、port configuration。

不得持有：

- 领域规则；
- MCP tool schema；
- Pi package 代码；
- desktop UI 状态。

### `rambledesk-mcp`

持有 Generic MCP Adapter 完整方案（与 `packages/pi-rambledesk` 对等）：

- MCP tool schema、tool handler、instructions、structured result / error 格式化；
- MCP request 到 `rambledesk-core` application call 的映射；
- 客户端检测/安装执行引擎：按 `rambledesk-hosts` 声明的 `ConfigFormat` 分发，把
  RambleDesk 服务器条目写入各宿主的配置文件（JSON/TOML），含幂等与修复。

不得持有：

- HTTP listener、token path、local JSON API routes；
- host-specific continuation 实现；
- per-host 知识（可执行文件名、配置路径、配置格式声明）——这些属于 `rambledesk-hosts`。

### `rambledesk-hosts`

持有：

- 宿主知识注册表（单一真相源）：每宿主的 executable、marker 目录、配置文件路径、
  配置格式（`ConfigFormat`），以及默认适配器选择；
- Host Profile catalog（从知识注册表派生）；
- host label/icon；
- continuation 模式声明；
- 手动 continuation 提示 payload；
- 未来原生 continuation strategy 接口。

不得持有：

- MCP implementation、Pi package implementation；
- 适配器安装/写入执行逻辑（检测执行与格式引擎属于 `rambledesk-mcp`）；
- storage 或 desktop UI 状态。

### `packages/pi-rambledesk`

持有 Pi 原生适配器：

- Pi tools：`request_ramble_feedback`、`get_ramble_feedback`；
- request/get/wait/cancel 到本地 JSON API 的调用；
- Pi tool call 内等待终态；
- Pi package 安装说明和测试。

不得持有：

- MCP client 行为；
- desktop UI 状态；
- RambleDesk storage 逻辑。

### Desktop

Tauri 壳负责：

- 进程、窗口、tray、系统通知、文件选择、权限提示；
- 装配 storage、core、本地服务、hosts；
- 暴露 Tauri commands；
- 桥接领域结果为前端事件。

Svelte UI 负责：

- 工作台投影；
- 人类反馈输入；
- 草稿、截图、录音、附件交互；
- 设置中的适配器安装 UX。

UI 不持有唯一事实状态。Tauri events 和系统通知都是提示，不是事实来源。

## 事实来源

| 数据 | 唯一事实来源 |
| --- | --- |
| 反馈请求状态 | SQLite |
| 草稿正文 | SQLite |
| 草稿附件 bytes | 应用 draft 目录，SQLite 存 metadata |
| 反馈包 | 不可变 package directory + manifest |
| 宿主身份 | adapter-provided `host_id`，服务端可按安装入口覆盖 |
| 宿主会话关联 | adapter-provided `host_session_id` |
| 上下文提示 | request `context_refs` / `source_hint` |
| UI 当前页面/展开项 | 前端内存 |
| 系统通知 | best-effort side effect |
| 局部转写 | speech session 内存；定期 checkpoint 到 Draft |

## 核心流程

### 通用 MCP 适配器

```text
MCP request_feedback
  → rambledesk-mcp 映射工具输入
  → rambledesk-core request_feedback
  → rambledesk-storage 持久化请求
  → 本地服务返回 waiting 结果
  → 宿主智能体结束当前 turn
  → 人类在工作台提交/取消
  → 手动 continuation 提示
  → 宿主智能体调用 get_feedback(request_id)
```

通用 MCP 适配器不提供自动恢复原宿主上下文的产品保证。

### Pi 原生适配器

```text
Pi request_ramble_feedback
  → /api/feedback/request
  → Workbench receives persisted request
  → /api/feedback/wait blocks inside Pi tool call
  → Human completes/cancels in Workbench
  → wait returns terminal Feedback Package
  → Pi continues original task
```

Pi 不需要提交后的 continuation。

### 人类提交

```text
SubmitFeedback(request_id, expected_revision)
  → verify non-terminal state
  → render package into temp directory
  → flush + hash
  → atomic publish
  → mark request completed
  → notify waiters / prepare continuation prompt
```

如果 package 已发布但数据库更新失败，启动恢复任务根据 manifest/request_id 对账；不得创建第二份 package。

## 状态模型

```text
waiting → in_progress → completed
   │           │
   └───────────┴──────→ cancelled
```

`completed` 和 `cancelled` 是终态。只有终态触发 continuation。

## 数据布局

应用数据：

```text
<local-data>/RambleDesk/
├── feedback.sqlite3
├── auth/
├── drafts/<request-id>/
├── feedback/<timestamp>-<request-id>/
├── models/
├── logs/
└── recovery/
```

反馈包默认写入应用数据目录。适配器若提供安全可写的路径 hint，未来可以作为导出或镜像目标，但核心协议不得要求源码 checkout 路径。

## 安全边界

- 本地服务只监听 loopback。
- 所有 `/api` 和 `/mcp` 请求必须通过 bearer token。
- Host header 必须是 loopback host。
- Origin 只允许受信任 desktop/webview origin 或空 origin 的本地工具调用。
- `host_id`、`host_session_id` 不是认证凭据。
- 返回路径只保证同机、共享文件系统可见。
