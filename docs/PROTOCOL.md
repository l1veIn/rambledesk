# RambleDesk 反馈协议

本文维护外部反馈适配器的输入、幂等、状态、结果与传输合同。MUST / SHOULD / MAY 分别表示必须、建议和可选；对象定义以[术语表](TERMINOLOGY.md)为准。

## 协议边界

Local Integration Server 的 `/mcp` 提供 Generic MCP tools，`/api/feedback/*` 提供反馈 JSON API，`/api/workbenches/*` 提供工作台目录查询。它们映射到 core application contract，core 不持有 JSON/HTTP/MCP 实现。Pi / dsh 原生工具的参数 schema 仍保留原 Ramble 入口；新增类型通过 Generic MCP、Generic JSON 或托管命令调用。

本文不枚举 Workbench 的全部 Application Transport commands，也不把托管 Agent 入口当成外部协议。Web Access HTTP/WS 合同见[架构](ARCHITECTURE.md#application-transport-与恢复合同)；生产 ACP 的内置 feedback 命令和运行时归属见 [ACP 指南](ACP_MANAGED_SESSIONS.md)。

## 请求输入与身份

当前 Rust 输入类型为 `RequestFeedbackInput`。MCP tool input 与之对应，生成 schema 和 Rust DTO 是字段类型来源。

| 字段 | 必需 / 默认 | 规则 |
| --- | --- | --- |
| `request_id` | 可选，由服务端生成 | UUID；创建幂等 key，也是持久读取 key。 |
| `host_id` | 可选，默认 `generic` | 宿主家族；已安装适配器可通过 `RAMBLEDESK_HOST` / 可信 `X-RambleDesk-Host` 注入或覆盖。 |
| `host_session_id` | 必需 | 同一外部宿主会话的关联 id，不是认证凭据、MCP transport session 或自动恢复证明。 |
| `title` | 可选 | 工作台展示的短标题。 |
| `what_happened` | 必需 | 1–200 字符的现状摘要；人类先读它，在数秒内理解发生了什么。 |
| `actions` | 未提供 `workbench` 时必需 | 旧 Ramble 入口的 1–20 项有序体验动作，每项为 `id` 与 `instruction`；指定工作台类型时省略顶层 `actions`。 |
| `workbench` | 可选 | `{type, version, data}`；省略时保持旧 Ramble 语义。类型专属 schema 通过 `describe_workbench` 查询。 |
| `context_refs` | 可选，空列表 | 每项为 `label` 与 `uri`，仅提供可读上下文。 |
| `attachments` | 可选，空列表 | 人类需要审阅的 Markdown 或图片，内容规则见下文。 |
| `source_hint` | 可选 | 来源提示，可含路径或标题，不是身份或认证字段。 |
| `allow_finish` | 可选，默认 `false` | 仅 Ramble（含未指定工作台的旧入口）支持；用于需要简单批准/拒绝的最终确认请求。 |
| `final_summary` | `allow_finish=true` 时必需 | 人类可直接批准的确切结束语草稿；不能脱离 `allow_finish` 单独提供。 |

`what_happened` MUST 是一段人类可扫读的摘要：200 字符内讲清发生了什么、为什么需要反馈。详细材料放在类型专属 `data` 或请求附件中，而不是继续加长本字段；超过 200 字符的请求按 `INVALID_ARGUMENT` 拒绝，服务端不截断也不静默接受。字数按 Unicode 标量值计，中英文同权，不按 UTF-16 code unit 计数。

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

### 工作台发现与类型合同

1. `list_workbenches({offset?, limit?})` 返回可用的第一方类型目录：`type`、`version`、`name`、`purpose`、`returns` 与 `interaction`。默认每页 5 项，最多 20 项；`next_offset` 表示下一页。目录不内嵌各类型完整 schema。
2. `describe_workbench({type, version?})` 返回所选类型的输入 schema、结果 schema、示例与说明。`version` 省略时当前选择 v1；已知合同可直接创建，无须每次重复发现。
3. `request_feedback` 使用同一持久请求生命周期。类型、版本与输入不匹配 MUST 在持久化之前以 `INVALID_ARGUMENT` 拒绝，不能静默改成 Ramble。

`interaction` 只作目录分类；`type/version` 才是验证和解释请求的合同身份。工作台选择不改变认证、会话归属、等待或 continuation。

| 类型 | 人类输入与完成条件 | 结构化提交结果 |
| --- | --- | --- |
| `ramble` v1 | `data.actions` 给出体验动作；反馈正文提供人类输入。 | `kind: "free_feedback"`；实际正文与附件在包的公共位置。 |
| `questions` v1 | 一个或多个问题，逐题选择稳定 `value`，允许时可键入或口述自定义回答；每题都需回答。单题且 `allowOther:false` 覆盖严格单选。 | `answers: [{id,value,label,wasCustom,index?}], cancelled: false`。 |
| `document_review` v1 | 审阅不可变原稿，可作段落标记、批注与改写建议；必须显式给出整稿判断。 | `source_version, verdict, annotations, paragraph_marks`；详见[文稿审阅合同](workbench/document-review.md)。 |
| `web_review` v1 | 浏览真实网页，对选中元素就地批注并集中回看；网页需允许嵌入，跨源页面需接入评审桥接脚本。 | `source_version, annotations`，每条保留页面 URL、视口、元素定位和意见；详见[网页评审](workbench/web-review.md)。 |

| `terminal` v1 | 在请求目录中亲自试用 CLI；建议命令只填入，输出可引用到反馈正文。 | `sessions`，保留目录、shell、尺寸、ANSI 转录、最新画面与会话终态；详见[终端试用](workbench/terminal.md)。 |

发现目录提供上述五种类型。新方案选择请求使用 `questions`，不再创建独立 `single_choice` 工作台。`describe_workbench(single_choice)` 提示改用单题问答。

旧 `single_choice` v1 合同保留兼容：已知旧客户端仍可原样创建/重试，已有请求、草稿和结果保持 `selected_option_id` 及 `status`，不静默重写为 `answers[]`，以免改变请求幂等比较或结果含义。客户端使用同一个问答视图承载旧单选，兼容层转换展示和交互状态，不迁移持久输入。未回答状态仍不能成功提交。

问答的 `cancelled` 在成功结果中仍为 `false`，真实取消的 `workbench.result` 为 `null`。旧单选结果的 `unanswered` 不是新增请求状态，不应据此推断请求是否取消或完成。

能够识别工作台 envelope 的客户端遇到未知类型/版本的已有请求时 MUST 只读保留材料、正文和交互数据，不覆盖保存、不提交、不通过直接批准绕过合同。该只读状态不改变请求生命周期；显式取消仍按通用请求操作处理。此保护不追溯适用于不认识工作台 envelope 的 0.3.3；旧安装包的回退限制见[数据兼容](DATA_COMPATIBILITY.md)。

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

不可变输入包括规范化后的 `host_id`、`host_session_id`、`title`、`what_happened`、有序 `actions` / `context_refs` / `attachments`、`source_hint`、`allow_finish` 和 `final_summary`。提供 `workbench` 时，其完整类型、版本和专属输入也参与比较；文稿原文、段落顺序与来源版本不能在同一请求内改写。附件比较使用实际读入的文件名、媒体类型、字节数和内容哈希，不以路径字符串替代内容。未指定工作台的旧请求保持原兼容哈希；不允许客户端自行推导或绕过服务端幂等判断。

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

类型专属结果位于 `feedback_package.manifest.workbench.result`，同一 `workbench` 同时保留原请求的 `type/version/data`。Agent MUST 以结构化结果读取选择、批注或判断，不从 `feedback_package.markdown` 猜测答案；正文只是自由反馈或补充说明，可以为空。MCP 提供结构化内容及有界文本预览，预览截断不意味着原始结果丢失。

Draft 中未完成的交互状态不等于已发布 Result。提交时服务端 MUST 同时检查请求未终止、草稿 revision、存在有效人类输入和该类型的完整性，并根据不可变请求重新确定选项标签、顺序与文稿锚点。正文与交互状态 MUST 来自同一次提交快照。取消不发布作答结果；直接批准最终总结的语义保持独立，不能作为必答类型的替代提交方式。

## 本地 JSON API

全部反馈与目录 API endpoint MUST 仅监听 loopback、要求 bearer token、校验 loopback Host 并拒绝不允许的 Origin，使用 JSON 请求/响应。listener、token 和安全预算见[架构](ARCHITECTURE.md#安全边界)。`GET /web-review/bridge.js` 是校验 loopback Host 的公开静态 JavaScript，只有页面选择桥接代码，不含请求、草稿或凭证；Agent 可下载后在待评审项目中自行托管。

| Endpoint | 对应合同 |
| --- | --- |
| `POST /api/feedback/request` | `request_feedback` / `RequestFeedbackInput` |
| `POST /api/feedback/get` | `get_feedback` |
| `POST /api/feedback/wait` | 同一工具调用内的 `wait_feedback` |
| `POST /api/feedback/recover` | `recover_feedback` |
| `POST /api/feedback/cancel` | `cancel_feedback` |
| `POST /api/feedback/approve` | 人类操作界面的最终总结批准；Agent adapter 不得通过 MCP 或原生工具自行批准。 |
| `POST /api/workbenches/list` | `list_workbenches`，只读目录查询。 |
| `POST /api/workbenches/describe` | `describe_workbench`，只读类型合同查询。 |

原生等待/恢复返回已有终态结果及可用的包内容，业务规则与上文相同。`X-RambleDesk-Host` 只有在受信任的适配器入口上下文中才作为 host 覆盖来源；它本身不是认证凭据。

## 适配器等待方式

Generic MCP 暴露 `list_workbenches`、`describe_workbench`、`request_feedback`、`get_feedback`、`cancel_feedback`。目录查询不创建请求，也不确认交付。请求返回后 Agent 结束当前 turn，人类完成后按手动继续提示返回宿主，Agent 读取原 request id。MCP 断线后仍读取同一请求；没有 blocking wait tool，也不承诺自动恢复原可见上下文。

宿主如有原生交互确认工具，可以用它等人类返回并确认，然后调用 `get_feedback`。等待发生在宿主通道，受宿主自身能力约束，不增加 RambleDesk 的 MCP 工具；手动继续提示仍保留。

Pi / dsh 原生适配器经 JSON API 创建请求，并在原工具调用中 wait；中断后通过原 request id 恢复或读取。它们不需要提交后的 continuation，不把一次等待失败当作创建新请求的理由。

## 反馈包

```text
<library>/feedback/<timestamp>-<request-id>/
├── feedback.md       # 正式自由正文或补充说明
├── uncooked.md       # 人类原始正文
├── manifest.json     # 请求、结构化结果、附件与校验信息
└── attachments/
```

- 发布后的包 MUST 不可变；manifest 关联 `request_id` 并包含附件哈希等校验信息。
- `feedback.md` 是正式正文；类型专属结构化结果以 manifest 为准。`uncooked.md` MUST 保留且不得被 Cooking 覆盖，关闭 Cooking 时两份正文可相同。Cooking 只处理正文，没有正文的结构化提交跳过 Cooking。
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
