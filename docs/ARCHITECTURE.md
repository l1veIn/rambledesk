# RambleDesk 架构基线

> 状态：Development baseline
> 版本：v1 · 2026-07-29
> 详细工具合同见 [PROTOCOL.md](PROTOCOL.md)，工程计划见
> [DEVELOPMENT.md](DEVELOPMENT.md)。

## 1. 架构目标

架构首先保证：

1. 请求、草稿和完成结果不依赖某条 MCP 连接存活；
2. 同一业务逻辑可由 Tauri UI、MCP server 和 CLI 调用；
3. 桌面框架、传输协议、SQLite 和语音引擎可以分别替换；
4. M0–M2 不为尚未验证的语音重依赖买单；
5. 同机 local-first 安全边界清晰。

## 2. 运行时拓扑

```text
┌──────────────────┐        Streamable HTTP        ┌──────────────────────────┐
│ Codex / Claude   │ ────────────────────────────→ │ RambleDesk desktop       │
│ / MCP Inspector  │        127.0.0.1:<port>/mcp   │                          │
└──────────────────┘                               │  MCP adapter             │
                                                   │       │                  │
┌──────────────────┐        Tauri commands/events  │  Application services   │
│ Svelte UI        │ ←───────────────────────────→ │       │                  │
└──────────────────┘                               │  Domain + ports          │
                                                   │    │       │       │     │
                                                   │ SQLite  files  notify    │
                                                   └──────────────────────────┘
```

开发和自动化环境可用 `rambledesk-cli` 替代桌面壳装配同一个 MCP adapter、
application services 和 storage。CLI 不是第二套业务实现。

## 3. Monorepo 结构

```text
rambledesk/
├── apps/
│   └── desktop/
│       ├── src/                  # Svelte 5 UI
│       └── src-tauri/            # Tauri 薄壳 / composition root
├── crates/
│   ├── rambledesk-core/
│   ├── rambledesk-storage/
│   ├── rambledesk-mcp/
│   ├── rambledesk-speech/
│   └── rambledesk-cli/
├── docs/
├── scripts/
└── tests/
```

选择原因、依赖方向和拆分判据见
[ADR 001](adr/001-apps-crates-monorepo.md)。

## 4. 组件职责

### 4.1 `rambledesk-core`

包含：

- Project、AgentSession、FeedbackRequest、Draft、Attachment、FeedbackResult；
- Request 与 Session 状态机；
- create/get/list/cancel/submit/notify use cases；
- Repository、Transaction、PackagePublisher、Notifier、Clock、IdGenerator ports；
- 稳定错误码和领域事件。

禁止依赖：

- Tauri；
- MCP SDK；
- SQLx/SQLite；
- HTTP server；
- cpal/sherpa-onnx；
- 操作系统全局目录。

### 4.2 `rambledesk-storage`

包含：

- SQLite schema 与 migrations；
- repository/transaction 实现；
- Draft 与附件暂存；
- Feedback Package publisher；
- 应用数据目录和项目内 `.rambledesk` 路径策略。

它只实现 core ports，不向上泄漏 SQL row、连接池或文件系统细节。

### 4.3 `rambledesk-mcp`

包含：

- MCP tool schema；
- Streamable HTTP transport；
- bearer token、Host 和 Origin 校验；
- Tasks 与 polling 执行适配；
- MCP error 与领域错误映射；
- invocation attempt 诊断。

它不直接写数据库。断线只结束 Invocation Attempt，不改变 Feedback Request。

### 4.4 `rambledesk-speech`

M3 引入：

- 音频设备枚举和 cpal 采集；
- PCM 标准化、RMS 和长录音分段；
- STT engine registry；
- 本地模型管理；
- 转写事件和失败恢复。

speech 输出 Draft patch 或 transcript segments，由 application service 决定如何
合并；speech 自身不能提交 Feedback Request。

### 4.5 `rambledesk-cli`

提供：

- headless MCP host；
- schema/数据库/Feedback Package 诊断；
- 创建 fixture request；
- 导入 WAV 并运行 speech 回归；
- CI smoke test。

### 4.6 Desktop

Tauri Rust 壳只负责：

- 进程与窗口生命周期；
- tray、系统通知、文件选择和权限提示；
- 装配具体 ports；
- 暴露 Tauri commands；
- 把领域事件桥接为前端事件。

Svelte UI 负责投影和用户输入，不持有唯一事实状态。

## 5. 事实来源

| 数据 | 唯一事实来源 |
|------|--------------|
| Request/Session 状态 | SQLite |
| Draft 正文 | SQLite |
| Draft 附件 bytes | 应用 draft 目录，SQLite 存 metadata |
| 完成反馈 | 不可变 Feedback Package |
| UI 当前页面/展开项 | 前端内存 |
| MCP 连接 | transport 内存 + invocation attempt 日志 |
| 系统通知 | best-effort side effect |
| partial transcript | speech session 内存；定期 checkpoint 到 Draft |

Tauri events、MCP progress 和系统通知都只是提示，不是事实来源。

## 6. 核心流程

### 6.1 创建请求

```text
MCP request_feedback
  → validate/authenticate
  → core CreateFeedbackRequest
  → transaction:
      resolve/create Project
      resolve/create AgentSession
      idempotency check by request_id + input_hash
      insert Request + Actions
      append domain event
  → commit
  → notify UI / system
  → return task handle or waiting result
```

只有事务提交成功后才能通知 UI 或向 Agent 返回已创建。

### 6.2 编辑草稿

```text
UI command
  → Load Request + revision
  → validate non-terminal state
  → persist Draft revision
  → emit RequestChanged
  → UI re-query
```

附件先写到 request 专属 staging 目录，再写 metadata。删除 attachment 必须同时
处理文件和记录；失败时保留可诊断的 orphan 标记，不假装成功。

### 6.3 提交

```text
SubmitFeedback(request_id, expected_revision)
  → transaction A: acquire submit lease / verify state
  → render package into sibling temp directory
  → flush + hash + fsync
  → atomic rename to final directory
  → transaction B:
      mark completed
      store immutable result paths + hashes
      release task/poll waiters
  → notify Agent-facing execution adapter
```

若 package 已发布但 transaction B 失败，启动恢复任务根据 manifest/request_id
完成数据库对账；不得创建第二份 package。

### 6.4 Agent 取得结果

- Tasks 客户端取得 task final result；
- polling 客户端调用 `get_feedback`；
- 相同 `request_id` 再次调用 `request_feedback` 可取得同一结果；
- 三条路径读取相同 application query。

## 7. 状态与生命周期

Feedback Request：

```text
waiting → in_progress → completed
   │           │
   └───────────┴──────→ cancelled
```

`completed`、`cancelled` 为终态。

工作台关闭时：

- 停止接受新 MCP 调用；
- 完成正在提交的短事务或回滚；
- 未结束 Request 保持原业务状态；
- Draft 已按 revision 持久化；
- 开放 Invocation Attempt 记为 disconnected。

工作台启动时：

1. 打开数据库；
2. 执行 migrations；
3. 对账 package temp/final 目录；
4. 恢复未结束 Request；
5. 启动 MCP；
6. 启动 UI/tray；
7. 对 waiting 请求补发本地通知（受去重策略限制）。

## 8. 数据与文件布局

应用数据：

```text
<app-data>/rambledesk/
├── rambledesk.sqlite3
├── auth/
├── drafts/<request-id>/
├── models/                       # M3
├── logs/
└── recovery/
```

项目内最终产物：

```text
<project-root>/.rambledesk/
└── feedback/<timestamp>-<request-id>/
    ├── feedback.md
    ├── manifest.json
    └── attachments/
```

若项目路径不存在、不可写或不在同一文件系统，使用 app-data 下的 final package
目录，并把实际路径返回 Agent。

路径必须 canonicalize。写入项目目录前必须确认最终目标仍位于选定
`.rambledesk/feedback` 根下。

## 9. 并发与一致性

- SQLite 使用 WAL；
- 所有写操作通过 application transaction；
- Request 使用递增 `revision` 做乐观并发；
- 同一 request 只有一个 submit lease；
- tool idempotency 使用 canonical input hash；
- package final path 由 request_id 唯一确定；
- 系统通知和 UI events 在事务提交后发送；
- 失败的 side effect 进入可重试 outbox，不回滚已经完成的领域事实。

M1 可以使用数据库 outbox 表；不使用只存在内存中的“稍后再通知”队列。

## 10. 安全边界

MVP 假设 Agent 与 RambleDesk 在同一台可信电脑，但仍防止浏览器和其他本地进程
无意调用：

- 只监听 loopback；
- 随机 bearer token；
- Host/Origin 验证；
- 限制请求体大小和字段长度；
- root path canonicalization；
- attachment MIME sniff + 文件大小/数量限制；
- 日志不记录 token、完整反馈正文或图片；
- UI 对即将分享的反馈提供最终预览；
- MCP 的 `agent/session_id` 仅作标签。

## 11. 可观测性

结构化事件至少包含：

- `event_name`
- `timestamp`
- `request_id`（如适用）
- `invocation_attempt_id`（如适用）
- `duration_ms`
- `outcome`
- `error_code`

默认不包含：

- transcript 或 feedback 正文；
- action 原文；
- attachment 内容/完整文件名；
- bearer token；
- 任意环境变量。

## 12. 测试边界

### Core

- 状态机属性测试；
- 幂等性与并发提交；
- session 结束规则；
- 错误码。

### Storage

- migration；
- SQLite transaction；
- package 原子发布；
- crash recovery fixture；
- path traversal/symlink。

### MCP

- schema golden；
- Tasks/polling 等价；
- auth/Host/Origin；
- cancellation/disconnection；
- MCP Inspector smoke。

### Desktop

- command/event 映射；
- tray/notification；
- Playwright 关键路径。

### Speech

- WAV fixture；
- bounded channel/backpressure；
- 10 分钟录音资源上限；
- 模型缺失、设备拔出和取消。

## 13. 架构门禁

CI 至少运行：

```text
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
pnpm check
pnpm test
pnpm build:web
protocol schema drift check
```

带 sherpa-onnx 等重 feature 的测试单独运行，不拖慢默认 core 循环。

## 14. 已知待验证项

这些是 M0 技术验证，不是开放产品问题：

- 目标 Codex/Claude Code 的 MCP 协议版本；
- Tasks 扩展支持；
- 自定义 bearer header 配置；
- 普通工具调用的超时和取消行为；
- 官方 Rust SDK 对当前规范的覆盖；
- Rust → TypeScript DTO 生成工具。

验证结果写入 `docs/COMPATIBILITY.md` 并锁定依赖版本。
