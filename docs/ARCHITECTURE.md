# RambleDesk 架构基线

> 状态：v3 Unified Workbench 共存基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文描述 Module、Interface、Seam 与持久化实现。
> 本文描述分支最终目标架构，不代表当前 Desktop 已完成接线；实际阶段以 [V3_IMPLEMENTATION_PLAN.md](V3_IMPLEMENTATION_PLAN.md) 为准。

## 架构目标

v3 把 Session 提升为顶层对象，并在不合并 Core 的前提下让 Managed ACP Session 与 Adapter Session 进入同一个 Workbench。架构必须同时满足：

1. `core` 协议中立，只拥有 RambleDesk 领域事实与原子 use case。
2. `rambledesk-acp-client` 是深 Module：调用方不编排 initialize、进程树、JSON-RPC、resume/load/new、权限关联或 Delivery 重试。
3. Agent transcript 不进入 RambleDesk 数据模型；live event 与持久事实明确分离。
4. Launch、Steering 与 Feedback 的 Agent side effect 都从持久 intent 对账，避免“数据库成功但 Prompt 丢失”或重复发送。
5. Package 身份与 Artifact 存储实现分离，本地路径不进入内容合同。
6. v3 Core 只读写新表，Adapter Runtime 只读写既有 store；Desktop 只合并投影，不双写、不跨 Core fallback。
7. 旧数据无需迁移即可在 Adapter Session 中查看并继续原操作；有损迁移是显式可选项。

## 运行时拓扑

```text
┌──────────────────────────────── Workbench ────────────────────────────────┐
│ Launch │ Sessions / Context View │ Inbox │ Ramble Workspace │ Settings    │
└───────────────────────────────┬────────────────────────────────────────────┘
                                │ Tauri commands + live event stream
                                ▼
┌──────────────────────── apps/desktop composition root ────────────────────┐
│ source adapters → Unified Workbench Projection → source-aware commands    │
└───────────────┬───────────────────────────────────────┬────────────────────┘
                │ Managed ACP source                    │ Adapter source
                ▼                                      ▼
┌──────────────────────────┐              ┌───────────────────────────────┐
│ rambledesk-core (v3)     │              │ Adapter Workbench Source     │
│ durable v3 facts + work  │              │ projection + command routing │
└───────────┬──────────────┘              └──────────────┬────────────────┘
            │                                            ▼
    ┌───────┴────────┐                     ┌───────────────────────────────┐
    ▼                ▼                     │ maintained Adapter Runtime    │
┌──────────┐  ┌──────────────┐             │ MCP/local-server/hosts/Pi     │
│ v3 SQLite│  │Artifact Store│             └───────────────────────────────┘
└──────────┘  └──────────────┘
            ▲
            │ durable intents / observations
            ▼
┌─────────────────────────────────────────┐
│ rambledesk-acp-client                   │── ACP stdio / JSON-RPC ──▶ Agent
│ preflight / run / recover / live input  │
└─────────────────────────────────────────┘
```

`apps/desktop` 是唯一装配根。它分别创建 Managed ACP Workbench Source 与 Adapter Workbench Source，再把 source-tagged Unified Workbench Projection 交给 UI。UI 不直接调用 ACP wire method、Adapter route 或数据库；选择与命令必须携带 `workbench_session_ref`，不能先试一个 Core、失败后再试另一个。

## 深 Module 与 Interface

### `rambledesk-core`（v3）

`rambledesk-core` 隐藏领域状态机、幂等、原子提交、Package manifest、Draft CAS 和待执行 Agent intent。其外部 Interface 按行为分组，但仍属于一个领域 Module：

```text
launch(LaunchSubmission) -> LaunchOutcome
steer(SteeringSubmission) -> SteeringOutcome
request_feedback(CreateFeedbackRequest) -> FeedbackRequestSnapshot
resolve_feedback(ResolveFeedbackRequest) -> FeedbackResolutionOutcome
mutate_draft(DraftMutation) -> DraftSnapshot
get_feedback(GetFeedback) -> FeedbackDeliveryEnvelope
read_workbench(WorkbenchQuery) -> WorkbenchSnapshot
read_session_recovery(session_id) -> SessionRecoverySnapshot
claim_agent_work(WorkScope) -> AgentWorkBatch
record_agent_observation(AgentObservation) -> AgentObservationOutcome
record_agent_work(AgentWorkResult) -> AgentWorkRecordOutcome
```

Interface 的关键不变量：

- 每个正式 Ramble 都由调用方提供稳定 `submission_id`；Core 从完整 command 重新计算 `submission_digest`，调用方 digest 只能作为可选 assertion，不能要求调用方复制 canonical 算法。
- 相同 id + 相同 digest 返回同一组稳定 identity，并带当前 Agent work / Delivery 投影；相同 id + 不同 digest 返回 conflict。
- Feedback Request 只有 `waiting` 非终态；编辑 Draft 不触发状态迁移。
- 提交 Feedback 在一个本地事务中固定 Submission、Package、`submitted` resolution、pending Delivery 和所需 Agent work。
- 取消 Feedback 在一个本地事务中固定 `cancelled` resolution、pending Delivery 和所需 Agent work，不创建 Package。
- Agent work 被 claim 不等于完成；只有可证明的 Agent 观察结果才能 complete。
- retryable Agent work failure 会释放 lease、保留 attempt 与最近错误，并保持对应 Delivery pending。
- ACP Session Link 在 `session/new/load/resume` 成功后立即通过 Agent observation 固定，不能等 Prompt work 完成后才保存。
- 不确定 side effect 必须以原 work id 重试，不能生成替代事实。
- 该 Interface 只拥有 Managed ACP Session 与 Imported Session；它不读取 Adapter store，也不提供 Adapter Runtime 的 fallback command。

`core` 不得持有：

- ACP、MCP、HTTP、Tauri 或厂商命令细节；
- 进程、stdio、JSON-RPC request id 或 tool timeout；
- Agent transcript、branch、worktree 或 checkout 生命周期；
- 绝对本地 Package 路径。

持久化 port 位于 `core` 的内部 Seam，收敛为 `apply(commit)`、`query(query)`、`claim_work(claim)` 与 `record_work(result)` 四个行为，不复刻行级 CRUD。生产使用 SQLite Adapter；测试使用 in-memory Adapter。Artifact 内容使用独立 port，生产先实现本地 content-addressed Adapter，测试使用内存 Adapter。调用方只看 `core` Interface，不跨过 Seam 测试内部表。

`SessionRecoverySnapshot` 只包含 RambleDesk 自己的 durable context：Session、current ACP Session Link、原 Launch Ramble、按顺序的 Steering Ramble、成对的 terminal Feedback Request + pending Delivery，以及 pending Agent work。它不复制 Agent transcript；`session/new` fallback 用它构建明确的 Recovery Prompt。

### `rambledesk-acp-client`

`rambledesk-acp-client` 隐藏受管 Agent 的复杂实现。其外部 Interface：

```text
preflight(LaunchProfileRef) -> PreflightReport
reconcile(SessionScope) -> ManagedSessionSnapshot
answer_permission(PermissionAnswer) -> LiveAnswerOutcome
answer_question(QuestionAnswer) -> LiveAnswerOutcome
cancel_turn(session_id) -> CancelOutcome
shutdown() -> ShutdownOutcome
subscribe(session_id) -> LiveSessionEvent stream
```

`reconcile` 是主要深度来源。调用方只说明要对账哪个 Session；Implementation 自己完成：

- 读取 Launch Configuration 与 pending Agent work；
- 启动 Agent 子进程并拥有完整进程树；
- ACP initialize 与 capability negotiation；
- 连接 Session Toolset；
- `session/resume`、`session/load`、`session/new` 的选择与降级；
- Prompt 发送、live update 归一化、stop reason 处理；
- Permission Request 的 JSON-RPC 关联与取消；
- Ask Question 的 Question Channel 映射；
- pending Feedback Delivery 的至少一次重试；
- 只有在观察到安全完成条件后回写 Agent work。

ACP Client 的内部 Seam 可以使用进程 Adapter、ACP transport Adapter 与时钟 Adapter 做确定性测试；这些内部 Seam 不暴露给 desktop。

首个 Implementation 只支持 Codex Launch Profile。Claude 进入后，若启动或能力行为确实变化，再抽取 Agent-specific Adapter；在只有一个生产实现时不提前建立空洞的厂商抽象。

### `rambledesk-storage`

`rambledesk-storage` 实现 `core` 的持久化 port：

- 新 v3 SQLite schema、事务和索引；
- Submission、Session、Feedback Request、Package、Delivery 与 Agent work；
- 结构化 Ramble Draft 与附件 metadata；
- claim lease、失败记录与崩溃后的重新 claim；
- 数据一致性检查与只读诊断。

它不发布 ACP event，不解释 Access Mode，也不保存完整 transcript。

### Adapter Workbench Source

Adapter Workbench Source 是 Desktop 与 Adapter Runtime 之间的隔离 Seam；名称刻意与面向 Agent 的 Host Adapter 区分。它只暴露 Workbench 所需的最小能力：

```text
snapshot() -> source-tagged Session / Attention projection
execute(AdapterWorkbenchCommand) -> AdapterCommandOutcome
subscribe() -> adapter projection invalidation stream
```

Source 内部可以调用现有 Adapter controllers、local server 或原生 Adapter，但输出必须带稳定 adapter source identity；Desktop 不接触 `host_id`、Adapter row、token path 或 continuation payload。保存、提交、取消与继续原操作由同一个 Source 路由回原 owner。它不调用 `rambledesk-core` 写影子 Session，不把既有结果转换为 v3 Delivery，也不为 Adapter 路径实现 ACP Permission、Ask 或 runtime config。

Adapter Runtime 进入维护冻结：保留可达性与原行为，允许修复安全、数据完整性和阻断性兼容问题；新主动管理能力只进入 Managed ACP Session。未来是否缩小 Adapter 路径以能力成熟度、迁移质量与真实使用证据决定，不设预定删除阶段。

### Artifact Store

Artifact Store 的 Interface 只表达不可变 bytes：

```text
put(bytes, expected_integrity) -> StoredBlob
open_verified(stored_blob) -> verified byte stream
```

`StoredBlob` 只包含 opaque `storage_key`、size 与 digest。`artifact_id` 是 Package 或 Request 内的具名身份，不是全局 blob key；media type 与 display name 属于 Artifact Entry metadata。首个本地 Adapter 可以把 bytes 写入应用数据目录，但绝对路径只存在于 Adapter 的 Implementation，不能成为 Package identity 或 Agent contract。

Locator 也不由 Artifact Store 生成。Session Toolset 根据 `(package_id, artifact_id)` 建立一次交付引用，Core 再把它解析到内部 `storage_key`；locator 变化不改变任何 Package digest。

未来加入远程对象 Adapter 时，`core` 与 ACP Client Interface 不变。

### Desktop 与 Workbench

Tauri Implementation 负责：

- 装配各 Module；
- 从 Managed ACP Workbench Source 与 Adapter Workbench Source 读取 source-tagged snapshot，构建 Unified Workbench Projection；
- 按 `workbench_session_ref` 把选择、保存、提交、取消与 live 回答路由回唯一 owner；
- App/window/tray 生命周期；
- 文件选择、通知、全局快捷键、截图与录音；
- 把 Core outcome 与 ACP live stream 投影给 Svelte。

Svelte Implementation 负责：

- Launch、Sessions、Context View、Inbox 与 Ramble Workspace；
- 唯一可编辑的结构化 Editor；
- Permission 与 Ask Question 的 live 回答 UI；
- waiting Feedback、Draft 和 Delivery 状态的持久投影。
- 只按 projection 声明的 capability 展示动作，不从 Agent logo、空字段或在线状态推断 Session Source。

UI 内存、Tauri event 和系统通知都不是事实来源。重新聚焦窗口或丢失 event 后，UI 必须分别刷新两个 source snapshot 并重新构建 Unified Workbench Projection；一个 source 暂时失败不能清空另一个 source 的可用事实。

## Feedback Draft 所有权

v3 继续遵守 [ADR 004](adr/004-single-editor-structured-draft.md)：

- Workbench 最多一个可编辑 Rich Editor；
- `document_json` 是结构化真源，Markdown 是同一文档的派生投影；
- 后台 Active Ramble 通过 JSON transformation、串行队列与 CAS 写入；
- Action Group、speech segment、Tidy 与 Cooking 保持既有结构化合同；
- Draft 必须显式记录 Ramble Intent 与目标 Session；Feedback Draft 还记录目标 Request；
- 打开或编辑 Draft 不改变 Feedback Request 的 `waiting` 状态。

## Managed ACP v3 数据模型

以下逻辑表只属于 v3 Core，不是 Unified Workbench Projection 的共享 schema，也不要求 Adapter Runtime 迁入。字段不必拆成完全相同的物理列；但身份、唯一性和事务关系必须保持。

### `sessions_v3`

```text
session_id                 primary key
session_kind               managed | imported
title
lifecycle                  ready | stopped | failed
launch_configuration_json  nullable for Imported Session
created_at / updated_at
```

Launch Configuration 是创建时快照。`running` 来自当前 Agent Run live overlay，Waiting for Feedback 由未解决 Request 派生；二者都不写成可能在崩溃后陈旧的 Session lifecycle。Session 的 live model/mode/config 也只存在于当前 Agent Run 投影，不反写成“历史真相”。

### `acp_session_links_v3`

```text
link_id                    primary key
session_id                 foreign key
agent_profile_id
launch_profile_id
acp_session_id             opaque Agent identity
capabilities_json
session_toolset_digest
is_current
created_at / last_used_at
```

一个 RambleDesk Session 可以先后关联多个 ACP Session；最多一个 link 为 current。仅保存 `acp_session_id` 不够，恢复还依赖 Workspace Reference、Launch Configuration、toolset 与 capability context。

### `ramble_submissions_v3`

```text
submission_id              primary key, client generated
session_id                 foreign key
intent                     launch | steering | feedback
request_id                 nullable, required for feedback
document_json
body_markdown               人类原始正文；Feedback 正式正文从 Package 读取
submission_digest
created_at
```

唯一约束确保一个 Launch Submission 只能创建一个 Session，一个 Feedback Submission 只能解决它绑定的 Request 一次。

### `ramble_drafts_v3`

```text
draft_id                    primary key
intent                     launch | steering | feedback
session_id                 nullable for pre-launch Draft
request_id                 nullable, required for feedback
launch_configuration_json  required only for launch
document_json
body_markdown
revision
created_at / updated_at
```

Draft attachments 使用独立 metadata 表并指向 Artifact Store 中的临时对象；正式提交后发布为 Package Artifact Entry。

### `feedback_requests_v3`

```text
request_id                 primary key
session_id                 foreign key
source_link_id             nullable ACP Session Link provenance
title / instructions
input_digest
resolution                 null | submitted | cancelled
response_package_id        nullable, required when submitted
cancel_reason              nullable
created_at / resolved_at
```

`resolution is null` 就是 `waiting`。不存在 `in_progress`、`completed`、`approved`、`allow_finish` 或 `final_summary`。

### `packages_v3` 与 `package_artifacts_v3`

```text
package_id                 primary key
submission_id              unique foreign key
package_purpose            launch | response
request_id                 nullable, required for response
manifest_json
content_digest              位置无关语义内容 digest
manifest_digest             完整 canonical manifest digest
published_at

artifact_id                package-scoped stable id
package_id                 foreign key
position / display_name
role                       feedback | uncooked | attachment | ...
media_type / size_bytes / sha256
storage_key
```

`submission_digest`、Package `content_digest` 与 `manifest_digest` 是三个独立事实：第一个保护提交幂等，第二个描述与位置无关的 Package 语义，第三个保护完整 manifest。manifest 不保存 `directory_path`、`markdown_path` 或 `manifest_path`。Artifact Locator 在具体 Delivery 构建时产生，不写入 Package identity。

### `artifact_objects_v3`、`feedback_request_artifacts_v3` 与 `draft_artifacts_v3`

```text
storage_key                primary key, opaque content-addressed key
size_bytes / sha256
created_at

owner request_id|submission_id|draft_id
artifact_id / position / display_name
media_type / size_bytes / sha256 / storage_key
```

Agent 随 Feedback Request 提供的证据属于 Request，不得塞进人类 Draft；它通过 `feedback_request_artifacts_v3` 关联。Draft Artifact 与正式 Package Artifact 可以引用同一 blob，只改变数据库引用，不移动或改写 bytes。

### `feedback_deliveries_v3`

```text
delivery_id                primary key
request_id                 unique foreign key
session_id                 foreign key
resolution                 submitted | cancelled
package_id                 nullable, required when submitted
state                      pending | delivered
attempt_count
last_error_code
created_at / delivered_at
```

人类 resolution 与 Delivery state 分离。Delivery 失败不会把 Request 退回 `waiting`。

### `agent_work_v3`

这是持久 outbox 的 Implementation 记录，不是新的产品对象：

```text
work_id                    primary key
session_id                 foreign key
kind                       launch_prompt | steering_prompt | feedback_resume
source_id                  unique submission_id or delivery_id
payload_digest
state                      pending | claimed | completed
lease_until / attempt_count / last_error_code
created_at / completed_at
```

Launch 与 Steering 使用 Submission 作为稳定 source；Feedback Resume 使用 Delivery。崩溃或 lease 过期后以同一 `work_id` 重试。

## Attention Item 投影

```text
Core Feedback Request history ─┐
ACP pending Permission Request ├─> Attention Item read model ─> Request list
Question Channel pending Ask ──┤                              └─ waiting subset: Inbox
Adapter attention/history ─────┘
```

- Managed ACP Feedback Request 全部来自 v3 SQLite，App 重启后恢复；只有 waiting 项计入待处理 Inbox，terminal 项保留为结构化请求历史。
- Permission 与 Ask Question 来自当前 Agent Run 内存，断开即取消。
- Adapter attention 来自原 owner 的 snapshot；其恢复与操作语义不由 v3 Core 重解释。
- Attention Item 没有统一持久表，也不强迫三类请求共享状态机。
- 已提交但 pending 的 Delivery 属于 Session 状态，不进入未处理 Inbox；对应 terminal Feedback Request 仍留在请求历史并可重开结果。
- projection key 必须包含 Session Source；同 id 或同标题不能触发跨 source 去重。只有迁移 manifest 的显式 source mapping 可以抑制重复展示。

## Managed ACP Agent Run 与历史

ACP Client 为每个 active Managed ACP Session 保存内存态：子进程、连接、protocol request correlation、当前 config、Prompt Turn、Permission、Question、tool call 与 live events。

这些数据不建立历史表。恢复规则：

1. 有 current ACP Session Link 且 Agent 支持 resume：`session/resume`，不等待历史 replay。
2. 否则 Agent 支持 load：`session/load`，把 replay 事件送到当前 Context View，但不落 transcript。
3. 否则：`session/new`，保留原 RambleDesk Session，写入新的 ACP Session Link，并发送包含恢复上下文的 Prompt。

Agent 返回的 Session config options 优先于旧 modes 字段。Preflight 只用于 Managed ACP Launch UI，不创建 Session 或历史；真正建立/恢复 Session 后，返回的 config 才是 live 真源。Adapter Session 不显示伪造的 ACP runtime 状态或配置。

## Feedback Delivery 对账

```text
Human resolves Feedback Request
  → Core atomically stores resolution + optional Package + pending Delivery + work
  → ACP Client reconcile(session_id)
  → reuse healthy Run, or initialize a new Run
  → resume, else load, else new + Recovery Prompt
  → send Feedback Resume Prompt with request_id and delivery_id
  → Agent calls get_feedback(request_id)
  → Agent consumes stable Delivery Envelope
  → normal Prompt Turn completion proves handoff
  → Core marks work completed and Delivery delivered
```

如果 ACP connection、tool response 或 Prompt Turn 结果不确定，Delivery 保持 `pending`。下次对账继续使用同一 `delivery_id`。Agent 侧被要求按 `delivery_id` 去重；系统提供至少一次，不声称跨进程 exactly-once。

## 进程与 App 生命周期

- `rambledesk-acp-client` 启动 Agent 时建立独立进程组或等价进程树所有权。
- 关闭窗口和进入托盘不触发 `shutdown`。
- 取消 Prompt Turn 优先发送 ACP `session/cancel`；若不生效，不立即杀死整个 Session Run。
- 停止 Session 先尝试 ACP `session/close`（若支持），再终止该 Session 进程树。
- 完整退出 App 对全部 Agent Run 执行有界优雅关闭，随后清理各自进程树。
- 下次 App 启动只扫描 pending Agent work 与 Delivery；不会为了纯 `waiting` Feedback Request 主动启动 Agent。
- 上述进程所有权只覆盖 Managed ACP Agent Run；Adapter Runtime 的外部 Agent 与 continuation 继续由原路径拥有。

## 共存边界

以下既有 Implementation 组成维护冻结的 Adapter Runtime：

- `rambledesk-mcp` 的 Generic MCP tool surface 与安装引擎；
- `rambledesk-local-server` 的旧 `/api/feedback/*` 与 `/mcp` 路由；
- `rambledesk-hosts` 的 host profile 与 continuation strategy；
- `packages/pi-rambledesk` 的长等待流程；
- Resume Prompt UI 与原 continuation。

它们不追赶 Managed ACP 的 Launch、Permission、Ask、runtime 状态/配置或 Managed Feedback Resume，也不写 v3 新表；Desktop 只能经 Adapter Workbench Source 访问它们。它们保持可达，供既有数据直接显示并继续原操作。修复范围默认限于安全、数据完整性和阻断性兼容；未来缩小或替换必须有逐能力验收、迁移证据和新的决策记录，不能只由新能力已经存在自动推出。

两个 source 在 Desktop 投影层汇合，不在 persistence 或 use case 层汇合。禁止：

- 同一用户动作向 Adapter Runtime 与 v3 Core 双写；
- v3 command 失败后静默调用旧 command，或反向 fallback；
- 用含大量 nullable 字段的 DTO 把两套命令伪装成同一状态机；
- 通过标题、workspace 或裸 id 猜 source；
- 因 Adapter source 暂时不可用而隐藏或删除其持久事实。

## 测试策略

- `core`：通过公开 Interface + in-memory persistence/Artifact Adapter 测试状态、幂等、冲突、事务和重启恢复。
- `storage`：对真实临时 SQLite 与本地 Artifact Adapter 做 schema、事务、崩溃恢复和 digest 测试。
- `rambledesk-acp-client`：用可编程 fake ACP Agent 覆盖 initialize、config、permission、elicitation、prompt、resume/load/new、断线与重复 Delivery；不通过私有函数测试 wire 细节。
- Codex smoke：只证明真实 Codex Launch Profile 与能力映射，不替代 fake Agent 的故障矩阵。
- Adapter Workbench Source：用 Adapter Runtime fixture 验证 Adapter Session/Request 可直接投影、原保存/提交/取消/continuation 仍路由到原 owner，且 v3 store 零写入。
- Desktop：同时验证 Managed ACP 的 Launch → Permission → Feedback → App restart → Delivery 竖切，以及两 source Session/Inbox 合并、身份碰撞、source 局部失败与命令路由。
- 旧路径既有测试保留在维护边界内；不要求复制为 v3 Interface 测试，也不因新路径覆盖相似 UI 就删除原行为保护。

## 外部协议依据

- [ACP Session Setup](https://agentclientprotocol.com/protocol/v1/session-setup)：`session/new`、`session/load`、`session/resume` 都携带 cwd 与 MCP server 配置；load replay 历史，resume 不 replay。
- [ACP Tool Calls](https://agentclientprotocol.com/protocol/v1/tool-calls)：Permission Request 是 Agent 发向 Client 的 live JSON-RPC request，Prompt Turn 取消时 Client 必须回 `cancelled` outcome。
- [ACP Elicitation](https://agentclientprotocol.com/protocol/v1/elicitation)：原生 form mode 可以承载 Ask Question；这是 transport 映射，不把 “Elicitation Request” 引入产品术语。
- [ACP Session Config Options](https://agentclientprotocol.com/protocol/v1/session-config-options)：Agent 返回实际可用的配置和值；Workbench 应优先使用它而不是即将淘汰的 modes 字段。
