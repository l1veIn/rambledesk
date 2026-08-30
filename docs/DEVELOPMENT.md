# RambleDesk 开发基线

> 状态：v2 历史开发基线，已冻结，不用于 v3 新代码。
> v3 当前 Module 与 Interface 见 [ARCHITECTURE.md](ARCHITECTURE.md)，实施顺序见 [V3_IMPLEMENTATION_PLAN.md](V3_IMPLEMENTATION_PLAN.md)。

## 技术栈

| 领域 | 决定 |
| --- | --- |
| 桌面框架 | Tauri 2 |
| 前端 | Svelte 5 + TypeScript + Vite |
| UI 系统 | shadcn-svelte + Tailwind CSS |
| 核心逻辑 | Rust application contract |
| 异步运行时 | Tokio |
| 通用适配器传输 | 官方 Rust SDK `rmcp`，Streamable HTTP |
| 原生适配器传输 | loopback Local JSON API |
| 持久化 | SQLite + 显式 migrations |
| 序列化/schema | Serde + Schemars |
| 日志 | tracing；默认只记录元数据 |
| ID | UUIDv7 |
| 时间 | UTC，边界使用 RFC 3339 |
| 包管理 | pnpm + Cargo |

Svelte 和 Tauri 是实现选择，不是应用协议。Rust application contract、SQLite
事实状态和反馈包格式不能依赖前端组件或桌面窗口生命周期。

## 目录与职责

```text
rambledesk/
├── apps/
│   └── desktop/
│       ├── src/                         # Workbench UI
│       └── src-tauri/                   # Tauri composition root
├── crates/
│   ├── rambledesk-core/                 # application contract
│   ├── rambledesk-storage/              # SQLite + package publication
│   ├── rambledesk-local-server/         # listener + auth + routes
│   ├── rambledesk-mcp/                  # Generic MCP Adapter
│   ├── rambledesk-hosts/                # Host Profiles + continuation
│   ├── rambledesk-speech/               # local speech capability
│   └── rambledesk-cli/                  # headless composition root
├── packages/
│   ├── pi-rambledesk/                    # Pi Native Adapter
│   └── dsh-rambledesk/                   # DeepSeek Harness (dsh) Native Adapter
├── docs/
└── scripts/
```

### `rambledesk-core`

- 定义 request/get/wait/cancel/list、draft、attachment、submit use cases；
- 定义状态机、稳定 DTO、错误码和 ports；
- 不依赖 HTTP、JSON、MCP、Pi、Tauri、SQLite 或宿主安装逻辑；
- 不读取环境变量，不推导源码目录。

### `rambledesk-storage`

- 执行 SQLite migrations；
- 实现 request、draft、attachment metadata 和宿主会话关联；
- 发布、校验并恢复不可变反馈包；
- 不认识 transport、适配器安装或 UI。

### `rambledesk-local-server`

- 只绑定 loopback listener；
- 管理 bearer token、Host/Origin guard 和 route mounting；
- 暴露 `/api/feedback/request|get|wait|cancel`；
- 将 Generic MCP Adapter 挂载在 `/mcp`；
- 不实现领域规则或宿主专用行为。

### `rambledesk-mcp`

- 定义通用 MCP tools、instructions、handlers；
- 将 MCP 输入映射为 core application calls；
- 将 core 结果与错误映射为 MCP structured content；
- 执行宿主检测与配置写入（per-host 知识来自 `rambledesk-hosts` 注册表）；
- 不持有 listener、token、JSON API、SQLite，也不向全局界面投影 transport 可用性。

### `rambledesk-hosts`

- 持有宿主知识注册表（executable/marker/配置路径/`ConfigFormat`）、Host Profile catalog、标签、图标和适配器提示；
- 持有 continuation payload、strategy contract 和手动恢复提示；
- 不实现 MCP、Pi、storage、desktop UI，也不持有适配器安装/写入执行逻辑。

### `packages/pi-rambledesk`

- 注册 Pi 原生反馈工具；
- 调用 Local JSON API 的 request/get/wait/cancel；
- 在同一个 Pi tool call 内等待终态；
- 不依赖 MCP。

### `packages/dsh-rambledesk`

- 注册 dsh（DeepSeek Harness）原生反馈工具（request/resume/get/cancel）；
- 调用 Local JSON API 的 request/wait/get/recover/cancel；
- 在同一个 dsh 工具调用内等待终态（不声明 `timeoutMs`，只在执行信号中断时中止）；
- 在插件旁持久化 request 状态与 `host_session_id`，支持跨重启恢复；
- 由桌面安装引擎把 `rambledesk-hosts` 中的通用 `ramble` skill 写入 `~/.agents/skills`；该 skill 会自动选择 dsh 原生等待流程；
- 不依赖 MCP，零 npm 依赖。

### Desktop

- Tauri 负责进程、窗口、tray、通知、权限、文件选择和 crate 装配；
- Svelte 负责 Inbox、Request Workspace、Resume Prompt、Settings / Adapters；
- 前端通过 command 查询事实状态，通过 event 获知可能发生了变化；
- UI store、窗口状态和通知都不是唯一事实来源。

## 依赖方向

```text
rambledesk-mcp ───────────┐
rambledesk-local-server ──┼──→ rambledesk-core
rambledesk-storage ───────┤
rambledesk-hosts ─────────┘

rambledesk-cli ─────────────→ core + storage + local-server
rambledesk-desktop ─────────→ core + storage + local-server + hosts + speech
```

`rambledesk-local-server` 可以装配 `rambledesk-mcp`，但
`rambledesk-mcp` 不得反向依赖本地服务。跨适配器编排只在 composition root
发生。

## 前后端合同

Rust DTO 是事实来源。修改导出类型后必须运行：

```bash
cargo run -p rambledesk-core --example export_types
pnpm contracts:check
```

生成文件位于 `apps/desktop/src/lib/generated/`。前端不得手写第二套 request
状态枚举或字段别名。

## 本地运行

安装依赖：

```bash
pnpm install
```

启动浏览器工作台：

```bash
pnpm dev:web
```

启动桌面应用：

```bash
pnpm dev
```

隔离本地状态：

```bash
RAMBLEDESK_DATABASE_FILE=/absolute/test/feedback.sqlite3 \
RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE=/absolute/test/local-server.token \
RAMBLEDESK_LOCAL_SERVER_PORT=0 \
pnpm dev
```

本地服务默认只绑定 loopback。通知、麦克风和屏幕录制权限必须由明确的人类操作触发，
自动化测试不得主动弹出系统权限框。

## SQLite 基线

### `host_sessions`

- `id`
- `host_id`
- `host_session_id`
- `created_at`
- `updated_at`
- 唯一键：`host_id, host_session_id`

### `feedback_requests`

- `id`
- `host_session_record_id`
- `title`
- `what_happened`
- `source_hint`
- `status`
- `revision`
- `input_hash`
- lifecycle timestamps
- cancellation metadata

### 请求内容

- `request_actions`
- `request_context_refs`
- `drafts`
- `attachments`

### 交付与恢复

- `feedback_results`
- `submission_plans`

数据库约束阻止非法终态回退；application transaction 负责跨表不变量。

## 进程与恢复

桌面启动顺序：

1. 打开数据库并执行 migrations；
2. 对账未完成的反馈包发布；
3. 构建 application services；
4. 启动本地服务；
5. 初始化窗口、tray 和通知桥接；
6. UI 查询 Inbox 和当前工作区。

非正常退出后，SQLite 与 draft 目录负责恢复。`waiting` 和 `in_progress`
请求不能因为进程重启而隐式取消。Tauri events 只提示 UI 重新查询。

## 发布与更新说明

- 发布前在 `docs/CHANGELOG.md` **顶部**为本次版本新增条目（`## vX.Y.Z`），
  纯文本、英文在前中文摘要在后（更新弹窗以 `<pre>` 渲染，不解析 Markdown）。
- `release.yml` 的 checksums 阶段会自动把该条目写入 GitHub Release 正文和
  `latest.json` 的 `notes`；没有条目的版本回退到通用说明（会打警告）。
- 手动生成说明：`node scripts/release-notes.mjs --tag vX.Y.Z`。
- 修正已发布 release 的说明：先改 CHANGELOG，再手动刷 release 正文和
  `latest.json`（`scripts/release-notes.mjs` + `scripts/patch-updater-notes.mjs`），
  避免两处不一致。

## 完成标准

一个改动只有满足以下条件才算完成：

- 实现、协议、术语和 UI 文案一致；
- 没有字段 alias、fallback 或 deprecated route；
- 正常、重试、取消、断线和重启路径按风险有测试；
- 不记录正文、token 或附件内容到默认日志；
- Rust 格式化、clippy、测试通过；
- TypeScript/Svelte 检查、测试、构建和合同漂移检查通过；
- 术语残留扫描通过；
- 涉及原生窗口、截图、语音、tray 或权限时列出人工验收结果。

建议的完整门禁：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
pnpm check
pnpm test
pnpm build:web
pnpm contracts:check
pnpm test:pi
pnpm test:dsh
pnpm mcp:self-test
pnpm mcp:inspector-smoke
```
