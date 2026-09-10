# RambleDesk 反馈协议

本文维护外部反馈适配器的输入、幂等、状态、结果与传输合同。MUST / SHOULD / MAY 分别表示必须、建议和可选；对象定义以[术语表](TERMINOLOGY.md)为准。

## 协议边界

Local Integration Server 的 `/mcp` 提供 Generic MCP tools，`/api/feedback/*` 提供 Pi、dsh 等原生适配器使用的 JSON API。二者映射到 core application contract，core 不持有 JSON/HTTP/MCP 实现。

本文不枚举 Workbench 的全部 Application Transport commands，也不把托管 Agent 入口当成外部协议。Web Access HTTP/WS 合同见[架构](ARCHITECTURE.md#application-transport-与恢复合同)；生产 ACP 的内置 feedback 命令和运行时归属见 [ACP 指南](ACP_MANAGED_SESSIONS.md)。

## 请求输入与身份

当前 Rust 输入类型为 `RequestFeedbackInput`。MCP tool input 与之对应，生成 schema 和 Rust DTO 是字段类型来源。

| 字段 | 必需 / 默认 | 规则 |
| --- | --- | --- |
| `request_id` | 可选，由服务端生成 | UUID；创建幂等 key，也是持久读取 key。 |
| `host_id` | 可选，默认 `generic` | 宿主家族；已安装适配器可通过 `RAMBLEDESK_HOST` / 可信 `X-RambleDesk-Host` 注入或覆盖。 |
| `host_session_id` | 必需 | 同一外部宿主会话的关联 id，不是认证凭据、MCP transport session 或自动恢复证明。 |
| `title` | 可选 | 工作台展示的短标题。 |
| `what_happened` | 必需 | 当前变化、背景或需要检查的事项。 |
| `actions` | 必需 | 1–20 项有序操作清单；每项为 `id` 与 `instruction`。 |
| `context_refs` | 可选，空列表 | 每项为 `label` 与 `uri`，仅提供可读上下文。 |
| `attachments` | 可选，空列表 | 人类需要审阅的 Markdown 或图片，内容规则见下文。 |
| `source_hint` | 可选 | 来源提示，可含路径或标题，不是身份或认证字段。 |
| `allow_finish` | 可选，默认 `false` | 仅在需要简单批准/拒绝的最终确认请求中启用。 |
| `final_summary` | `allow_finish=true` 时必需 | 人类可直接批准的确切结束语草稿；不能脱离 `allow_finish` 单独提供。 |

需要审阅、提意见、逐段反馈的请求 MUST 省略 `allow_finish`，不能用直接批准取代详细反馈。`actions[].id` MUST 匹配 `^[a-z0-9][a-z0-9_-]{0,63}$`，同一请求内唯一。

外部反馈 MUST NOT 要求源码 checkout 路径。`context_refs` / `source_hint` 里的路径只是提示，不执行或自动信任引用内容；`attachments[].path` 则明确授权服务端读取该本机文件作为附件内容。托管会话的执行目录不属于此身份合同。

```json
{
  "host_id": "pi",
  "host_session_id": "pi-session-example",
  "title": "Settings review",
  "what_happened": "The settings sidebar was changed.",
  "actions": [{ "id": "open-settings", "instruction": "Open settings and inspect the sidebar." }],
  "context_refs": [{ "label": "Instructions", "uri": "file:///absolute/path/README.md" }],
  "attachments": [{ "file_name": "shot.png", "path": "/absolute/path/shot.png" }]
}
```

### 附件内容

`RequestAttachmentInput` 的 `file_name` 与以下三种内容之一组成附件；MUST 恰好提供一种：

| 字段 | 规则 |
| --- | --- |
| `markdown` | 短 Markdown 正文；文件名以 `.md` 或 `.markdown` 结尾。 |
| `contents_base64` | 无现成文件的小图；必须解码为 PNG / JPEG / GIF / WebP。 |
| `path` | 本机现存普通文件的绝对路径；Markdown 扩展名按文档读取，其余必须是上述图片类型。 |

已有本地文件 SHOULD 使用 `path`；MCP/工具调用 MUST NOT 为传递本机已有图片而把整图读进 `contents_base64`。附件路径不是源码 checkout 身份，不授予额外目录管理能力。

## 状态与幂等

```text
waiting → in_progress → completed
   │           │
   └───────────┴──────→ cancelled
```

`completed` 与 `cancelled` 为终态。完成、取消、传输断开和 Agent 续接是不同事实：终态释放 waiter，但不代表所有路径都会续接 Agent；托管取消不入 continuation outbox。直接批准可以完成请求而没有反馈包。

`request_feedback` 创建或重新关联持久请求。服务端按 `request_id` 判断：

- 不存在：验证输入并持久化，然后返回。
- 已存在、不可变输入一致：返回原请求，包括原完成或取消结果，不重新打开。
- 已存在、不可变输入不同：返回 `REQUEST_CONFLICT`。

不可变输入包括规范化后的 `host_id`、`host_session_id`、`title`、`what_happened`、有序 `actions` / `context_refs` / `attachments`、`source_hint`、`allow_finish` 和 `final_summary`。附件比较使用实际读入的文件名、媒体类型、字节数和内容哈希，不以路径字符串替代内容。旧请求的兼容哈希由存储层保留，不允许客户端自行推导或绕过服务端幂等判断。

## 查询、等待与恢复

| Operation | 输入 | 行为 |
| --- | --- | --- |
| `get_feedback` | `request_id` | 只读取，不改变状态；未知 id 返回 `REQUEST_NOT_FOUND`。 |
| `wait_feedback` | `request_id` | 等待终态，多个 waiter 由同一结果释放；属于 application/JSON API，不是 Generic MCP tool。 |
| `recover_feedback` | 可选 `request_id`、可选 `host_id`、必需 `host_session_id` | 从持久请求恢复，校验所属宿主会话；不新建请求。 |
| `cancel_feedback` | `request_id`、取消原因 `reason` | 显式取消 waiting/in_progress；completed 返回 `REQUEST_ALREADY_COMPLETED`。重复取消返回原状态和原因。 |

客户端 SHOULD 保留并提供原 `request_id`。恢复时服务端校验 `(host_id, host_session_id)`，可信适配器头可覆盖 host；缺少 request id 时只有恰好一个候选可恢复，多个候选返回 `RECOVERY_AMBIGUOUS`，不得选最新请求代替。

Transport 断开或取消等待只结束本次 wait attempt，不取消持久请求。客户端 MUST NOT 把固定间隔空轮询作为默认等待路径。Generic MCP 持有 id 后直接 `get_feedback`，不需要单独的恢复工具。

## 返回结果

请求、查询和恢复返回请求状态，包含持久 `request_id`、宿主关联、状态和时间；`execution_mode` 描述 poll/wait 形式。已经发布的结果带 `feedback` 路径元数据。本地 JSON API 还可包含读入的 `feedback_package`：

```json
{
  "request_id": "01900000-0000-7000-8000-000000000001",
  "status": "completed",
  "execution_mode": "wait",
  "feedback": {
    "directory_path": "/absolute/library/feedback/example",
    "markdown_path": "/absolute/library/feedback/example/feedback.md",
    "manifest_path": "/absolute/library/feedback/example/manifest.json"
  },
  "feedback_package": {
    "manifest": { "schema_version": 1, "attachments": [] },
    "markdown": "# RambleDesk Feedback\n...",
    "attachment_paths": []
  }
}
```

以上是字段示意，不是完整 DTO。已发布结果 SHOULD 带反馈包元数据；没有包的批准结果不能虚构路径。读取已存在包失败应明确返回错误，不能伪装成尚未完成。

## 本地 JSON API

全部 endpoint MUST 仅监听 loopback、要求 bearer token、校验 loopback Host 并拒绝不允许的 Origin，使用 JSON 请求/响应。listener、token 和安全预算见[架构](ARCHITECTURE.md#安全边界)。

| Endpoint | 对应合同 |
| --- | --- |
| `POST /api/feedback/request` | `request_feedback` / `RequestFeedbackInput` |
| `POST /api/feedback/get` | `get_feedback` |
| `POST /api/feedback/wait` | 同一工具调用内的 `wait_feedback` |
| `POST /api/feedback/recover` | `recover_feedback` |
| `POST /api/feedback/cancel` | `cancel_feedback` |
| `POST /api/feedback/approve` | 人类操作界面的最终总结批准；Agent adapter 不得通过 MCP 或原生工具自行批准。 |

原生等待/恢复返回已有终态结果及可用的包内容，业务规则与上文相同。`X-RambleDesk-Host` 只有在受信任的适配器入口上下文中才作为 host 覆盖来源；它本身不是认证凭据。

## 适配器等待方式

Generic MCP 仅暴露 `request_feedback`、`get_feedback`、`cancel_feedback`：请求返回后 Agent 结束当前 turn，人类完成后按手动继续提示返回宿主，Agent 读取原 request id。MCP 断线后仍读取同一请求；没有 blocking wait tool，也不承诺自动恢复原可见上下文。

宿主如有原生交互确认工具，可以用它等人类返回并确认，然后调用 `get_feedback`。等待发生在宿主通道，受宿主自身能力约束，不增加 RambleDesk 的 MCP 工具；手动继续提示仍保留。

Pi / dsh 原生适配器经 JSON API 创建请求，并在原工具调用中 wait；中断后通过原 request id 恢复或读取。它们不需要提交后的 continuation，不把一次等待失败当作创建新请求的理由。

## 反馈包

```text
<library>/feedback/<timestamp>-<request-id>/
├── feedback.md       # 宿主默认读取的正式结果
├── uncooked.md       # 人类原始反馈证据
├── manifest.json
└── attachments/
```

- 发布后的包 MUST 不可变；manifest 关联 `request_id` 并包含附件哈希等校验信息。
- `feedback.md` 是正式结果；`uncooked.md` MUST 保留且不得被 Cooking 覆盖，关闭 Cooking 时二者可相同。
- manifest MUST 记录两份 Markdown 的 SHA-256；Cooking 使用 provider/model 标识，不保存 API Key、Authorization header 或模型服务响应 metadata。
- 包默认位于配置的 RambleDesk library，路径只保证同机、共享文件系统可见。适配器路径 hint 不成为协议前提。
- 发布失败不能形成可见的半成品；已发布但数据库尚未登记的包按原请求对账，不能重复发布。

## 错误码

| 错误码 | 含义 |
| --- | --- |
| `INVALID_ARGUMENT` | 输入形状、文本限制、UUID 或 Action id 不合法。 |
| `REQUEST_NOT_FOUND` | 未知 request id。 |
| `REQUEST_CONFLICT` | 同一 request id 的不可变输入不同。 |
| `REQUEST_ALREADY_COMPLETED` | 尝试取消或修改已完成请求。 |
| `REQUEST_TERMINAL` | 尝试修改终态请求。 |
| `DRAFT_CONFLICT` | 草稿 revision 过期。 |
| `ATTACHMENT_LIMIT` | 附件数量或字节数超限。 |
| `RECOVERY_AMBIGUOUS` | 无明确 request id 且存在多个恢复候选。 |
| `FEEDBACK_PACKAGE_READ_FAILURE` | 终态包存在但无法读取。 |

详细 DTO 与映射以 core/transport 的生成合同为准。新增字段或错误映射须同步合同与[协议检查](DEVELOPMENT.md#前后端合同)，不能只改文档示例。
