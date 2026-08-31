# RambleDesk 应用协议

> 状态：v3 Managed ACP Path 可执行基线；Adapter Session 共存边界已确定。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文若与术语表冲突，以术语表为准。

本文定义 RambleDesk v3 Core Interface、Managed ACP Path 映射、Session Toolset 与稳定错误。Adapter Runtime 的既有 transport 合同见 [COMPATIBILITY.md](COMPATIBILITY.md)；两者只在 Desktop Unified Workbench Projection 汇合。规范词 MUST / SHOULD / MAY 分别表示必须、建议和可选。

## 协议分层

| 层 | 责任 |
| --- | --- |
| Core Interface | Session、Ramble Submission、Feedback Request、Package、Delivery、幂等与事务。 |
| Managed Session Interface | Launch Preflight、Agent Run、ACP Session Link、Prompt、Permission、Question、恢复与对账。 |
| ACP wire | initialize、session lifecycle、session update、permission、elicitation、cancel 与 JSON-RPC correlation。 |
| Session Toolset | `request_feedback`、`get_feedback`，以及 capability 验证后的 Question Channel。 |
| Artifact access | 以 Artifact Entry 和 Artifact Locator 交付内容，不暴露 Package 本地目录合同。 |

现有 `/mcp`、`/api/feedback/*`、Pi wait 与原生 Adapter 由维护冻结的 Adapter Runtime 继续拥有，不进入 v3 Core。Desktop 可以并列展示 Adapter Session 与 Managed ACP Session，但保存、提交、取消和 continuation 必须按 Session Source 返回原 owner；禁止双写或跨 source fallback。

## 标识与幂等

| 字段 | 生成方 | 规则 |
| --- | --- | --- |
| `session_id` | Core | RambleDesk Session 的稳定身份。 |
| `submission_id` | Workbench/调用方 | 每次正式 Ramble 在提交前生成；所有重试 MUST 复用。 |
| `request_id` | 调用方或 Core | Feedback Request 的稳定 lookup key；拿到返回值后的所有交互 MUST 复用。 |
| `package_id` | Core | 不可变 Package 身份；同一 Submission 只能有一个。 |
| `artifact_id` | Core | Package、Request 或 Draft 内稳定 Artifact Entry 身份；不是全局 blob key。 |
| `delivery_id` | Core | 一个 Feedback Request 的唯一交付身份。 |
| `acp_session_id` | Agent | opaque identity；只在 ACP Session Link 中保存。 |
| `work_id` | Core | 持久 Agent side effect 的重试身份。 |

通用幂等规则：

- 相同稳定 id 与相同 Core-computed `submission_digest` MUST 返回同一组稳定 identity；其中 Agent work / Delivery state MAY 反映当前投影。
- 相同稳定 id 与不同 `submission_digest` MUST 返回 `IDEMPOTENCY_CONFLICT`。
- 服务端生成 id 的首次调用若结果未知，不能声称可安全重试；Managed Path SHOULD 在进入 Core 前生成并保留稳定 id。
- 幂等返回不能重复发布 Package、创建 Session、固定 resolution 或追加 Agent work。

Core MUST 从完整 command 与 Artifact digest 重新计算版本化 `submission_digest`；调用方 MAY 省略 assertion，提供时只用于核对，不能被要求复制 Core 私有 canonical 算法。Package 另有位置无关 `content_digest`，完整 manifest 另有 `manifest_digest`。三者都不包含临时 locator、绝对路径或传输 metadata。

## Launch

### Launch Preflight

Launch Preflight 输入：

```json
{
  "agent_profile_id": "codex",
  "launch_profile_id": "codex-acp-local"
}
```

输出：

```json
{
  "available": true,
  "agent_version": "opaque display string",
  "config_options": [],
  "capabilities": {
    "load_session": true,
    "resume_session": true,
    "elicitation_form": true
  },
  "warnings": []
}
```

规则：

- Preflight MAY 短暂启动 Agent、完成 initialize、创建临时探测 Session 后立即拆除。
- Preflight MUST NOT 创建 RambleDesk Session、Launch Submission、Package、ACP Session Link 或用户历史。
- UI MUST 展示 Agent 实际返回的 config options；不得用静态列表伪造 capability。
- config options 是首选；只有 Agent 未提供时才 MAY 读取兼容的 modes 信息。

### `launch`

输入：

```json
{
  "submission_id": "019...",
  "submission_digest_assertion": "sha256:...",
  "launch_configuration": {
    "agent_profile_id": "codex",
    "launch_profile_id": "codex-acp-local",
    "workspace_reference": "/absolute/workspace",
    "model": "agent-returned-option",
    "reasoning_effort": "agent-returned-option",
    "access_mode": "workspace_write",
    "agent_config_json": "{}"
  },
  "ramble": {
    "document_json": {},
    "body_markdown": "Implement the requested change.",
    "artifacts": []
  }
}
```

`access_mode` 的产品值固定为：

- `read_only`
- `workspace_write`
- `yolo`

Launch Profile MUST 把它映射到 Agent 实际支持的 config option 或启动参数。映射不存在时 MUST 返回 `UNSUPPORTED_ACCESS_MODE`，不得假装已生效。

成功输出：

```json
{
  "session_id": "019...",
  "submission_id": "019...",
  "package_id": "019...",
  "agent_work_id": "019...",
  "agent_work_state": "pending"
}
```

Core MUST 在同一事务中：

1. 固定 Launch Submission；
2. 发布 `package_purpose=launch` 的 Package metadata；
3. 创建唯一 Managed Session 与 Launch Configuration；
4. 创建唯一 `launch_prompt` Agent work。

Artifact bytes 可以在事务前先写入 content-addressed Store；事务失败后的未引用对象由垃圾回收清理。Package 在数据库事务成功前不得对外可见为已发布。

## Steering

### `steer`

输入：

```json
{
  "submission_id": "019...",
  "session_id": "019...",
  "submission_digest_assertion": "sha256:...",
  "ramble": {
    "document_json": {},
    "body_markdown": "Please keep the existing command names.",
    "artifacts": []
  }
}
```

Core 固定 Steering Submission 与唯一 `steering_prompt` Agent work。Steering 不创建 Feedback Package。

如果 Session 正在等待 Permission 或 Ask Question，UI SHOULD 明确区分“回答当前请求”和“追加 Steering”；不得用 Steering 文本隐式回答 JSON-RPC request。

## Ramble Draft

`mutate_draft` 以 `draft_id + expected_revision` 提供 CAS 保存、添加 Artifact、删除 Artifact 与重排 Artifact 四类 mutation。Draft 的身份必须与 Ramble Intent 一致：

- Launch Draft 尚未创建 Session，因此 `session_id` 与 `request_id` 都为空，并持有完整 `launch_configuration`；
- Steering Draft 必须绑定 `session_id`，不绑定 Request，也不持有 Launch Configuration；
- Feedback Draft 必须同时绑定 `session_id` 与 `request_id`，不持有 Launch Configuration。

`document_json` 是由 Workbench Editor 负责生成和解释的 opaque、versioned 结构化真源；Core 与 Storage 不解析其内部节点，`body_markdown` 是同一 revision 的投影。保存正文或修改 Artifact 都使 revision 加一。revision 不匹配返回 `DRAFT_CONFLICT`，不得覆盖更新后的 Draft。编辑 Draft 不改变 Feedback Request 的 `waiting` 状态。

## Feedback Request

### `request_feedback`

这是 Managed ACP Session Toolset 的应用 operation。输入：

```json
{
  "request_id": "optional stable UUID",
  "session_id": "injected by trusted Session Toolset",
  "source_link_id": "optional current ACP Session Link",
  "title": "Review the new launch flow",
  "instructions": "Use the desktop build and judge whether the flow feels natural.",
  "actions": [
    {
      "id": "launch-codex",
      "instruction": "Launch a Codex session in Workspace Write mode."
    }
  ],
  "context_refs": [
    {
      "label": "Acceptance notes",
      "uri": "rambledesk-context://019..."
    }
  ]
}
```

规则：

- Managed Session Toolset MUST 注入可信 `session_id` 与当前 `source_link_id`，不得接受模型覆盖。
- `actions` MUST 包含 1–20 项；id 在同一 Request 内唯一并匹配 `^[a-z0-9][a-z0-9_-]{0,63}$`。
- `context_refs` 是供人类判断的引用。RambleDesk 不自动执行其中内容，也不要求它是本机路径。
- 输入不包含 `allow_finish`、`final_summary` 或批准捷径。只需一个即时选择时 Agent 应使用 Ask Question。
- 创建成功后工具 MUST 立即返回，不等待人类。
- 同一 `request_id` + 同一输入返回原 Request；同一 id + 不同输入返回 conflict。

返回：

```json
{
  "request_id": "019...",
  "session_id": "019...",
  "status": "waiting",
  "created_at": "2026-08-30T08:00:00Z",
  "instruction": "End the current turn. Feedback may arrive in a future Agent Run."
}
```

Feedback Request 的状态投影只有：

- `waiting`：`resolution` 为空；
- `submitted`：人类提交了 `response` Package；
- `cancelled`：人类明确取消，没有 Package。

`submitted` 与 `cancelled` 是 resolution，不存在 `in_progress`、`completed` 或 `approved`。

### `get_feedback`

输入：

```json
{
  "request_id": "019..."
}
```

waiting 返回：

```json
{
  "request_id": "019...",
  "status": "waiting"
}
```

submitted 返回稳定 Feedback Delivery Envelope：

```json
{
  "delivery_id": "019...",
  "request_id": "019...",
  "session_id": "019...",
  "resolution": "submitted",
  "package": {
    "package_id": "019...",
    "package_purpose": "response",
    "content_digest": "sha256:...",
    "manifest_digest": "sha256:...",
    "feedback_markdown": "The launch controls are clear, but...",
    "artifacts": [
      {
        "artifact_id": "screenshot-1",
        "role": "attachment",
        "media_type": "image/png",
        "size_bytes": 120034,
        "sha256": "...",
        "locator": {
          "kind": "opaque_ref",
          "value": "rambledesk-artifact://019.../screenshot-1"
        }
      }
    ]
  }
}
```

cancelled 返回：

```json
{
  "delivery_id": "019...",
  "request_id": "019...",
  "session_id": "019...",
  "resolution": "cancelled",
  "reason": "The implementation changed; this review is obsolete."
}
```

规则：

- `get_feedback` 是读取 operation，不因读取本身改变人类 resolution。
- 相同 Request 的所有 terminal 读取 MUST 返回相同 `delivery_id` 与 Package identity。
- Agent MUST 按 `delivery_id` 去重。
- Artifact Locator MAY 更新或过期；更新 locator 不改变 Package digest。
- 未知 `request_id` 返回 `REQUEST_NOT_FOUND`。

## 人类解决 Feedback Request

### 提交 Feedback Ramble

输入：

```json
{
  "submission_id": "019...",
  "request_id": "019...",
  "expected_draft_revision": 7,
  "submission_digest_assertion": "sha256:...",
  "document_json": {},
  "uncooked_markdown": "原始反馈...",
  "feedback_markdown": "整理后的正式反馈...",
  "cooking_model": "opaque model id",
  "artifacts": []
}
```

Core MUST 原子固定：

- Feedback Submission；
- `package_purpose=response` Package；
- Request `submitted` resolution；
- 唯一 pending Feedback Delivery；
- 唯一 `feedback_resume` Agent work。

该提交路径只对 Managed ACP Session 开放。Imported Session 是可选迁移产生的只读事实快照；尝试通过 Managed Feedback resolution Interface 提交或取消其 Feedback Request 返回 `SESSION_NOT_MANAGED`，不得留下不可消费的 Agent work 或 Delivery。Adapter Session 的提交与取消不经过该 Interface，而是直接回到 Adapter Runtime。

相同 `submission_id` 的安全重试返回同一 Package、Delivery 与 work identity，state 使用当前投影。另一个 Submission 试图解决已终态 Request 返回 `REQUEST_TERMINAL`。

### 取消 Feedback Request

输入：

```json
{
  "request_id": "019...",
  "reason": "The requested build no longer exists."
}
```

Core MUST 原子固定 `cancelled` resolution、唯一 pending Delivery 与 `feedback_resume` Agent work。重复取消返回同一组稳定 identity 与当前 state；取消 submitted Request 返回 `REQUEST_TERMINAL`。

取消不创建 Ramble Submission 或空 Package。

## Feedback Package manifest

规范 manifest：

```json
{
  "schema_version": 3,
  "package_id": "019...",
  "package_purpose": "launch",
  "submission_id": "019...",
  "request_id": null,
  "content_digest": "sha256:...",
  "artifacts": [
    {
      "artifact_id": "feedback",
      "position": 0,
      "role": "feedback",
      "display_name": "feedback.md",
      "media_type": "text/markdown; charset=utf-8",
      "size_bytes": 1234,
      "sha256": "..."
    },
    {
      "artifact_id": "uncooked",
      "position": 1,
      "role": "uncooked",
      "display_name": "uncooked.md",
      "media_type": "text/markdown; charset=utf-8",
      "size_bytes": 1400,
      "sha256": "..."
    }
  ],
  "published_at": "2026-08-30T08:00:00Z"
}
```

规则：

- `package_purpose` MUST 是 `launch` 或 `response`。
- `response` MUST 有 `request_id`；`launch` MUST 没有 `request_id`。
- `content_digest` 覆盖 purpose、response request binding 与按 position 排序的 Artifact descriptor，但不覆盖 Package/Submission id、时间、storage key 或 locator。
- `manifest_digest` 在 manifest 本体之外保存，覆盖完整 canonical manifest；不得把自身纳入计算。
- manifest MUST 不包含绝对路径、临时 URL、opaque storage key、Authorization、API Key 或 Agent transcript。
- `feedback` 是 Agent 默认消费的正式正文；`uncooked` 是人类原始证据。
- Artifact descriptor 按 `(position, artifact_id)` 排序，二者在同一 Package 内各自唯一。
- 未使用 Cooking 时两者 bytes MAY 相同，但仍是两个 role。
- Package 发布后 manifest 与 Artifact bytes MUST 不可变。
- Locator 属于某次读取或 Delivery Envelope，不属于 manifest。

## ACP Session 建立与恢复

Managed Session 的建立顺序：

1. 启动 Agent Run。
2. 完成 ACP initialize，保存当前 capability snapshot。
3. 根据 capability 构造 Session Toolset 的 MCP server 配置。
4. 新 Session 调用 `session/new`；恢复 Session 按以下顺序尝试。

恢复顺序：

```text
if resume capability and current ACP Session Link exists:
    session/resume(sessionId, cwd, mcpServers)
else if load capability and current ACP Session Link exists:
    session/load(sessionId, cwd, mcpServers)
else:
    session/new(cwd, mcpServers)
    persist a new ACP Session Link
    send Recovery Prompt
```

规则：

- `session/resume` 与 `session/load` 前 MUST 检查 initialize capability。
- cwd 来自 Session 的 Workspace Reference，并 MUST 是绝对路径。
- 每次 resume/load MUST 重新提供完整 Session Toolset 配置；不能假设 Agent 记住旧连接。
- `session/load` replay 的 event 只进入当前 Context View，不写 RambleDesk transcript。
- `session/resume` 不期待 replay；成功后直接对账 pending work。
- `session/new` fallback 不创建新的 RambleDesk Session，只增加新的 ACP Session Link。
- Agent 返回的 config options/current values 是 live 真源；必要时更新 UI 投影，但不改写最初 Launch Submission。

## Agent work 与 Delivery 完成条件

### Launch 与 Steering

- ACP Client claim pending Agent work 后发送 `session/prompt`。
- 只有 Agent 接受对应 Prompt request，并且其终止结果可归属到同一 `work_id` 时才 complete。
- 连接在接受结果不确定时断开，work 保持可重试；Prompt 必须携带稳定 work marker，使 Agent 能识别重复输入。
- ACP Client 记录 retryable failure 时 MUST 使用原 `work_id + claim_token` 释放 lease 并保存稳定错误码；不得创建替代 work。
- 每次成功 claim 都递增 work `attempt_count`；`feedback_resume` 同时递增对应 Delivery attempt。
- Retry disposition 把 work 重新置为 `pending`，保留最近 `last_error_code/at`，Delivery 仍为 `pending`；成功完成时才清除最近错误。

### Feedback Resume

基线 Prompt MUST 包含：

- `request_id`
- `delivery_id`
- 明确要求调用 `get_feedback(request_id)`
- 明确要求按 `delivery_id` 去重
- 若是新 ACP Session，包含足以解释原任务的 Recovery Context，而不是本机 Package 路径

Delivery 只有在 ACP Client 观察到对应 `get_feedback` tool call 成功，且承载它的 Prompt Turn 正常结束后才标记 `delivered`。任何不确定失败都保持 `pending` 并使用同一 id 重试。

Recovery Context 由 Core 的 `SessionRecoverySnapshot` 生成，只含最初 Launch Ramble、后续 Steering Ramble、Launch Configuration、成对的 terminal Feedback Request + pending Delivery 与 pending Agent work；它不包含或推断 Agent transcript。Feedback Request 与 Delivery 必须属于同一 Session 和同一 `request_id`，这样新 Agent 无需 transcript 也能理解人类反馈所回答的原问题。

## Permission Request

ACP `session/request_permission` 直接投影为 Permission Request：

```json
{
  "live_request_id": "json-rpc correlation owned by ACP Client",
  "session_id": "019...",
  "tool_call": {
    "tool_call_id": "call_001",
    "title": "Run project tests",
    "kind": "execute",
    "raw_input": {}
  },
  "options": [
    {
      "option_id": "allow-once",
      "name": "Allow once",
      "kind": "allow_once"
    },
    {
      "option_id": "reject-once",
      "name": "Reject",
      "kind": "reject_once"
    }
  ]
}
```

规则：

- ACP Client MUST 保存原 JSON-RPC correlation，答案只能回到原 request。
- UI MUST 展示 Agent 给出的全部选项，不把它简化成固定 yes/no。
- 多个 pending Permission Request MUST 独立排队。
- Access Mode MAY 根据用户已确认设置自动回答，但自动结果仍必须是 Agent 提供的 option。
- Prompt Turn 取消时所有未回答 Permission Request MUST 回 `cancelled` outcome。
- Agent Run 断开后 Permission Request 不写入持久 Inbox。

## Ask Question

产品只暴露 Ask Question，不暴露 “Elicitation Request” 作为独立类型。

首选 transport 是 ACP `elicitation/create` form。Implementation 把 flat object schema 中的 enum/string/boolean 字段投影为一个或多个结构化问题，并把人类最终表单回答回给原 JSON-RPC request。

其他 Agent MAY 使用经该 Launch Profile 验证的 Question Channel。不得因为 `session/new` 支持 MCP server 配置，就推断任意 MCP 工具可以无限等待。

规则：

- Ask Question MUST 关联 active Agent Run 与原 live request。
- UI MUST 提供回答、跳过/拒绝和取消当前 turn 的明确操作。
- Agent 未声明或未通过 Question Capability 验收时，Workbench 不宣称支持。
- live 通道存在时可以无限等待人类；通道断开时返回 cancel/error，不持久化恢复。
- 需要真实体验、附件、自由 ramble 或跨重启等待时 MUST 使用 Feedback Request。

## Session Toolset

Managed Session 至少尝试提供：

- `request_feedback`
- `get_feedback`

Question Channel 是否通过 ACP elicitation、Agent 原生工具或经验证的 injected tool 提供，由 Launch Profile capability 决定。

规则：

- ACP Agent 必须支持 stdio MCP server 配置才可使用基线 Session Toolset。
- Toolset 注入失败时 Launch Preflight 或 Session 建立 MUST 明确降级/失败，不得在 UI 中显示虚假能力。
- `request_feedback` 必须短调用；持久等待发生在 RambleDesk Request 状态，不发生在 MCP request。
- `get_feedback` 返回内容与位置无关的 Delivery Envelope。
- Toolset config digest 保存在 ACP Session Link 中；恢复时变更必须显式记录并重新注入。

## Attention Item

Workbench snapshot 把三类来源归一化：

```json
{
  "attention_items": [
    {
      "kind": "feedback",
      "stable_id": "request:019...",
      "session_id": "019...",
      "recoverable": true
    },
    {
      "kind": "permission",
      "stable_id": "live:...",
      "session_id": "019...",
      "recoverable": false
    },
    {
      "kind": "question",
      "stable_id": "live:...",
      "session_id": "019...",
      "recoverable": false
    }
  ]
}
```

Attention Item 是 read model，不是统一写模型。`recoverable=true` 只适用于 waiting Feedback Request。

## 稳定错误码

| 错误码 | 含义 |
| --- | --- |
| `INVALID_ARGUMENT` | 输入 shape、限制、id 或 digest 无效。 |
| `IDEMPOTENCY_CONFLICT` | 相同稳定 id 携带不同 canonical digest。 |
| `SESSION_NOT_FOUND` | 未知 RambleDesk `session_id`。 |
| `SESSION_NOT_MANAGED` | operation 要求 Managed ACP Session，但目标是 Imported Session。 |
| `ACP_SESSION_LINK_NOT_FOUND` | Feedback Request 声明的来源 Link 不存在或不属于目标 Session。 |
| `REQUEST_NOT_FOUND` | 未知 `request_id`。 |
| `REQUEST_TERMINAL` | 试图再次解决 terminal Feedback Request。 |
| `DRAFT_CONFLICT` | Draft revision 已过期。 |
| `ARTIFACT_NOT_FOUND` | Artifact Entry 存在，但内容不可读取。 |
| `ARTIFACT_DIGEST_MISMATCH` | Artifact bytes 与 manifest digest 不一致。 |
| `WORK_NOT_FOUND` | 待处理 Agent work 不存在。 |
| `WORK_CLAIM_CONFLICT` | Agent work lease、claim token 或完成边界已失效。 |
| `CORRUPT_DATA` | 已持久化的领域事实彼此矛盾。 |
| `STORAGE_FAILURE` | Fact Store 或 Artifact Store 暂时失败。 |
| `AGENT_UNAVAILABLE` | Launch Profile 不可启动。 |
| `ACP_INITIALIZE_FAILED` | Agent Run 未完成 ACP initialize。 |
| `ACP_SESSION_RECOVERY_FAILED` | resume/load/new 全部失败。 |
| `UNSUPPORTED_ACCESS_MODE` | Launch Profile 无法实现所选 Access Mode。 |
| `UNSUPPORTED_CAPABILITY` | 当前 Agent Run 不支持请求的 Permission、Question、load 等能力。 |
| `LIVE_REQUEST_GONE` | Permission 或 Ask Question 的原 live request 已取消或断开。 |
| `DELIVERY_PENDING` | 人类事实已提交，但 Agent 尚未确认交付。 |

错误响应 MUST 区分“人类事实未提交”和“事实已提交但 Agent side effect pending”。后者不得诱导 UI 重复提交。

## Adapter Runtime 与 Imported Session 边界

- Generic MCP、Pi 与原生 Adapter 继续创建和操作 Adapter Session，不借道 v3 Core，也不获得 Managed ACP 的进程、历史或自动 resume 承诺。
- 可选迁移器可以把可解释的旧事实写成 Imported Session，并保存显式来源映射；它不得自动停用 Adapter Runtime 或删除旧 store。
- Imported Session 不复活 `host_id`、`host_session_id`、旧状态机或本地 Package path 结果，也不重新唤醒原 Agent。
- Unified Workbench Projection 必须利用迁移来源映射避免同时展示同一来源的 Imported Session 与 Adapter Session。
- 未来若要把某个 Adapter 能力迁入 v3 Core，必须逐能力定义新合同并完成端到端验收，不能以失败回退代替 owner 路由。
