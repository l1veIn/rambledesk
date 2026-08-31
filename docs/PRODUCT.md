# RambleDesk 产品基线

> 状态：v3 Unified Workbench 共存基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文只描述产品承诺，不重新定义产品对象。
> 本文描述分支最终产品承诺；尚未完成的 Runtime 与 UI 接线见 [V3_IMPLEMENTATION_PLAN.md](V3_IMPLEMENTATION_PLAN.md)。

## 一句话

RambleDesk 是本地 human-agent workbench：它既主动启动和观察 ACP Coding Agent，也让既有 Adapter Session 保持可见可用；它把 Agent 对人类注意力的需求呈现在同一个 Inbox，再把授权、回答或结构化 Feedback Package 按 Session Source 送回原路径。

## 产品判断

- Agent 可以连续完成大量工程工作，但授权、范围选择、真实体验和最终判断仍属于人类。
- 这些人类输入不应散落在普通对话中；它们需要清晰的请求形态、可恢复的反馈草稿和可引用的提交产物。
- ACP 让 RambleDesk 新增 Session 主动 Launch、运行观察与配置、Prompt、Permission、Ask 和恢复能力；这些能力只对 Managed ACP Session 承诺。
- 已有 MCP、Pi 与原生 Adapter 积累了仍有价值的数据和操作。统一 Workbench 应直接呈现这些 Adapter Session，而不是要求用户先迁移或放弃原路径。
- Agent 自己是完整会话历史的权威来源；RambleDesk 只持久化自己拥有的事实，不复制第二套 transcript。
- 人类等待时间没有上限。Feedback Request 的正确性不能依赖某个 tool call、Prompt Turn、Agent Run 或 App 进程持续存活。
- Feedback Package 的身份与内容不能依赖本机绝对路径；本地目录只是首个存储实现。

## 产品承诺

### RambleDesk 必须做到

- 以 Session 为顶层对象统一导航 Managed ACP Session 与 Adapter Session，并明确显示每个 Session 实际可用的动作。
- 让旧数据无需迁移即可通过 Adapter Workbench Source 直接显示、编辑并继续原操作。
- 让 Launch Ramble 只创建 Managed ACP Session，并提供 Agent 实际支持的 runtime 状态与配置。
- Launch 时展示 Agent 实际支持的 model、reasoning、mode 等选择，并让人类确认 Workspace Reference 与 Access Mode。
- 在 Inbox 中统一呈现 Permission Request、Ask Question 和 Feedback Request。
- 让 Managed ACP Feedback Request 在解决后仍保留于请求列表；提交态可重开已验证的 Feedback Package，取消态保留原请求事实。
- 在 Managed ACP Session 中原样关联并回答 ACP Permission Request；同一 Prompt Turn 可以同时有多个待处理授权。
- 让 Managed ACP Ask Question 在 live 通道存活期间持续等待回答或跳过。
- 让 Feedback Request 跨 Prompt Turn、Agent Run 和 App 重启保持 `waiting`，直到人类提交或明确取消。
- 让 Launch Ramble 与 Feedback Ramble 使用同一套幂等 Package 发布合同。
- 人类提交后先固定本地事实，再恢复 Agent 并交付；Agent 离线不能使提交回滚。
- 以稳定 `delivery_id` 进行至少一次交付，失败后可以在本次或下次启动继续对账。

### RambleDesk 不承诺

- 独立于 Agent 的永久完整会话历史。
- 管理 branch、worktree、checkout 或源码真源。
- 把所有 Agent 的 model、reasoning、permission 选项强行统一成相同语义。
- 所有 Agent 都支持注入工具、Ask Question、session resume 或历史 replay。
- 用长时间占用 MCP tool call 的方式保证 Feedback Request 等待。
- Managed ACP 与 Adapter 的功能完全齐平；主动 Launch、Permission、Ask、runtime config 和 Managed Feedback Resume 可以只属于 ACP source。
- 把 Adapter Runtime 并入 v3 Core、让两个 owner 双写，或在一个命令失败后静默 fallback 到另一个 owner。
- 强迫用户迁移旧数据；有损迁移始终是显式可选操作。

## v3 首发范围

| 范围 | 首发承诺 |
| --- | --- |
| 参考 Agent | Codex ACP；Claude ACP 在 Codex 闭环稳定后进入下一阶段。 |
| Launch | Agent Profile、Launch Profile、Workspace Reference、model、reasoning effort、Access Mode。 |
| Session | 创建、运行观察、Steering、取消当前 turn、App 退出后的恢复。 |
| Inbox | Permission Request、Ask Question、Feedback Request 三类 Attention Item。 |
| Ramble | Launch、Steering、Feedback 三种 intent；继续使用结构化 TipTap Draft。 |
| Package | `launch` 与 `response` 两种 purpose；不可变、内容寻址、位置无关。 |
| Recovery | `session/resume` → `session/load` → `session/new` + Recovery Prompt。 |
| History | 当前 live Context View + RambleDesk 自己拥有的结构化请求/提交记录；Agent 能 replay 时展示其历史，但不建立 RambleDesk transcript。 |
| Session Sources | Managed ACP Session 与 Adapter Session 同列展示；可选有损迁移产生 Imported Session。 |
| Adapter | 现有 Generic MCP、Pi、local server、host 与原生 Adapter 维护冻结，但既有数据和原操作保持可达。 |

“维护冻结”表示不把 ACP 新能力回填到 Adapter 入口，默认只接受安全、数据完整性和阻断性兼容修复；不表示删除、隐藏或强制迁移。Desktop 通过 Unified Workbench Projection 合并两个 source 的只读模型，所有命令回到原 owner；Adapter Runtime 与 v3 Core 不共享表、不双写，也不互相 fallback。

## 核心旅程

### 0. 打开既有 Session

1. App 启动后分别读取 Managed ACP Workbench Source 与 Adapter Workbench Source snapshot，并在第一栏合并为 Session 列表；每项以 Agent 标识和实际 capability 呈现，不要求用户理解数据代际。
2. 人类选择 Adapter Session 时，原有 Feedback Request、Draft 和可用操作直接出现，不先复制到 v3 Core。
3. 保存、提交、取消或继续原操作按 Session Source 回到 Adapter Workbench Source；Managed ACP source 暂时失败不影响 Adapter Session 可用，反之亦然。
4. 人类可另行选择有损迁移。迁移不是查看前提；已迁移来源通过显式 mapping 避免在列表中重复出现。

### 1. Managed ACP Launch

1. 人类选择 Agent 与 Launch Profile。
2. RambleDesk 执行短生命 Launch Preflight，只探测可用性与 Agent 实际返回的 session config options。
3. 人类确认 model、reasoning effort、Workspace Reference 和 Access Mode。
4. 人类完成 Launch Ramble；客户端生成稳定 `submission_id`。
5. RambleDesk 幂等发布 `package_purpose=launch` 的 Feedback Package，同时只创建一次 Session 和首个待发送 Prompt。
6. `rambledesk-acp-client` 启动 Codex、建立 ACP Session Link，并把从 Package 派生的首个 Prompt 发送给 Agent。

相同 `submission_id` 与相同内容的重试必须返回同一 Session 与 Package；相同 id 携带不同内容必须冲突。

### 2. Managed ACP 运行与 Steering

1. Context View 展示当前 Agent Run 的文字、计划、tool call、usage 与状态。
2. 人类可以提交 Steering Ramble，给当前 Session 追加 Prompt。
3. Steering Submission 必须幂等发送，但不生成 Feedback Package。
4. App 可以取消当前 Prompt Turn；取消 live turn 不删除 Session 或已持久化事实。

RambleDesk 不把这些 live event 写成永久 transcript。重新打开 Session 后，Agent 若支持 `session/load` 可以 replay；若只支持 `session/resume`，则继续上下文但不补播历史。

### 3. Managed ACP Permission Request

1. 非 YOLO Agent 在执行 tool call 前通过 ACP 请求授权。
2. RambleDesk 在 Inbox 中显示 tool call、影响位置和 Agent 给出的全部选项。
3. 人类选择后，RambleDesk 直接回答原 JSON-RPC request。
4. 同一 Prompt Turn 的多个授权各自保留关联并排队显示。
5. Prompt Turn 或 Agent Run 断开时，未回答授权以 `cancelled` outcome 结束。

Permission Request 是 live 交互，不生成 Feedback Package，也不在重启后恢复成历史待办。

### 4. Managed ACP Ask Question

1. Agent 通过 ACP 原生 elicitation form 或经验证的 Agent-specific Question Channel 发起结构化问题。
2. RambleDesk 把它投影为 Ask Question，显示在 Inbox 与 Session 输入区上方。
3. 人类可以回答、选择跳过，或取消承载它的 Prompt Turn。
4. live 通道存在时问题可以持续等待；通道断开后不把它恢复成持久待办。

Ask Question 解决具体选择，不代替需要真实体验、审阅证据和长时间等待的 Feedback Request。

### 5. Managed ACP Feedback Request

1. Agent 调用 Session Toolset 的 `request_feedback`，提交说明、actions 和 context refs。
2. RambleDesk 持久化 Feedback Request 后立即返回 `request_id`；不让 tool call 无限等待。
3. Agent 可以安全结束当前 Prompt Turn。Session 进入 Waiting for Feedback。
4. 人类可以稍后打开 Request、ramble、截图、附加证据并保存结构化 Draft。
5. 编辑 Draft 不改变 Request 状态；唯一非终态始终是 `waiting`。
6. 人类提交时，RambleDesk 幂等发布 `package_purpose=response` 的 Feedback Package，并在本地固定 `submitted` resolution 与 pending Feedback Delivery。
7. 人类明确取消时，RambleDesk 固定 `cancelled` resolution 与 pending Delivery，不创建空 Package。
8. 本地提交成功后，`rambledesk-acp-client` 恢复 Session，并以同一个 `delivery_id` 对账交付。

如果 Agent 当前不可用，用户提交仍然成功；UI 将该事实显示为“已提交，待交付”，而不是重新放回未回答 Inbox。

### 6. App 退出与恢复

- 关窗口或进入托盘：Managed ACP Agent Run 继续。
- 完整退出 RambleDesk：结束所有由 RambleDesk 启动的 live Agent Run；Managed ACP Session、Draft、Request、Package 与 Delivery 保留。Adapter 外部进程仍服从原 owner。
- 再次打开：分别恢复 Managed ACP 与 Adapter snapshot，合并 Session list 与 Inbox；不会为了纯 waiting Managed ACP Feedback Request 启动 Agent。
- 人类提交或 App 对账 pending Delivery 时，才按需建立新 Agent Run。
- 恢复优先级固定为 `session/resume`、`session/load`、`session/new` + Recovery Prompt。
- Agent 返回的 live config 是恢复后的当前真源；Launch Configuration 只是在必须创建新 ACP Session 时重用的启动快照。

## 工作台信息架构

```text
RambleDesk
├── Launch
│   ├── Agent / Launch Profile
│   ├── Workspace Reference
│   ├── Model / Reasoning
│   ├── Access Mode
│   └── Launch Ramble
├── Sessions
│   ├── unified Session list, source and capabilities
│   └── Context View
│       ├── Agent live activity
│       ├── Ask Question above input
│       ├── Steering Ramble
│       └── cancel / resume affordances
├── Inbox
│   ├── Permission Request
│   ├── Ask Question
│   └── Feedback Request
├── Ramble Workspace
│   ├── instructions and actions
│   ├── structured Draft
│   ├── voice / screenshot / attachments
│   ├── optional Feedback Cooking
│   └── submit / cancel
└── Settings
    ├── Agent and Launch Profiles
    ├── notifications
    ├── speech / Tidy / Cooking
    ├── appearance and language
    └── global shortcuts
```

请求列表统一呈现两个 Session Source 的请求；其中 waiting Permission、Ask 和 Feedback 才是当前需要人类处理的 Inbox 子集，动作必须回到原 owner。Managed ACP 的 terminal Feedback Request 继续作为 RambleDesk 自己拥有的结构化历史显示：submitted 可以重开已验证的 Feedback Package，cancelled 只读展示原请求。尚未送达 Agent 的 Feedback Delivery 另显示为对应 Session 的交付状态，不把 terminal Request 伪装成未回复请求；Adapter Session 继续使用原路径的终态与交付状态。

## Feedback Package 产品合同

- Launch Ramble 和 Feedback Ramble 使用同一格式，分别标记 `launch` 与 `response`。
- Package 不可变；身份由 `package_id` 与 digest 决定，不由目录名或 URI 决定。
- manifest 记录正式正文、原始正文和各 Artifact Entry 的 media type、size 与 digest。
- 本地文件、远程对象、内联内容都可以承载 Artifact；交付时通过 Artifact Locator 读取。
- `uncooked.md` 始终保留；Feedback Cooking 只能生成可选择的正式正文，不能覆盖人类原稿。
- API Key、Authorization、Agent 私有 transcript 和模型服务私有 metadata 不进入 Package。

## 首条端到端验收场景

Managed ACP 第一条竖切验收一个完整故事：

1. 通过 Launch Preflight 选择 Codex、Workspace Reference、model、reasoning effort 与 Workspace Write。
2. 提交 Launch Ramble，得到唯一 Session、Launch Submission 和 `launch` Package。
3. Codex 在该 Session 中运行并产生一个 Permission Request；人类允许后继续。
4. Codex 调用 `request_feedback`，得到 `request_id` 并结束当前 turn。
5. 完整退出 RambleDesk，再次打开后，Inbox 仍有同一个 waiting Feedback Request 和原 Draft。
6. 人类提交 Feedback Ramble；本地立即显示 `submitted`，即使 Agent 尚未启动。
7. RambleDesk 恢复原 ACP Session；若恢复不可用则新建 Session 并发送 Recovery Prompt。
8. Codex 调用 `get_feedback(request_id)`，读取同一个 `delivery_id` 对应的 Package，完成下一轮工作。
9. 强制重试 Launch、提交和 Delivery，均不得产生重复 Session、Package 或人类 resolution。

这条故事未通过前，不扩展 Claude，也不先做完整历史浏览器。它通过后仍不能以 ACP 成熟为由移除 Adapter Runtime；统一 Workbench 还必须单独通过共存验收：Adapter Session/Request 直接可见、原操作可继续、两 source id 碰撞不串写、任一 source 失败不清空另一边。

## 成功指标

- Launch Submission 幂等成功率与重复首个 Prompt 次数。
- Managed ACP Session 创建、resume、load 和 recovery 各路径成功率。
- Permission Request 从出现到回答的时间与关联错误数。
- waiting Feedback Request 跨 App 重启恢复率。
- Feedback Submission 本地提交成功率，不受 Agent 在线状态影响。
- pending Feedback Delivery 最终送达率、重试次数和重复消费率。
- 人类从 Inbox 打开到提交 Feedback Package 的中位时长。
- Adapter Session 直接显示与原操作成功率，以及跨 source 误路由/重复展示数。
