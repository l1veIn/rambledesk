# RambleDesk MCP 与反馈协议

> 状态：M1 pure-text feedback loop implemented
> 版本：v1 · 2026-07-29  
> 规范词：MUST / SHOULD / MAY 分别表示必须、建议和可选。

## 1. 兼容策略

RambleDesk 使用 MCP Streamable HTTP，由桌面进程在本机提供单一 `/mcp` 端点。
规范设计面向 MCP `2026-07-28`，但首发 wire profile 必须根据目标客户端实测，
允许兼容 `2025-11-25`。

协议分成两层：

1. **应用协议**：本文定义的工具、字段、幂等性、状态和结果，是稳定合同；
2. **MCP 执行适配**：根据客户端能力使用 Tasks 扩展或普通工具结果，不改变应用语义。

M0 已用 Claude Code 和 MCP Inspector 实测，并锁定官方 `rmcp` 3.0.0；
本机 Codex CLI 因安装缺失原生二进制未能启动，详见
[COMPATIBILITY.md](COMPATIBILITY.md)。实现不得自行手写一套 MCP JSON-RPC 栈。

### 1.1 执行模式

v1 首发对所有客户端固定使用 polling：`request_feedback` 立即返回 `waiting`，
Agent 使用 `get_feedback` 查询。即使客户端声明 MCP Tasks，也不能仅凭该声明
自动改变 wire result；只有服务端在后续兼容切片中显式启用 Tasks、并对目标客户
端完成 create/get/result/cancel 回归后，才可返回持久 task handle。

长时间保持单个 HTTP/SSE 响应只可作为兼容优化，不能成为正确性的前提。

无论采用哪种模式，Feedback Request 都必须在第一次工具调用返回前持久化。

## 2. 共同约定

### 2.1 标识与时间

- `request_id`、`project_id` 使用 UUID；
- 新 ID 默认使用 UUIDv7；
- 时间使用 UTC RFC 3339；
- UUID 输入解析后统一为带连字符的小写规范形式，存储与比较均使用规范值；
- `agent`、`session_id` 用于关联和展示，不用于认证。

### 2.2 字符串限制

| 字段 | 限制 |
|------|------|
| `agent` | 1–64 个字符 |
| `session_id` | 1–256 个字符 |
| `project.name` | 1–120 个字符 |
| `what_happened` | 1–12,000 个字符 |
| 单个 action instruction | 1–2,000 个字符 |
| actions 数量 | 1–20 |
| `summary` | 1–4,000 个字符 |

服务端必须拒绝 NUL、无效 UTF-8、路径穿越和超过限制的输入，不得静默截断。

### 2.3 内容与返回

工具结果应同时提供：

- 给 Agent 阅读的简短文本；
- 稳定的结构化 JSON；
- 完成后可访问的 Feedback Package URI 和本机路径。

路径只保证对与 RambleDesk 共享文件系统的同机 Agent 可见。

## 3. `request_feedback`

创建或重新关联一次 Feedback Request。该工具是幂等的。

### 3.1 输入

```json
{
  "request_id": "optional UUID",
  "agent": "codex",
  "session_id": "agent-defined stable id",
  "project": {
    "project_id": "optional UUID",
    "name": "RambleDesk",
    "root_path": "/absolute/local/path"
  },
  "what_happened": "The new onboarding flow is implemented.",
  "actions": [
    {
      "id": "open-onboarding",
      "instruction": "Launch the app and complete onboarding with a new profile."
    }
  ],
  "context_refs": [
    {
      "label": "Build instructions",
      "uri": "file:///absolute/local/path/README.md"
    }
  ]
}
```

规则：

- `request_id` 可省略，由服务端生成；
- 调用方希望跨重试稳定关联时，应自行生成并复用 `request_id`；
- `project` 必填，且 `project_id`、`root_path` 至少提供一个；
- `project.project_id` 已知时优先使用；
- 未知 `project_id` 必须同时提供 `root_path`，否则返回 `PROJECT_NOT_FOUND`；
- 只提供 `root_path` 时，服务端按 canonical path 查找或创建 Project；
- `root_path` 若提供，必须是 RambleDesk 所在机器上的绝对路径；
- `context_refs` 只作为可见引用保存，MVP 不自动读取或执行；
- `actions[].id` 在单个请求内必须唯一，格式为 `^[a-z0-9][a-z0-9_-]{0,63}$`。
- Project identity 是服务端解析后的 `project_id`；仅提供路径时，先 canonicalize
  `root_path` 再查找或创建 Project；
- `project.name` 是展示元数据，不参与 Project identity 或请求幂等 hash；已有
  Project 不会因后续请求携带不同 name 而产生冲突。

### 3.2 幂等性

服务端按 `request_id` 执行：

- 不存在：校验输入并创建请求；
- 已存在且不可变输入一致：返回或重新关联现有请求；
- 已存在但不可变输入不同：返回 `REQUEST_CONFLICT`；
- 已完成：直接返回原始完成结果；
- 已取消：返回取消结果，不隐式重新打开。

不可变输入包括 `agent`、`session_id`、project identity、`what_happened`、
有序 `actions` 和有序 `context_refs`。

不使用 `resume: true`。复用同一 `request_id` 本身就是恢复语义。

### 3.3 普通结果

```json
{
  "request_id": "019...",
  "project_id": "019...",
  "status": "waiting",
  "execution_mode": "poll",
  "created_at": "2026-07-29T08:00:00Z",
  "updated_at": "2026-07-29T08:00:00Z",
  "poll_after_ms": 30000,
  "feedback": null
}
```

完成时：

```json
{
  "request_id": "019...",
  "project_id": "019...",
  "status": "completed",
  "execution_mode": "poll",
  "created_at": "2026-07-29T08:00:00Z",
  "updated_at": "2026-07-29T08:12:00Z",
  "feedback": {
    "package_uri": "rambledesk://feedback/019...",
    "directory_path": "/absolute/path/.rambledesk/feedback/20260729T081200Z-019...",
    "markdown_path": "/absolute/path/.rambledesk/feedback/20260729T081200Z-019.../feedback.md",
    "manifest_path": "/absolute/path/.rambledesk/feedback/20260729T081200Z-019.../manifest.json"
  }
}
```

Tasks 模式下，最终 task result 必须使用相同完成结果结构。

## 4. `get_feedback`

用于不支持 Tasks 的客户端、断线恢复和诊断。

输入：

```json
{
  "request_id": "019..."
}
```

输出与 `request_feedback` 的普通结果相同。未知 ID 返回 `REQUEST_NOT_FOUND`。

该工具不得改变请求状态。

## 5. `list_feedback_requests`

只读恢复工具。默认只列出未结束请求。

输入：

```json
{
  "project_id": "optional UUID",
  "agent": "optional agent name",
  "session_id": "optional session id",
  "status": ["waiting", "in_progress"],
  "limit": 50,
  "cursor": "optional opaque cursor"
}
```

约束：

- `limit` 默认 50，最大 100；
- 返回摘要，不返回完整 transcript 或附件内容；
- cursor 对调用方不透明；
- 结果按 `updated_at DESC, request_id DESC` 排序。

## 6. `cancel_feedback`

显式取消一个尚未完成的请求。

输入：

```json
{
  "request_id": "019...",
  "reason": "The implementation changed; this test is obsolete."
}
```

规则：

- `waiting`、`in_progress` 可转为 `cancelled`；
- `completed` 返回 `REQUEST_ALREADY_COMPLETED`；
- 对已取消请求重复调用成功返回原状态，保留第一次取消的时间和原因；
- 草稿默认保留，直到用户显式删除；
- MCP Tasks 的取消必须映射到同一领域操作。

## 7. `notify_complete`

非 holding 工具，用于通知一个 Agent Session 已完成。

输入：

```json
{
  "agent": "codex",
  "session_id": "agent-defined stable id",
  "project_id": "UUID",
  "summary": "Onboarding feedback was applied and verified.",
  "next_steps": [
    "Re-test on Windows when a build is available."
  ]
}
```

规则：

- Session 仍有未结束请求时返回 `SESSION_HAS_OPEN_REQUESTS`；
- 同一 session 的相同 summary 重试不得重复发送系统通知；
- ended session 不能创建新请求；调用方应使用新的 `session_id`。

## 8. 状态模型

### 8.1 Feedback Request

```text
waiting ───────→ in_progress ───────→ completed
   │                  │
   └──────────────→ cancelled ←──────┘
```

- `waiting`：请求已持久化，尚未由 Operator 开始；
- `in_progress`：Operator 已打开工作区，允许继续编辑；
- `completed`：Feedback Package 已原子发布，终态；
- `cancelled`：请求已取消，终态。

工作台退出、HTTP 断线、Agent 超时均不改变这个状态机。

### 8.2 Invocation Attempt

每次 MCP 调用单独记录：

```text
open → responded
open → disconnected
open → cancelled
open → failed
```

Invocation Attempt 是诊断数据，不是 Feedback Request 的事实来源。

## 9. 并发规则

- UI 使用 `feedback_requests.revision` 作为唯一 aggregate CAS token；
- Draft 保存成功后，`drafts.revision` 记录对应的 aggregate revision；
- 相同正文和旧 revision 的重放视为响应丢失后的幂等重试；
- 提交在数据库事务中冻结 `source_revision`、时间和唯一 publication 路径；
- 只有一个提交可以把 Request 从非终态推进到 `completed`；
- Feedback Package 使用同一父目录下的持久化临时路径写入，文件与目录 `fsync`
  成功后原子 rename；启动时自动对账未完成的 publication intent；
- 单条 publication 对账失败必须记录稳定错误并保持请求可见，不能阻止其他请求、
  Desktop 或 MCP 启动；
- 完成后的 package 不可修改，修改反馈必须创建新 Request。

## 10. Feedback Package

默认项目内目录：

```text
<project-root>/.rambledesk/feedback/<utc-timestamp>-<request-id>/
├── feedback.md
├── manifest.json
└── attachments/
    ├── 001-onboarding.png
    └── ...
```

项目根不可写或未提供时，使用应用数据目录，并在结果中返回实际路径。

M1 的 durable publication 由 storage 平台兼容层提供：macOS/Unix 使用目录
`fsync` + 同目录 rename + 父目录 `fsync`；Windows 使用
`MoveFileExW(MOVEFILE_WRITE_THROUGH)` 移动已逐文件 flush 的同卷暂存目录。
只有平台 barrier 成功后才允许提交事务 B 或返回 completed。

`manifest.json`：

```json
{
  "schema_version": 1,
  "request_id": "019...",
  "project_id": "019...",
  "agent": "codex",
  "session_id": "...",
  "submitted_at": "2026-07-29T08:12:00Z",
  "source_revision": 7,
  "draft_revision": 7,
  "feedback_markdown": "feedback.md",
  "feedback_sha256": "...",
  "attachments": [
    {
      "id": "019...",
      "file_name": "onboarding.png",
      "media_type": "image/png",
      "byte_size": 84231,
      "sha256": "...",
      "path": "attachments/001-onboarding.png"
    }
  ]
}
```

`feedback.md` 包含请求摘要、有序 actions、Operator 的 Markdown 正文和按用户顺序
排列的附件相对链接。`attachments` 只能写包内相对路径，不得写入草稿或发布目录的
绝对路径；发布前后必须核对字节数与 SHA-256。

## 11. 错误结构

应用错误通过稳定 code 返回：

```json
{
  "code": "REQUEST_CONFLICT",
  "message": "request_id already exists with different immutable input",
  "retryable": false
}
```

能够可靠关联时，服务端 MAY 额外返回 `request_id`；调用方不得依赖错误结果一定
含有该字段。

首版必须覆盖：

- `INVALID_ARGUMENT`
- `UNAUTHORIZED`
- `PROJECT_PATH_UNAVAILABLE`
- `PROJECT_NOT_FOUND`
- `REQUEST_NOT_FOUND`
- `REQUEST_CONFLICT`
- `REQUEST_ALREADY_COMPLETED`
- `REQUEST_CANCELLED`
- `SESSION_HAS_OPEN_REQUESTS`
- `STORAGE_FAILURE`
- `PACKAGE_PUBLISH_FAILURE`
- `INTERNAL_ERROR`

内部错误不得向 MCP 返回令牌、完整文件内容、数据库路径或 Rust backtrace。

## 12. 安全基线

- 仅绑定 `127.0.0.1`；IPv6 支持须显式加入 `::1`；
- 每次请求验证 `Host`；
- 存在 `Origin` 时必须验证 allowlist，拒绝未知 Origin；
- MCP 端点要求安装时生成的至少 256-bit 随机 bearer token；
- token 存在操作系统安全存储或权限受限文件中，不写入日志；
- UI 显示当前端口、认证状态和复制配置入口；
- `root_path` 必须 canonicalize，并阻止通过 symlink/`..` 越界写入；
- 日志默认记录元数据，不记录完整 ramble、截图内容或 token。

## 13. 版本演进

- 工具 schema 从 v1 开始；
- 新增可选字段是兼容变更；
- 删除、改名、改变状态含义属于破坏性变更；
- Feedback Package 用独立 `schema_version`；
- 数据库迁移与工具协议版本不得共用一个版本号。

## 14. 规范参考

- [MCP 2026-07-28 Specification](https://modelcontextprotocol.io/specification/2026-07-28)
- [MCP Tasks Extension](https://modelcontextprotocol.io/extensions/tasks/overview)
- [MCP 2025-11-25 Streamable HTTP](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)

Tasks 是双方显式声明支持的可选扩展；RambleDesk 不得向未声明支持的客户端返回
task result。
