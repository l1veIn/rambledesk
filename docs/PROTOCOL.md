# RambleDesk 反馈协议

> 状态：v2 当前基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文若与术语表冲突，以术语表为准。

本文定义 RambleDesk 的应用协议。规范词：MUST / SHOULD / MAY 分别表示必须、建议和可选。

## 协议边界

RambleDesk 提供两个本机 loopback 入口：

- `/mcp`：通用 MCP 适配器 transport；
- `/api`：本地 JSON API，供 Pi 等原生适配器调用。

`/mcp` 和 `/api` 都挂载在 `rambledesk-local-server` 上。`rambledesk-core` 只持有 application contract，不持有 JSON、HTTP 或 MCP 细节。

协议分三层：

| 层 | 职责 |
| --- | --- |
| 应用合同 | 请求、状态、幂等性、反馈包、错误码。 |
| 本地 JSON API | HTTP JSON 传输层，供原生适配器使用。 |
| 通用 MCP 适配器 | MCP tool surface，供通用宿主使用。 |

## 身份字段

| 字段 | 类型 | 必需 | 说明 |
| --- | --- | --- | --- |
| `request_id` | UUID string | 可选 | 幂等 key。省略时服务端生成。 |
| `host_id` | string | 可选 | 宿主家族 id，例如 `pi`、`claude`、`codex`、`opencode`、`generic`。自动注册客户端（`RAMBLEDESK_HOST` / `X-RambleDesk-Host`）由服务端注入；未提供时服务端默认 `generic`。 |
| `host_session_id` | string | 必需 | 宿主会话关联。同一宿主会话可发起多次 request。 |
| `title` | string | 可选 | 请求在人类工作台中的短标题。 |
| `what_happened` | string | 必需 | 宿主智能体对当前变化或需要检查事项的说明。 |
| `actions` | array | 必需 | 人类应执行的操作清单。 |
| `context_refs` | array | 可选 | 文件、URL、diff、截图等可读上下文引用。 |
| `source_hint` | string | 可选 | 来源提示，可包含路径或标题；不是身份字段。 |

规则：

- `request_id` 是唯一持久反馈 lookup key。
- `host_id` 用于 host profile 匹配、展示和 continuation strategy 选择；省略时默认 `generic`，或被可信适配器头覆盖。
- `host_session_id` 只用于关联同一宿主会话的多次 request；它不是认证凭据，也不证明可自动继续。
- RambleDesk MUST NOT 要求源码 checkout 路径。
- 路径如果出现，只能出现在 `context_refs` 或 `source_hint` 中。

## 输入结构

### `FeedbackRequestInput`

```json
{
  "request_id": "optional UUID",
  "host_id": "pi",
  "host_session_id": "pi-session-019...",
  "title": "Settings adapter review",
  "what_happened": "The adapter settings panel was changed.",
  "actions": [
    {
      "id": "open-settings",
      "instruction": "Open settings and inspect the adapter tab."
    }
  ],
  "context_refs": [
    {
      "label": "Terminology",
      "uri": "file:///absolute/path/docs/TERMINOLOGY.md"
    }
  ],
  "source_hint": "RambleDesk desktop checkout"
}
```

规则：

- `request_id`、`title`、`source_hint`、`context_refs`、`attachments`、`allow_finish`、`final_summary` 均可选。
- `host_id` 可选：自动注册客户端（`RAMBLEDESK_HOST` / `X-RambleDesk-Host`）由服务端注入；未提供时服务端默认 `generic`。
- `allow_finish`：**只有**当请求只需要人类做简单批准/拒绝、不需要反馈正文时才设 `true`（例如最终交付确认）；此时 `final_summary`（确切的结束语草稿）MUST 同时提供。需要人类审阅、提意见、逐段反馈的请求（校对、检查、提问等）MUST 省略 `allow_finish`，让人类提交详细反馈而非"直接完成"捷径。
- 未设置 `allow_finish` 时若提供了 `final_summary`，返回 invalid argument（`final_summary` 依赖 `allow_finish`）。

### `ActionInput`

```json
{
  "id": "open-settings",
  "instruction": "Open settings and inspect the adapter tab."
}
```

规则：

- `actions` MUST contain 1-20 items.
- `actions[].id` MUST match `^[a-z0-9][a-z0-9_-]{0,63}$`.
- `actions[].id` MUST be unique within one request.

### `ContextRef`

```json
{
  "label": "Build instructions",
  "uri": "file:///absolute/path/README.md"
}
```

规则：

- `context_refs` are saved as readable hints.
- RambleDesk MUST NOT execute or automatically trust referenced content.
- Local paths in `context_refs` are optional hints, not required identity.

## 状态模型

```text
waiting → in_progress → completed
   │           │
   └───────────┴──────→ cancelled
```

`completed` 和 `cancelled` 是终态。只有终态触发 continuation。

## `request_feedback`

创建或重新关联一个反馈请求。该操作 MUST 是幂等的。

### 输入

MCP tool input 与 `FeedbackRequestInput` 等价。通用 MCP 适配器 MAY 根据安装入口或 `X-RambleDesk-Host` 覆盖调用方传入的 `host_id`。

### 幂等性

服务端按 `request_id` 执行：

- 不存在：校验输入并创建请求；
- 已存在且不可变输入一致：返回现有请求；
- 已存在但不可变输入不同：返回 `REQUEST_CONFLICT`；
- 已完成：直接返回原完成结果；
- 已取消：返回取消结果，不隐式重新打开。

不可变输入包括：

- `host_id`
- `host_session_id`
- `title`
- `what_happened`
- ordered `actions`
- ordered `context_refs`
- ordered `attachments`（`file_name`、`markdown` 或 `contents_base64`）
- `source_hint`

### 结果

```json
{
  "request_id": "019...",
  "host_id": "pi",
  "host_session_id": "pi-session-019...",
  "status": "waiting",
  "execution_mode": "poll",
  "created_at": "2026-08-02T08:00:00Z",
  "updated_at": "2026-08-02T08:00:00Z",
  "feedback": null
}
```

完成结果：

```json
{
  "request_id": "019...",
  "host_id": "pi",
  "host_session_id": "pi-session-019...",
  "status": "completed",
  "execution_mode": "poll",
  "created_at": "2026-08-02T08:00:00Z",
  "updated_at": "2026-08-02T08:12:00Z",
  "feedback": {
    "package_uri": "rambledesk://feedback/019...",
    "directory_path": "/absolute/app-data/feedback/20260802T081200Z-019...",
    "markdown_path": "/absolute/app-data/feedback/20260802T081200Z-019.../feedback.md",
    "manifest_path": "/absolute/app-data/feedback/20260802T081200Z-019.../manifest.json"
  }
}
```

## `get_feedback`

读取当前反馈请求状态，不改变状态。

输入：

```json
{
  "request_id": "019..."
}
```

规则：

- Unknown `request_id` returns `REQUEST_NOT_FOUND`.
- Terminal result SHOULD include feedback package metadata.
- 通用 MCP 适配器使用它进行手动 continuation 和断线恢复。
- 客户端 MUST NOT 用固定间隔空轮询作为默认等待路径。

## `recover_feedback`

从服务端恢复一个持久化请求，不创建替代请求。该 operation 用于 Pi 等维持原生
等待状态的 adapter，不属于 Generic MCP 工具面；MCP 持有 `request_id` 后直接调用
`get_feedback` 即可。

输入：

```json
{
  "request_id": "019...",
  "host_id": "pi",
  "host_session_id": "pi-session-019..."
}
```

规则：

- 客户端 SHOULD 提供原始 `request_id`。
- Server MUST 校验请求属于给定的 `(host_id, host_session_id)`。
- 适配器 MAY 用可信的 `X-RambleDesk-Host` 覆盖输入 `host_id`。
- 缺少 `request_id` 时，只有恰好一个候选请求才可恢复。
- 多个候选 MUST 返回 `RECOVERY_AMBIGUOUS`，不得擅自选择最新请求。
- 恢复只读取已有生命周期状态，不得创建新请求。

## `wait_feedback`

等待请求进入终态。该 operation 属于 application contract 和本地 JSON API，不属于通用 MCP 工具面。

输入：

```json
{
  "request_id": "019..."
}
```

规则：

- 多个 waiter MUST 被同一个终态释放。
- transport-level disconnect 只结束当前 wait attempt。
- 取消等待不等于取消反馈请求；取消请求必须显式调用 `cancel_feedback`。

## `cancel_feedback`

显式取消未完成请求。

输入：

```json
{
  "request_id": "019...",
  "reason": "The implementation changed; this request is obsolete."
}
```

规则：

- `waiting` 和 `in_progress` MAY become `cancelled`.
- `completed` returns `REQUEST_ALREADY_COMPLETED`.
- Repeated cancellation returns the original cancelled state and reason.
- Cancellation is a business terminal state, not a transport disconnect.

## 本地 JSON API

本地 JSON API 属于 `rambledesk-local-server`。

所有 endpoint：

- MUST listen on loopback only.
- MUST require bearer token.
- MUST enforce loopback Host header.
- MUST reject disallowed Origin.
- MUST use JSON request/response bodies.

### `POST /api/feedback/request`

输入：`FeedbackRequestInput`。

输出：与 `request_feedback` 相同。

If `X-RambleDesk-Host` is present, the server MAY treat it as authoritative `host_id` for that installed adapter path.

### `POST /api/feedback/get`

Input:

```json
{ "request_id": "019..." }
```

输出：与 `get_feedback` 相同；终态输出 MAY 包含 `feedback_package`.

### `POST /api/feedback/wait`

Blocking wait endpoint for native adapters.

Input:

```json
{ "request_id": "019..." }
```

终态输出：

```json
{
  "request_id": "019...",
  "status": "completed",
  "execution_mode": "wait",
  "feedback": {
    "directory_path": "/absolute/package/path",
    "markdown_path": "/absolute/package/path/feedback.md",
    "manifest_path": "/absolute/package/path/manifest.json"
  },
  "feedback_package": {
    "manifest": { "schema_version": 1, "attachments": [] },
    "markdown": "# RambleDesk Feedback\n...",
    "attachment_paths": ["/absolute/package/path/attachments/example.png"]
  }
}
```

### `POST /api/feedback/recover`

输入和业务规则与 `recover_feedback` 相同；终态输出 MAY 包含反馈包。

### `POST /api/feedback/approve`

由 RambleDesk 人类操作界面调用。Agent adapter 不得通过 MCP 或 Pi tool 自行批准
最终总结。

### `POST /api/feedback/cancel`

输入和业务规则与 `cancel_feedback` 相同。

## Generic MCP Adapter

工具面：

- `request_feedback`
- `get_feedback`
- `cancel_feedback`

通用 MCP 适配器没有 blocking wait tool。目标流程是：

1. 宿主智能体调用 `request_feedback`。
2. 宿主智能体结束当前 turn。
3. 人类在 RambleDesk 中提交或取消。
4. RambleDesk 显示手动 continuation 提示。
5. 人类返回宿主。
6. 宿主智能体调用 `get_feedback(request_id)`；发生 MCP transport 断线后仍调用
   同一个 `get_feedback(request_id)`，不需要单独的恢复工具。

可选优化（宿主交互确认工具）：若宿主提供原生交互确认工具（如 `ask`、
`ask_choice`），宿主智能体可以在步骤 2 用它替代"直接结束 turn"：在工具调用内
阻塞等待人类完成反馈，人类返回宿主后点击确认，宿主智能体随即调用
`get_feedback(request_id)`。该等待发生在宿主原生通道，不受 MCP call timeout
约束，也不消耗模型 token；headless 宿主或不支持交互确认的宿主继续使用默认
流程。手动 continuation 提示始终保留为兜底。

## Pi 原生适配器

Pi package 流程：

1. Pi 调用 `request_ramble_feedback`。
2. Pi package 调用 `/api/feedback/request`。
3. Pi package 在同一个 tool call 内调用 `/api/feedback/wait`。
4. 人类在 RambleDesk 中提交或取消。
5. Wait 返回终态反馈包给 Pi。

Pi 原生适配器不需要提交后的 continuation。

## 反馈包

完成的反馈包包含：

```text
feedback/<timestamp>-<request-id>/
├── feedback.md       # canonical Cooked Feedback；未启用 Cooking 时等同原稿
├── uncooked.md       # 人类直接产生的 Uncooked Feedback
├── manifest.json
└── attachments/
```

规则：

- 反馈包发布 MUST 不可变。
- `feedback.md` 是适配器和宿主默认读取的正式结果。
- `uncooked.md` MUST 保留，不得被 Cooking 覆盖。
- manifest MUST 记录两份 Markdown 的 SHA-256；启用 Cooking 时记录 provider/model 标识。
- API Key、Authorization header 和模型服务响应 metadata MUST NOT 写入反馈包。
- 反馈包路径 SHOULD 默认位于 RambleDesk 应用数据目录。
- 适配器 MAY 提供路径提示，但协议正确性 MUST NOT 依赖源码 checkout 根路径。
- manifest MUST 包含足够 metadata，用于校验附件 hash 并关联 `request_id`。

## 错误码

稳定错误码：

| 错误码 | 含义 |
| --- | --- |
| `INVALID_ARGUMENT` | Invalid input shape, string limit, UUID, or action id. |
| `REQUEST_NOT_FOUND` | Unknown `request_id`. |
| `REQUEST_CONFLICT` | Same `request_id`, different immutable input. |
| `REQUEST_ALREADY_COMPLETED` | Attempted to cancel or mutate completed request. |
| `REQUEST_TERMINAL` | Attempted to mutate terminal request. |
| `DRAFT_CONFLICT` | Stale draft revision. |
| `ATTACHMENT_LIMIT` | Attachment count or bytes exceeded. |
| `FEEDBACK_PACKAGE_READ_FAILURE` | Terminal package exists but cannot be read. |
