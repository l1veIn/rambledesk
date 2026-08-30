# RambleDesk v3 ACP-first 实施计划

> 状态：Phase 1 已完成，Phase 2 待启动。当前 Desktop 仍运行旧装配，尚不能作为 ACP-first 产品验收。
> 领域术语以 [TERMINOLOGY.md](TERMINOLOGY.md) 为唯一来源；产品、Module 和协议合同分别见 [PRODUCT.md](PRODUCT.md)、[ARCHITECTURE.md](ARCHITECTURE.md)、[PROTOCOL.md](PROTOCOL.md)。

## 当前路线图

| 阶段 | 状态 | 收敛结果 |
| --- | --- | --- |
| Phase 0：可执行设计基线 | 已完成 | 术语、产品承诺、Module Interface、协议、数据模型与删除清单已锁定。 |
| Phase 1：新领域事实与新数据 | 已完成 | Core、SQLite v3、Artifact Store 与一次性有损迁移已形成可独立验证的闭环。 |
| Phase 2：ACP Client 与 Codex | 下一步 | 先用 fake ACP Agent 验证 Interface 与故障矩阵，再完成真实 Codex smoke。 |
| Phase 3：Session Toolset 与 Feedback Recovery | 未开始 | 闭合可无限等待、离线提交、恢复后读取的 Feedback Delivery。 |
| Phase 4：Workbench 重构 | 未开始 | Desktop 切换到 Session-first UI，并完成一次集中设计 Gate 与原生验收。 |
| Phase 5：旧路径删除与零残留 | 未开始 | 删除旧 MCP/Pi/host continuation 与旧装配，完成全树残留审计。 |

最终收敛点不是“ACP 可以启动”，而是：真实 Codex 的 Launch、Permission、Ask Question 与无限等待 Feedback 全部通过 Desktop 端到端验收；离线提交和跨重启 Delivery 可恢复；运行时只使用 v3 数据与位置无关 Package；旧 MCP/Pi/host continuation 不再进入装配，旧业务概念在迁移器以外零残留。

## 已锁定决策

1. 删除 Feedback 的旧批准路径。即时“是否可以结束”属于 Ask Question；需要真实审阅和持久等待时使用 Feedback Request。
2. 首个真实 Agent 是 Codex ACP；Claude 在首条闭环稳定后加入。
3. 现有 Generic MCP、Pi 与原生 Adapter 全部冻结，不进入 v3 首发验收，也不做新旧双模型兼容。
4. 当前运行时只支持新表。旧数据由独立脚本做快速、有报告的有损迁移。
5. Agent transcript 由 Agent 自己保存；RambleDesk 不复制完整历史。
6. Feedback Request 可以无限期 `waiting`；人类提交与 Agent Delivery 分成两个事实。

## 交付原则

- 自上而下：术语 → 产品 → Module Interface → 协议 → 数据 → Implementation → UI。
- 每个阶段只对当前新模型写入；不增加 legacy alias、fallback read 或双写。
- 每个 side effect 都从持久事实或持久 outbox 对账。
- 先完成一条真实竖切，再扩展 Agent、历史展示或 Compatibility Ingress。
- 新 Interface 测试覆盖旧行为后，删除旧浅测试；不长期维护两套测试面。
- 每阶段结束扫描代码、文档、UI、测试、fixture 与配置中的旧概念。

## Phase 0：可执行设计基线

本阶段产物：

- [x] v3 术语与架构公理。
- [x] v3 产品承诺、非目标、首发范围与端到端验收故事。
- [x] `core` 与 `rambledesk-acp-client` 深 Module Interface 设计；ACP Client crate Implementation 尚未开始。
- [x] ACP、Permission、Ask Question、Feedback、Package 与 Recovery 协议。
- [x] 新逻辑数据模型。
- [x] 有损迁移策略与旧概念删除清单。
- [x] 用户接受 Launch / Sessions / Inbox / Ramble Workspace 先按建议方案推进，完整 UI 实现前再集中验收。
- [x] 默认 Access Mode 采用 `workspace_write`。

Phase 0 完成门槛：文档内部无 v2 合同冲突，术语检查通过，后续代码任务可以直接从 Interface 和验收故事拆分。

## Phase 1：新领域事实与新数据

目标：在不启动 Agent 的情况下，完整证明 v3 事实、幂等、Package 与迁移。

### Core

- 用 Session 顶层聚合替换 request-first use case。
- 建立 Launch、Steering、Feedback 三种 Submission command。
- 删除 `allow_finish`、`final_summary`、Approve operation 与 `approved` resolution。
- Feedback 状态只保留 `waiting` / `submitted` / `cancelled` 投影。
- 建立位置无关 Package manifest 与 Artifact Entry。
- 建立 Feedback Delivery 与 Agent work outbox。
- 保留结构化 Draft、Action Group、Tidy 与 Cooking 合同。

### Storage

- 创建全部 `_v3` 新表与索引；不改写旧表供运行时使用。
- 实现 Core persistence port 与 Artifact Store 本地 Adapter。
- 实现 Submission/Request/Delivery 的唯一约束和事务。
- 实现 work claim lease、失败重试与启动对账。
- 提供只读 consistency report。

### 验收

- [x] 相同 Launch Submission 重试只生成一个 Session、Package 和 work。
- [x] 相同 id + 不同 digest 返回 conflict。
- [x] Feedback 提交在 Agent 不存在时仍固定 Package、resolution 与 pending Delivery。
- [x] App 进程重启后 waiting Request、Draft 和 pending Delivery 可恢复。
- [x] manifest 与数据库中不存在作为协议字段的绝对 Package 路径。
- [x] 迁移器完成 `inspect / dry-run / execute / verify`、只读 backup、损失报告与失败安全验证。

## Phase 2：`rambledesk-acp-client` 与 Codex

目标：完成受管 Codex Session 的 Launch、live event、Permission 与基础恢复。

### Module

- 新建 `crates/rambledesk-acp-client`。
- 实现 Codex Launch Profile 与短生命 Preflight。
- 实现进程树所有权、initialize、capability snapshot 和 bounded shutdown。
- 实现 `session/new` 与 ACP Session Link。
- 实现 live `session/update` 到 Context View 的归一化，不落 transcript。
- 实现 `session/request_permission` 的多请求关联、排队、回答与取消。
- 优先实现 ACP `elicitation/create` form 到 Ask Question 的映射。
- 实现 resume → load → new Recovery 的 reconcile。

### 测试

- 可编程 fake ACP Agent 覆盖正常、拒绝、断线、乱序、多 Permission、Prompt cancel 与 capability 缺失。
- 真实 Codex smoke 证明 Launch Profile、config options、Permission 和至少一种恢复能力。
- 测试只跨 ACP Client Interface，不要求 desktop 编排 wire method。

### 验收

- 同一 Session 可先后拥有多个 Agent Run，但只有一个 current ACP Session Link。
- 退出 App 杀净受管进程树，再启动可恢复 Session。
- `session/load` replay 只显示在当前 Context View，不生成 transcript 表。
- Permission/Ask Question 断线后从 Inbox 消失并回到正确 cancel/error 结果。

## Phase 3：Session Toolset 与 Feedback Recovery

目标：完成首条最关键的“Agent 请求反馈 → 人类晚些提交 → Agent 恢复读取”闭环。

### Session Toolset

- 实现短调用 `request_feedback`。
- 实现位置无关 `get_feedback(request_id)`。
- 每个 Agent Run 在 new/load/resume 时重新注入相同 toolset 配置。
- 记录 toolset digest，能力不满足时明确失败或降级。

### Recovery

- 人类提交后立即显示本地 `submitted`，不先检查 client 或 Agent 是否在线。
- ACP Client 对账 pending `feedback_resume` work。
- 健康 Run 复用；否则 resume、load、new 逐级恢复。
- new fallback 发送包含原任务上下文的 Recovery Prompt。
- Prompt 要求 Agent 调用 `get_feedback(request_id)` 并按 `delivery_id` 去重。
- 只有观察到 tool call 成功且 turn 正常结束才标记 Delivery `delivered`。

### 故障矩阵

- App 在 Request 创建后退出。
- App 在人类提交事务后、启动 Agent 前退出。
- Agent 在 resume 后、Prompt 前退出。
- `get_feedback` response 结果不确定。
- Agent 已消费 Delivery，但客户端在 turn completion 前退出。
- resume 不支持、load 不支持、旧 ACP Session 不存在。
- Package Artifact 临时不可读取。

所有不确定情形都保持同一个 `delivery_id`，不得回滚人类 resolution 或产生第二个 Package。

## Phase 4：Workbench 重构

目标：从 request-first 页面转为 Session-first human-agent workbench。

### Launch

- Agent / Launch Profile 选择。
- Workspace Reference。
- Agent 返回的 model、reasoning、config options。
- Access Mode：Read Only、Workspace Write、YOLO。
- Launch Ramble Draft 与幂等提交反馈。

### Sessions / Context View

- Session 状态、当前 Agent Run 与 live config。
- 当前 live activity、tool call 与 usage。
- Ask Question 固定显示在输入区上方。
- Steering Ramble、cancel turn、恢复/失败状态。
- Agent load replay 可显示但明确不承诺永久历史。

### Inbox

- Permission Request：live、可多项排队。
- Ask Question：live、回答或跳过。
- Feedback Request：durable、跨重启恢复。
- 提交但 pending Delivery 从 Inbox 移到 Session 交付状态。

### Ramble Workspace

- 继续使用单 Rich Editor 与结构化 Draft。
- Launch、Steering、Feedback intent 明确可见，不靠页面位置推断。
- Feedback 保留 Uncooked、可选 Cooking 与 Package 预览。

### 用户验收 Gate

进入完整 UI Implementation 前只需要一次集中讨论：

1. Launch / Sessions / Inbox / Ramble Workspace 的 wireframe 与信息密度。
2. Permission 与 Ask Question 同时存在时的 Inbox 排序和 Session 输入区优先级。

默认 Access Mode 已锁定为 `workspace_write`，不在该 Gate 重复讨论。

其余字段、状态、恢复和错误文案由已锁定合同直接实现，不再逐项打断用户。

## Phase 5：删除旧运行路径与零残留

目标：v3 分支不再携带会被误装配或误维护的 v2 业务实现。

### 删除或替换

| 旧概念或 Implementation | v3 处理 |
| --- | --- |
| `FeedbackStatus::InProgress` | 删除；Draft 编辑不改变 Request 状态。 |
| `FeedbackStatus::Completed` | 替换为 `submitted` resolution。 |
| `FeedbackResolution::Approved` | 删除；即时选择使用 Ask Question。 |
| `ApproveFeedbackInput` / approve command | 删除。 |
| `allow_finish` / `final_summary` | 删除全部字段、UI、fixture 与测试。 |
| `host_id` / `host_session_id` 业务身份 | 删除；Session 使用 `session_id`，ACP 使用 opaque link。 |
| `HostSessionSummary` / Host rail | 替换为 Session list 与 Agent Profile 展示。 |
| `package_uri` / `directory_path` / `markdown_path` / `manifest_path` | 替换为 Package identity、Artifact Entry 与 Locator。 |
| `ResumePromptDialog` / Managed 路径手动 continuation | 删除；由 Feedback Delivery reconcile 替代。 |
| v2 `/api/feedback/wait` | 删除，不复制到 v3。 |
| v2 `/api/feedback/approve` | 删除。 |
| Pi 同 tool-call wait | 冻结后删除出首发 workspace；未来重新设计。 |
| `rambledesk-hosts` continuation strategy | 删除；Launch Profile 知识进入 ACP Client Implementation。 |
| `rambledesk-mcp` 旧工具与安装引擎 | 从 v3 装配删除；未来以薄 Compatibility Ingress 重写。 |
| `rambledesk-local-server` v2 feedback/MCP routes | 删除；只有重写为新 Core Interface 的认证 IPC/薄 Ingress 才能保留。 |
| `rambledesk-cli` 旧 MCP host、自检与安装命令 | 从首发装配和 release gate 删除或按新 Interface 重写。 |
| `packages/dsh-rambledesk` 与旧 DSH installer | 删除出首发 workspace；若未来重做，按独立 Compatibility Ingress 重新验收。 |
| 旧 MCP/Pi/DSH 安装脚本、fixture 与 release check | 随对应 Implementation 删除，不允许继续让旧路径看似受支持。 |
| 完整 Turn / Timeline 持久化设想 | 不实现；Agent history 是权威来源。 |

### 残留扫描范围

- Rust/TypeScript/Svelte source。
- SQLite migrations、query、fixture 和 recovery code。
- Tauri commands/events 与 generated bindings。
- UI label、empty state、settings 与 onboarding。
- tests、snapshots、examples、scripts、docs 与 release notes。
- Cargo workspace、package scripts 与 installer assets。

扫描命中必须逐项解释：删除、重命名、或证明是协议/迁移器中的有意 legacy source。只检查文档不构成零残留。

## 一次性有损迁移

迁移器位于 `tools/migrate-v2-to-v3`，是 root workspace 之外的独立 Rust binary。它是显式用户动作，不在 v3 App 启动时自动运行。

### 安全流程

1. `inspect`：只读旧库，输出分类与预计损失。
2. `migrate --dry-run`：验证新对象、Artifact 可读性与唯一性，不写目标库。
3. 复制旧数据库，以及旧 manifest 明确引用或旧表逐条记录的 feedback/draft 文件，作为只读 backup；不得递归复制数据库给出的任意目录。
4. `migrate --execute`：只写全新的 v3 数据库与 Artifact Store。
5. `verify`：检查新外键、digest、Package manifest 与计数。
6. 输出机器可读 JSON 和人类可读 Markdown 报告。

执行失败不得修改旧库，也不得让 App 回退读取旧表。

### 映射策略

| v2 数据 | v3 结果 |
| --- | --- |
| `waiting` Request | 创建 Connected Session + waiting Feedback Request；保留原 `request_id`。 |
| `in_progress` Request | 有损映射为 waiting；编辑状态丢弃，Draft 结构保留。 |
| 对应结构化 Draft | 保留 `document_json`、Markdown projection、revision 与可读附件。 |
| completed + 可读且一致的 manifest | 创建 submitted Request、Feedback Submission、response Package 与 `delivered` Delivery；不创建 pending Agent work。 |
| completed 但 Package 不可读/不一致 | 丢弃该 Request，报告 `completed_package_unreadable`。 |
| cancelled Request | 默认丢弃，计入报告；不恢复成 Inbox。 |
| approved / `allow_finish` Request | 丢弃，报告 `unsupported_approval_semantics`。 |
| orphan Draft | 丢弃，报告 `orphan_draft`。 |
| 路径附件可读且满足资源上限 | 导入 Artifact Store，重新计算 digest。 |
| 路径附件不可读 | 丢弃附件；若它使 submitted Package 不完整，则丢弃整个 completed Request。 |
| 旧 `host_id` / `host_session_id` | 只用于把请求分组为 Connected Session，并写迁移来源 metadata；不进入新业务 Interface。 |

迁移完成的历史 submitted Request 是只读事实，其 Delivery 直接标记为 `delivered`，因为迁移器不会重新唤醒或推断原 Agent。迁移后的 waiting Request 可以由未来 Compatibility Ingress 读取；v3 首发只保证人类仍能查看和编辑其 Draft，不开放提交或取消，也不自动唤醒旧 Agent。对 Connected Session 调用当前 Feedback resolution Interface 返回 `SESSION_NOT_MANAGED`；未来 Ingress 必须先定义自己的交付确认 Seam，不能制造永久 pending work。

backup 是可验证的已知文件集合，不是旧目录的递归镜像。manifest 或旧表指向的文件若不可读、不一致、超出资源上限或路径不安全，迁移器逐项记录 loss；不得为了“尽量完整”而遍历或复制目录中的无关文件。

### 报告最小字段

```json
{
  "source_schema": "v2",
  "target_schema": "v3",
  "started_at": "...",
  "finished_at": "...",
  "counts": {
    "sessions_created": 0,
    "waiting_requests_migrated": 0,
    "submitted_requests_migrated": 0,
    "drafts_migrated": 0,
    "artifacts_migrated": 0,
    "records_dropped": 0
  },
  "losses": [
    {
      "legacy_id": "...",
      "reason": "unsupported_approval_semantics"
    }
  ]
}
```

## 分支完成定义

本分支完成不等于“ACP 可以启动”。必须同时满足：

- 首条端到端验收故事通过真实 Codex + 真实桌面 UI。
- Core、Storage、ACP Client 的 Interface 测试覆盖重试与故障矩阵。
- App 退出后没有遗留受管 Agent 进程。
- waiting Feedback 与 Draft 跨重启恢复。
- 人类提交离线成功，pending Delivery 最终可恢复送达。
- 新运行时不读取旧表，不返回绝对 Package path 合同。
- 旧 MCP/Pi/host continuation 未进入 desktop 装配。
- 全树旧概念残留已清零或仅存在于有明确隔离标记的一次性迁移器中。
- 迁移 dry-run、execute、verify 与损失报告通过 fixture。
- 用户完成 Launch / Inbox / Context View 的 native UI 验收。

## 当前下一步

进入 Phase 2：创建 `crates/rambledesk-acp-client`，先锁定最小 Interface 并用可编程 fake ACP Agent 验收 initialize、Launch、Permission、Ask Question、断线、乱序、取消与 bounded shutdown，再做真实 Codex smoke。Desktop、UI 与旧 Adapter 在各自阶段切换，不在 ACP Client 中提前混装；完整 Workbench UI 前保留既定的一次集中用户设计 Gate。
