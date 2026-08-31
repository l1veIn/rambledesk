# RambleDesk v3 Unified Workbench 实施计划

> 状态：Phase 0–2 已完成；Phase 3 Implementation 已完成；Phase 4 已按“只替换最左栏，保留原中栏与原 Workspace”纠正并通过浏览器交互验收。原生 Launch、Permission、无限等待 Feedback、跨重启 Draft 恢复与提交后 Session 恢复已通过；真实 Ask Question 与修复后的 Ramble Console 点按复验仍待完成。Phase 5 转为 Unified Workbench 共存与边界加固；旧路径不再以删除作为完成条件。
> 领域术语以 [TERMINOLOGY.md](TERMINOLOGY.md) 为唯一来源；产品、Module 和协议合同分别见 [PRODUCT.md](PRODUCT.md)、[ARCHITECTURE.md](ARCHITECTURE.md)、[PROTOCOL.md](PROTOCOL.md)。

## 当前路线图

| 阶段 | 状态 | 收敛结果 |
| --- | --- | --- |
| Phase 0：可执行设计基线 | 已完成 | 术语、产品承诺、Module Interface、协议、数据模型与边界清单已锁定。 |
| Phase 1：新领域事实与新数据 | 已完成 | Core、SQLite v3、Artifact Store 与一次性有损迁移已形成可独立验证的闭环。 |
| Phase 2：ACP Client 与 Codex | 已完成 | Fake ACP 故障矩阵与真实 Codex config、Permission、关闭后 resume smoke 已通过。 |
| Phase 3：Session Toolset 与 Feedback Recovery | Implementation 已完成，原生验收中 | 短调用 `request_feedback`、位置无关 `get_feedback`、持久 outbox 与启动/在线对账已接入。 |
| Phase 4：Workbench 重构 | Implementation 已完成，原生验收中 | 仅将最左 Host 栏替换为 Session 栏；原 Request List、Task Brief、Rich Editor、Ramble/附件/Cooking/交付右栏继续作为唯一生产实现；浏览器布局与三类请求切换已通过。 |
| Phase 5：Unified Workbench 共存与边界加固 | 下一步 | 接入 Adapter Session，完成 source-aware 投影/路由、无双写与隔离残留审计。 |

最终收敛点不是“ACP 可以启动”或“Adapter Runtime 被删除”，而是：真实 Codex 的主动 Launch、Permission、Ask Question、runtime 状态/配置与无限等待 Feedback 通过 Desktop 端到端验收；Adapter Session 无需迁移即可显示并继续原操作；Adapter Runtime 与 v3 Core 不双写、不互相 fallback；Desktop 以 Session Source 安全合并两边投影；迁移仍是可选、有报告的有损路径。

## 已锁定决策

1. 删除 Feedback 的旧批准路径。即时“是否可以结束”属于 Ask Question；需要真实审阅和持久等待时使用 Feedback Request。
2. 首个真实 Agent 是 Codex ACP；Claude 在首条闭环稳定后加入。
3. 现有 Generic MCP、Pi、host 与原生 Adapter 作为 Adapter Runtime 维护冻结：保留既有数据与原操作，不追赶 ACP 新能力，也不设预定删除阶段。
4. v3 Core 只读写新表，Adapter Runtime 只读写既有 store；Desktop 合并 source-tagged projection，不做跨 Core 双写或 fallback。旧数据迁移是独立、可选、有报告的有损路径。
5. Agent transcript 由 Agent 自己保存；RambleDesk 不复制完整历史。
6. Feedback Request 可以无限期 `waiting`；人类提交与 Agent Delivery 分成两个事实。

## 交付原则

- 自上而下：术语 → 产品 → Module Interface → 协议 → 数据 → Implementation → UI。
- 每个 source 只写自己的模型；不增加跨 Core alias、fallback read 或双写。
- 每个 side effect 都从持久事实或持久 outbox 对账。
- 先完成 Managed ACP 真实竖切，再接入 Unified Workbench Projection；不以新竖切替代 Adapter Runtime 的既有行为保护。
- 新 Interface 测试覆盖 Managed ACP 行为；Adapter Runtime 保留原路径测试，并增加 source 投影/命令路由与 v3 零写入测试。
- 每阶段结束扫描代码、文档、UI、测试、fixture 与配置中的跨 source 泄漏、双写、fallback 和能力误报。

## Phase 0：可执行设计基线

本阶段产物：

- [x] v3 术语与架构公理。
- [x] v3 产品承诺、非目标、首发范围与端到端验收故事。
- [x] `core` 与 `rambledesk-acp-client` 深 Module Interface 设计；ACP Client crate 已按该 Interface 实现。
- [x] ACP、Permission、Ask Question、Feedback、Package 与 Recovery 协议。
- [x] 新逻辑数据模型。
- [x] 有损迁移策略与领域边界清单。
- [x] 用户接受 Launch / Sessions / Inbox / Ramble Workspace 先按建议方案推进，完整 UI 实现前再集中验收。
- [x] 默认 Access Mode 采用 `workspace_write`。

Phase 0 完成门槛：文档内部无跨数据代际合同冲突，术语检查通过，后续代码任务可以直接从 Interface 和验收故事拆分。

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

- [x] 同一 Session 可先后拥有多个 Agent Run，但只有一个 current ACP Session Link。
- [x] 退出 App 杀净受管进程树，再启动可恢复 Session；真实 Codex smoke 已证明同一 Core Session 的关闭后 resume。
- [x] `session/load` replay 只显示在当前 Context View，不生成 transcript 表。
- [x] Permission/Ask Question 断线后从 Inbox 消失并回到正确 cancel/error 结果。

## Phase 3：Session Toolset 与 Feedback Recovery

目标：完成首条最关键的“Agent 请求反馈 → 人类晚些提交 → Agent 恢复读取”闭环。

### Session Toolset

- 实现短调用 `request_feedback`。
- 实现位置无关 `get_feedback(request_id)`。
- 每个 Agent Run 在 new/load/resume 时重新注入相同 toolset 配置。
- 能力不满足时明确失败；toolset 使用 Run 内随机鉴权地址，不把本地路径写入 Package 合同。

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

目标：从 request-first 页面转为 Session-first human-agent workbench，并为 Managed ACP 与 Adapter 两个 source 保留同一三栏体验。

### Launch

- Agent / Launch Profile 选择。
- Workspace Reference。
- Agent 返回的 model、reasoning、config options。
- Access Mode 只显示 Agent preflight 实际返回的选项；当前 Codex profile 提供 Workspace Write 与 YOLO，不伪装尚未支持的 Read Only。
- Launch Ramble Draft 与幂等提交反馈。

### Sessions / Context View

- 第一栏合并 Managed ACP Session 与 Adapter Session；选择 identity 必须包含 Session Source。
- Managed ACP Session 显示当前 Agent Run 与 live config；Adapter Session 只显示原 owner 能证明的状态与动作。
- 当前 live activity、tool call 与 usage。
- Ask Question 固定显示在输入区上方。
- Steering Ramble、cancel turn、恢复/失败状态。
- Agent load replay 可显示但明确不承诺永久历史。

### Inbox

- Permission Request：live、可多项排队。
- Ask Question：live、回答或跳过。
- Feedback Request：durable、跨重启恢复。
- Adapter attention 与 Managed ACP attention 同列显示，但保存、提交、取消与 continuation 回原 owner。
- 提交但 pending Delivery 从 Inbox 移到 Session 交付状态。

### Ramble Workspace

- 继续使用单 Rich Editor 与结构化 Draft。
- Launch、Steering、Feedback intent 明确可见，不靠页面位置推断。
- Feedback 保留 Uncooked、可选 Cooking 与 Package 预览。

### 用户验收 Gate

- [x] 保留三栏布局；第一栏只放 Sessions 与“启动新 Ramble”，Agent 以 Session 前的 logo 表达。
- [x] 第二栏统一承载 Feedback、Permission 与 Ask Question。
- [x] 第三栏按请求类型切换 Feedback、Permission 与 Ask View。
- [x] ACP Client profile 收敛到设置弹窗；Launch 只承载本次 Session 的配置。
- [x] 默认 Access Mode 锁定为 `workspace_write`，但最终可选项服从 Agent preflight capability。
- [ ] 用原生 App 完成真实 Launch、Permission、Ask Question 与跨等待 Feedback 的完整点按验收。Launch、Permission、Feedback、重启恢复和提交恢复已经通过；Ask Question 与 Ramble Console 桥接修复后的点按复验尚未完成。

## Phase 5：Unified Workbench 共存与边界加固

目标：让两个独立 owner 在一个 Workbench 中自然共存，同时防止 Adapter Runtime 与 v3 Core 混装、双写、fallback 或能力误报。Adapter Runtime 保持可达与维护冻结，不以删除 MCP、Pi、host、local server 或 continuation 作为本阶段验收。

### 实施内容

| 边界 | Phase 5 处理 |
| --- | --- |
| Session list | 从 Managed ACP Workbench Source 与 Adapter Workbench Source 读取 source-tagged snapshot，合并排序；Agent logo 只是展示，不承担 identity。 |
| Session identity | 引入包含 Session Source 与 native id 的选择/命令引用；测试两边裸 id 相同也不串写。 |
| Inbox | 合并 ACP Permission、ACP Ask、Managed Feedback 与 Adapter attention；每项保留 owner 与实际 capability。 |
| Command routing | 保存、提交、取消、Permission/Ask 回答和 continuation 只路由到声明它的 owner；禁止失败后跨 source fallback。 |
| Managed ACP capability | 主动 Launch、runtime 状态/配置、Permission、Ask、Managed Feedback Resume 只在真实 capability 存在时显示。 |
| Adapter capability | 继续原 MCP/Pi/host/native Adapter 操作；不回填 ACP 新能力，不把 Adapter Continuation 改名为 Managed Feedback Resume。 |
| Persistence | v3 Core 只写新表，Adapter Runtime 只写既有 store；同一用户动作不得双写。 |
| Migration | 保留 `inspect / dry-run / execute / verify` 的显式有损路径；直接显示旧数据不依赖迁移，迁移来源 mapping 防止重复展示。 |
| Maintenance freeze | 旧路径允许安全、数据完整性和阻断性兼容修复；功能扩展需单独决策与验收。 |
| History | 不增加统一 transcript；Managed ACP 历史服从 Agent capability，Adapter 历史服从原 owner。 |

### 隔离残留扫描

扫描 Rust/TypeScript/Svelte、SQLite、Tauri bindings、tests/fixtures、docs、workspace 与 installer，逐项确认：

- v3 Core/repository 不引用 Adapter row、`host_id` alias、Adapter Runtime query 或 command fallback；
- Adapter Runtime 不写 `_v3` 表，不依赖 ACP Client DTO；
- Desktop 不直接导航 Adapter controller internals 或 ACP wire payload，只经两个 Workbench Source 汇合；
- UI selection、draft owner 与 Attention command 全部携带 Session Source；
- capability 不由 logo、nullable field 或在线状态猜测；
- 可选迁移有显式 source mapping，不靠标题/workspace/content heuristic 去重；
- Adapter 路径仍有可达入口与既有行为测试，不因 Unified Workbench 接线被意外移除。

扫描的目标是清除越界耦合，不是清除 Adapter Runtime 名称。命中可以是合法的 Adapter owner、迁移输入或隔离测试；需要记录理由与边界，而非一律删除。

## 一次性有损迁移

迁移器位于 `tools/migrate-v2-to-v3`，是 root workspace 之外的独立 Rust binary。它是显式用户动作，不在 v3 App 启动时自动运行。

### 安全流程

1. `inspect`：只读旧库，输出分类与预计损失。
2. `migrate --dry-run`：验证新对象、Artifact 可读性与唯一性，不写目标库。
3. 复制旧数据库，以及旧 manifest 明确引用或旧表逐条记录的 feedback/draft 文件，作为只读 backup；不得递归复制数据库给出的任意目录。
4. `migrate --execute`：只写全新的 v3 数据库与 Artifact Store。
5. `verify`：检查新外键、digest、Package manifest 与计数。
6. 输出机器可读 JSON 和人类可读 Markdown 报告。

执行失败不得修改旧库或留下半成品 v3 数据。App 仍可通过 Adapter Workbench Source 正常读取原 store；这不是 v3 Core 的 fallback read。

### 映射策略

| v2 数据 | v3 结果 |
| --- | --- |
| `waiting` Request | 创建 Imported Session + waiting Feedback Request；保留原 `request_id` 与 source mapping。 |
| `in_progress` Request | 有损映射为 waiting；编辑状态丢弃，Draft 结构保留。 |
| 对应结构化 Draft | 保留 `document_json`、Markdown projection、revision 与可读附件。 |
| completed + 可读且一致的 manifest | 创建 submitted Request、Feedback Submission、response Package 与 `delivered` Delivery；不创建 pending Agent work。 |
| completed 但 Package 不可读/不一致 | 丢弃该 Request，报告 `completed_package_unreadable`。 |
| cancelled Request | 默认丢弃，计入报告；不恢复成 Inbox。 |
| approved / `allow_finish` Request | 丢弃，报告 `unsupported_approval_semantics`。 |
| orphan Draft | 丢弃，报告 `orphan_draft`。 |
| 路径附件可读且满足资源上限 | 导入 Artifact Store，重新计算 digest。 |
| 路径附件不可读 | 丢弃附件；若它使 submitted Package 不完整，则丢弃整个 completed Request。 |
| 旧 `host_id` / `host_session_id` | 只用于把请求分组为 Imported Session，并写迁移来源 metadata；不进入 v3 业务 Interface。 |

迁移完成的历史 submitted Request 是只读事实，其 Delivery 直接标记为 `delivered`，因为迁移器不会重新唤醒或推断原 Agent。Imported Session 的 waiting Request 只保证查看和编辑 Draft，不开放提交或取消，也不自动唤醒旧 Agent；需要继续原操作时使用仍由原 owner 持有的 Adapter Session。对 Imported Session 调用 Managed Feedback resolution Interface 返回 `SESSION_NOT_MANAGED`。Unified Workbench Projection 依据 source mapping 避免同时展示同一来源的 Imported 与 Adapter Session；删除旧 store 或停用 Adapter 路径必须是独立、显式用户动作。

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
- v3 Core 不读取旧表，Adapter Runtime 不写新表；两个 owner 不双写、不互相 fallback。
- Adapter Session 与 Managed ACP Session 同列可见，旧数据无需迁移即可打开并继续原操作。
- 所有 Session/Attention selection 与 command 都携带 source，裸 id 冲突和 source 局部失败不导致串写或清空另一边。
- ACP 主动 Launch、Permission、Ask、runtime 状态/配置与 Managed Feedback Resume 不被误报到 Adapter Session。
- 隔离残留扫描已逐项证明合法 owner，或清除跨 Core 泄漏；不以 Adapter Runtime 名称清零作为完成定义。
- 迁移 dry-run、execute、verify 与损失报告通过 fixture。
- 用户完成 Launch / Inbox / Context View 的 native UI 验收。

## 当前下一步

先完成真实 Managed ACP 原生端到端验收：Launch、Permission、Ask Question、Feedback Draft/Artifact、跨重启等待与提交后的 Delivery 恢复。随后进入 Phase 5：接入 Adapter Workbench Source snapshot/commands，完成 source-aware Session list 与 Inbox、两 source 身份碰撞/局部失败测试、Adapter Runtime 与 v3 Core 零双写审计，并验证 App 退出后只清理受管 ACP 进程、不破坏原 Adapter Continuation。
