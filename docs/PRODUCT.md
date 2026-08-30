# RambleDesk 产品基线

> 状态：v3 ACP-first 可执行基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文只描述产品承诺，不重新定义产品对象。

## 一句话

RambleDesk 是本地 human-agent workbench：它启动和观察本机 Coding Agent，把 Agent 对人类注意力的需求归一化到 Inbox，再把人类的授权、回答或结构化 Feedback Package 送回原 Session。

## 产品判断

- Agent 可以连续完成大量工程工作，但授权、范围选择、真实体验和最终判断仍属于人类。
- 这些人类输入不应散落在普通对话中；它们需要清晰的请求形态、可恢复的反馈草稿和可引用的提交产物。
- ACP 让 RambleDesk 自然地拥有 Session 启动、运行观察、Prompt、权限透传和恢复能力。
- Agent 自己是完整会话历史的权威来源；RambleDesk 只持久化自己拥有的事实，不复制第二套 transcript。
- 人类等待时间没有上限。Feedback Request 的正确性不能依赖某个 tool call、Prompt Turn、Agent Run 或 App 进程持续存活。
- Feedback Package 的身份与内容不能依赖本机绝对路径；本地目录只是首个存储实现。

## 产品承诺

### RambleDesk 必须做到

- 以 Session 为顶层对象启动、观察和继续 Coding Agent。
- Launch 时展示 Agent 实际支持的 model、reasoning、mode 等选择，并让人类确认 Workspace Reference 与 Access Mode。
- 在 Inbox 中统一呈现 Permission Request、Ask Question 和 Feedback Request。
- 原样关联并回答 ACP Permission Request；同一 Prompt Turn 可以同时有多个待处理授权。
- 让 Ask Question 在 live 通道存活期间持续等待回答或跳过。
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
- v3 首个里程碑继续支持现有 MCP、Pi 或其他原生 Adapter。

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
| History | 当前 live Context View；Agent 能 replay 时展示其历史，但不建立 RambleDesk transcript。 |
| Compatibility | 现有 Generic MCP、Pi 和原生 Adapter 全部冻结，不属于 v3 首发验收。 |

“冻结”表示不为旧入口做双模型兼容，也不把它们接入新功能。分支完成前，旧运行路径必须从 v3 装配中移除；未来若恢复 Generic MCP，只能作为新领域模型上的薄 Compatibility Ingress 重新实现。

## 核心旅程

### 1. Launch

1. 人类选择 Agent 与 Launch Profile。
2. RambleDesk 执行短生命 Launch Preflight，只探测可用性与 Agent 实际返回的 session config options。
3. 人类确认 model、reasoning effort、Workspace Reference 和 Access Mode。
4. 人类完成 Launch Ramble；客户端生成稳定 `submission_id`。
5. RambleDesk 幂等发布 `package_purpose=launch` 的 Feedback Package，同时只创建一次 Session 和首个待发送 Prompt。
6. `rambledesk-acp-client` 启动 Codex、建立 ACP Session Link，并把从 Package 派生的首个 Prompt 发送给 Agent。

相同 `submission_id` 与相同内容的重试必须返回同一 Session 与 Package；相同 id 携带不同内容必须冲突。

### 2. 运行与 Steering

1. Context View 展示当前 Agent Run 的文字、计划、tool call、usage 与状态。
2. 人类可以提交 Steering Ramble，给当前 Session 追加 Prompt。
3. Steering Submission 必须幂等发送，但不生成 Feedback Package。
4. App 可以取消当前 Prompt Turn；取消 live turn 不删除 Session 或已持久化事实。

RambleDesk 不把这些 live event 写成永久 transcript。重新打开 Session 后，Agent 若支持 `session/load` 可以 replay；若只支持 `session/resume`，则继续上下文但不补播历史。

### 3. Permission Request

1. 非 YOLO Agent 在执行 tool call 前通过 ACP 请求授权。
2. RambleDesk 在 Inbox 中显示 tool call、影响位置和 Agent 给出的全部选项。
3. 人类选择后，RambleDesk 直接回答原 JSON-RPC request。
4. 同一 Prompt Turn 的多个授权各自保留关联并排队显示。
5. Prompt Turn 或 Agent Run 断开时，未回答授权以 `cancelled` outcome 结束。

Permission Request 是 live 交互，不生成 Feedback Package，也不在重启后恢复成历史待办。

### 4. Ask Question

1. Agent 通过 ACP 原生 elicitation form 或经验证的 Agent-specific Question Channel 发起结构化问题。
2. RambleDesk 把它投影为 Ask Question，显示在 Inbox 与 Session 输入区上方。
3. 人类可以回答、选择跳过，或取消承载它的 Prompt Turn。
4. live 通道存在时问题可以持续等待；通道断开后不把它恢复成持久待办。

Ask Question 解决具体选择，不代替需要真实体验、审阅证据和长时间等待的 Feedback Request。

### 5. Feedback Request

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

- 关窗口或进入托盘：Agent Run 继续。
- 完整退出 RambleDesk：结束所有 live Agent Run；Session、Draft、Request、Package 与 Delivery 保留。
- 再次打开：恢复 Session 列表、waiting Feedback Request、Draft 和 pending Delivery，但不为了等待人类而启动 Agent。
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
│   ├── Session list and state
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

Inbox 只表示当前正在等待人类处理的项目。已提交但尚未送达 Agent 的 Feedback Delivery 显示在对应 Session 的交付状态中，不伪装成未处理 Feedback Request。

## Feedback Package 产品合同

- Launch Ramble 和 Feedback Ramble 使用同一格式，分别标记 `launch` 与 `response`。
- Package 不可变；身份由 `package_id` 与 digest 决定，不由目录名或 URI 决定。
- manifest 记录正式正文、原始正文和各 Artifact Entry 的 media type、size 与 digest。
- 本地文件、远程对象、内联内容都可以承载 Artifact；交付时通过 Artifact Locator 读取。
- `uncooked.md` 始终保留；Feedback Cooking 只能生成可选择的正式正文，不能覆盖人类原稿。
- API Key、Authorization、Agent 私有 transcript 和模型服务私有 metadata 不进入 Package。

## 首条端到端验收场景

v3 第一条竖切只验收一个完整故事：

1. 通过 Launch Preflight 选择 Codex、Workspace Reference、model、reasoning effort 与 Workspace Write。
2. 提交 Launch Ramble，得到唯一 Session、Launch Submission 和 `launch` Package。
3. Codex 在该 Session 中运行并产生一个 Permission Request；人类允许后继续。
4. Codex 调用 `request_feedback`，得到 `request_id` 并结束当前 turn。
5. 完整退出 RambleDesk，再次打开后，Inbox 仍有同一个 waiting Feedback Request 和原 Draft。
6. 人类提交 Feedback Ramble；本地立即显示 `submitted`，即使 Agent 尚未启动。
7. RambleDesk 恢复原 ACP Session；若恢复不可用则新建 Session 并发送 Recovery Prompt。
8. Codex 调用 `get_feedback(request_id)`，读取同一个 `delivery_id` 对应的 Package，完成下一轮工作。
9. 强制重试 Launch、提交和 Delivery，均不得产生重复 Session、Package 或人类 resolution。

这条故事未通过前，不恢复 Generic MCP、Pi，不扩展 Claude，也不先做完整历史浏览器。

## 成功指标

- Launch Submission 幂等成功率与重复首个 Prompt 次数。
- Managed Session 创建、resume、load 和 recovery 各路径成功率。
- Permission Request 从出现到回答的时间与关联错误数。
- waiting Feedback Request 跨 App 重启恢复率。
- Feedback Submission 本地提交成功率，不受 Agent 在线状态影响。
- pending Feedback Delivery 最终送达率、重试次数和重复消费率。
- 人类从 Inbox 打开到提交 Feedback Package 的中位时长。
