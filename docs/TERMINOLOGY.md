# RambleDesk 术语表

> 状态：v3 ACP-first 重构基线。
> 目标：固定产品对象、协议角色、身份字段和 package 边界。代码、文档、UI 文案、测试命名若与本文冲突，以本文为准。
> 这是规范目标，不是当前 Desktop 接线快照；过渡期旧 Implementation 的删除进度见 [V3_IMPLEMENTATION_PLAN.md](V3_IMPLEMENTATION_PLAN.md)。

本文是 RambleDesk 的唯一术语源。其他文档只引用本文，不重新定义产品对象。

## 产品定义

RambleDesk 是本地 human-agent workbench。它把 Agent 对人类注意力的需求归一化为 Inbox 中的 Permission Request、Ask Question 和 Feedback Request，再把人类的授权、答案或结构化 Feedback Package 送回原 Session。人类也可以用 Launch Ramble 表达并密封任务，启动、观察和指挥本机 Coding Agent。

RambleDesk 在 ACP 中扮演 **Client**：它可以启动 Agent 子进程、建立 Session、发送 Prompt、处理权限、接收事件和结束任务。RambleDesk 不因此成为 IDE，也不取得源码 checkout 的所有权。

## 架构公理

1. **Session 是顶层产品对象。** Session 可以在没有任何 Feedback Request 时创建并运行，也可以包含多个 Agent Run、Prompt Turn、Attention Item 和 Feedback Package。
2. **ACP 是受管 Session 的主路径。** RambleDesk 通过 `rambledesk-acp-client` 实现 ACP Client 角色；产品和代码不再使用 `rambledesk-acp-host`。
3. **Ramble 是人类表达动作，提交产物由 intent 决定。** Launch Ramble 和 Feedback Ramble 都发布 Feedback Package，分别标记为 `launch` 和 `response`；Launch 同时派生首个 Prompt，Steering 只派生后续 Prompt。
4. **Session、Ramble Submission、Feedback Request 和 Feedback Package 是核心事实。** Agent Run、Prompt Turn、Permission Request 和 Ask Question 是 live 运行交互；Attention Item 只是实时 read model。不得为了复刻 Agent 会话再复制第二套事实库。
5. **所有接入路径共享同一套新领域模型。** ACP Managed Path 是主路径；MCP 和既有原生 Adapter 是过渡期 Compatibility Ingress，不得形成第二套 Session、Request 或 Package 语义。
6. **Workspace Reference 是受管 Session 的启动上下文。** RambleDesk 可以要求一个本机目录来启动 Agent，但不管理 branch、worktree、checkout、git 生命周期或文件真源。
7. **`core` 保持协议中立。** ACP stdio/JSON-RPC、MCP、HTTP、Tauri command、进程树和厂商启动命令不得进入 `core`。
8. **v3 目标运行时只支持新数据模型。** 新对象写入新表；运行时不得双读旧表、回退旧字段、维护兼容别名，或在同一 use case 中分叉新旧语义。
9. **旧数据只通过独立的一次性脚本迁移。** 迁移允许显式、有报告的语义损失；迁移器读取旧表并写入新表，但旧表不是运行时数据源。
10. **应用生命周期拥有受管进程生命周期。** 关窗口或进入托盘不结束 Agent Run；退出 RambleDesk 结束所有 live Agent Run；取消一个 Session 只结束该 Session 当前的进程树。Session Record、Ramble Submission、Feedback Request、Package 和 Delivery 不随进程退出而删除。
11. **受管工具能力按 Agent Run 验收。** ACP Client 可以在 Session 建立时向 Agent 提供 MCP server 配置，但不得假设所有 Agent 都会接收、转发或无限期等待注入工具。
12. **Feedback Package 与存储位置无关。** Package 身份、内容清单和完整性不得依赖绝对本地路径；本地文件、远程对象和内联内容只是可替换的存储或交付方式。
13. **Ramble 提交和 Package 发布是幂等操作。** 客户端在提交前生成稳定 `submission_id`；同一 id 与同一内容 digest 的重试返回同一结果，同一 id 携带不同内容必须冲突，不能覆盖或追加发布。
14. **Agent 是完整会话历史的权威来源。** RambleDesk 不承诺独立于 Agent 的稳定完整历史视图，也不持久化完整 ACP transcript；重新打开 Session 时，历史可见性取决于 Agent 的 `session/load`、原生历史能力和保留策略。
15. **RambleDesk 只持久化自己拥有的事实。** 所有 Ramble Submission、Feedback Request、Feedback Package 和 Delivery 必须可恢复；Permission Request 与 Ask Question 不另建持久历史，断开其所属 Agent Run 时必须取消。
16. **Feedback Request 的人类决议与 Agent 交付是两个独立事实。** 人类提交或取消时先在本地原子固定 Request resolution、提交时的 Package 和 pending Delivery，再恢复 Agent Run 并交付；Agent 临时离线不得使已提交的人类事实回滚。

## 两条 Session 路径

### ACP Managed Path（主路径）

1. RambleDesk 先通过 Launch Preflight 确认 Agent 可用性和它实际提供的 session config options；人类再确认 Launch Configuration：Agent Profile、Launch Profile、model、Workspace Reference、reasoning effort 和 Access Mode。
2. 人类提交 Launch Ramble；RambleDesk 以 `submission_id` 幂等发布 `package_purpose=launch` 的 Feedback Package，并确保只创建一次对应 Session 和初始启动意图。
3. `rambledesk-acp-client` 建立 Agent Run 和 ACP Session Link，把从 `launch` Package 派生的首个 Prompt 交给 ACP Agent。进程启动失败可以重试，但不得重复发布 Package、创建 Session 或发送同一个首个 Prompt。
4. RambleDesk 持久化 Session Record 和每个 Ramble Submission；ACP `session/update` 只归一化为当前 Context View 的 live event，不写入完整 transcript。Prompt Turn 是运行边界，不要求独立 `turns` 表。
5. 非 YOLO 模式下，Agent 可以通过 ACP 发出零到多个 Permission Request。RambleDesk 按原 JSON-RPC request 逐项关联和排队，完整呈现 tool call 与选项，并把人类选择直接回给对应请求。
6. Agent Run 具备 Question Capability 时，Agent 可以发起 Ask Question；首选 transport 是 ACP `elicitation/create` form，也可以是经 Launch Profile 验证的 Agent-specific Question Channel。问题显示在 Session 输入框上方，直到人类回答、跳过，或承载通道被取消/断开。
7. Agent 需要真实体验和判断时，通过注入的 `request_feedback` 工具创建 Feedback Request。请求进入无业务超时的 Waiting for Feedback；工具在持久化确认后立即返回 `request_id`，不把无限等待绑定到某个 MCP request。
8. Agent 收到确认后结束当前 Prompt Turn；Session 保持 Waiting for Feedback，直到人类提交或明确取消。这个等待可以跨 Prompt Turn、窗口关闭和 Agent Run。重启 RambleDesk 只恢复 Inbox 中的该 Feedback Request，不为了等待人类而提前启动 Agent。
9. 人类提交 Feedback Ramble 时，RambleDesk 在同一本地事务中幂等固定 Ramble Submission、`package_purpose=response` 的 Feedback Package、Request 的 `submitted` resolution 和唯一 pending Feedback Delivery；人类明确取消时固定 `cancelled` resolution 和 pending Delivery，不伪造空 Package。这个事务不依赖 Agent Run 在线。
10. 本地事实提交后，`rambledesk-acp-client` 幂等对账 pending Delivery：复用健康的当前 Agent Run；否则按 Launch Configuration 启动新 Run，完成 initialize 后依 capability 按 `session/resume` → `session/load` → `session/new` + Recovery Prompt 恢复。任一步失败都不回滚 resolution，Delivery 保持 pending 并可在本次或下次 App 启动时继续对账。
11. **保底交付路径：** `rambledesk-acp-client` 在已恢复的 ACP Session 中发送 Feedback Resume Prompt，明确要求 Agent 调用 `get_feedback(request_id)`；该工具返回稳定 Feedback Delivery Envelope。输送确认前的不确定失败使用同一 `delivery_id` 重试。
12. 直接把 Feedback Delivery Envelope 随 Feedback Resume Prompt 内联、经验证的长等待工具直接返回，或未来使用 ACP 原生能力，都只是更优雅的交付策略；必须与保底路径共享同一 `request_id`、`package_id`、`delivery_id` 和幂等语义。

### External Compatibility Path（过渡路径）

1. RambleDesk 外部运行的 Agent 通过 MCP 或原生 Adapter 创建 Feedback Request。
2. 若不存在对应 Session，RambleDesk 先在新数据模型中创建 Connected Session，再挂入 Feedback Request。
3. 人类完成 Feedback Ramble 并发布 Feedback Package。
4. 外部 Agent 通过原接入路径读取 Feedback Delivery Envelope 并继续。

Compatibility Ingress 可以缺少受管进程、ACP 历史和自动 Prompt 能力，但不得使用旧表或复制一套领域对象。

## 核心术语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| 人类（Human） | 使用 RambleDesk 表达需求、指挥执行并作出真实判断的人。 | 拥有产品判断和授权决定；不拥有协议状态。 |
| Agent | 执行任务、作为完整会话历史权威来源、发起需要人类处理的交互的 coding actor。 | 拥有任务推理和 Agent transcript；不拥有 RambleDesk 的 Ramble、Request、Package 或 Delivery 状态。 |
| Agent Profile | 人类可识别的一类 Agent，例如 Codex、Claude Code、Gemini。 | 描述身份、能力和展示元数据；不等于某条启动命令。 |
| Launch Profile | 启动一个 Agent 的具体方法，包括 command、args、环境要求、探测和安装提示。 | 一个 Agent Profile 可以有多个 Launch Profile；不承载 Session 状态。 |
| Launch Preflight | 在创建产品 Session 前对 Launch Profile 进行的短生命可用性与能力探测。 | 可以短暂启动 Agent 并读取 ACP config options，随后立即拆除；不创建 RambleDesk Session、Launch Package、ACP Session Link 或用户历史。 |
| Launch Configuration | 人类在 Launch 时确认的受管 Session 启动快照，包含 Agent/Launch Profile、model、Workspace Reference、reasoning effort 和 Access Mode。 | 归属 Session Record，用于首次启动与新 Agent Run 恢复；ACP Session 恢复后，Agent 返回的当前 mode/model/config 才是 live 状态真源。 |
| ACP Agent | 通过 ACP 暴露能力、由 RambleDesk 连接的 Agent 进程。 | 在协议中是 Agent/Server；不是 RambleDesk 的“宿主”。 |
| ACP Client | RambleDesk 在 ACP 中的协议角色。 | 负责连接、Session 调用、事件和权限回包；不拥有反馈编辑语义。 |
| 工作台（Workbench） | RambleDesk 桌面产品，包括 Session 导航、Ramble 编辑器、Inbox 和 Context View。 | 拥有人类工作流；不直接实现 ACP 或 MCP wire protocol。 |
| Workspace Reference | 启动受管 Agent 时使用的本机目录引用。 | 是执行上下文，不是 checkout 所有权、持久身份或认证凭据。 |
| Access Mode | Launch Configuration 中的执行与授权姿态，产品值为 Read Only、Workspace Write 或 YOLO。 | Launch Profile 必须把其显式映射到该 Agent 的 sandbox/permission 能力；它不是某个 Permission Request，也不能超出 Agent 实际能力宣称统一强制语义。 |
| Session | RambleDesk 中一段可持久、可观察的 human-agent 工作关系，用 `session_id` 标识。 | 是顶层聚合；拥有 ACP Session Link、Ramble 和反馈关联；运行时可以先后建立多个 Agent Run。 |
| Managed Session | 由 RambleDesk 经 ACP 创建并监督的 Session。 | 拥有受管进程和 ACP 关联；生命周期受 RambleDesk 控制。 |
| Connected Session | 为外部 MCP/原生 Adapter 请求建立的兼容 Session。 | 可以没有受管进程和 ACP 关联；仍使用当前领域模型。 |
| Agent Run | Managed Session 中一次具体的 Agent 启动与连接实例。 | 是 live runtime 对象，拥有进程树和连接状态；默认不建立跨重启历史表，Session 可以比它活得更久。 |
| ACP Session Link | RambleDesk Session 与 Agent 返回的 ACP `sessionId` 之间的持久关联。 | 保存恢复所需的 opaque id、协议能力和 Session Toolset 配置，并引用 Session 的 Launch Configuration；可以跨 Agent Run 被 `resume` 或 `load`，一个 RambleDesk Session 也可以先后关联多个 ACP Session。 |
| Session Record | RambleDesk 为 Session 保存的最小持久记录。 | 至少包含本地身份、类型、Launch Configuration、ACP Session Link、状态和时间戳；不保存完整 Agent transcript。 |
| Prompt Turn | 一次 `session/prompt` 开始、到 Agent 返回 stop reason 或被取消为止的 ACP 运行区间。 | 是 live 执行边界，不是独立聚合，不强制独立表或跨 replay 稳定的 `turn_id`。 |
| Live Session Event | `rambledesk-acp-client` 从当前 ACP 输入、`session/update` 和交互状态归一化出的瞬时 UI 事件。 | 用于当前 Context View；不持久化为 RambleDesk transcript，断线后可以丢失。 |
| Agent Session History | Agent 自己保存并在能力允许时向 Client 提供的完整会话历史。 | 可能来自 `session/load` replay 或 Agent 原生 store；RambleDesk 不复制、不保证永久可用，也不解析厂商私有历史库。 |
| Context View | 展示当前 Session live activity，并在 Agent 能提供时展示历史的 UI。 | 可以发送 Steering Ramble、取消或选择能力；不是第二个 Inbox、反馈编辑器或稳定 transcript archive。 |
| Attention Item | Inbox 对“当前正在等待人类处理”的统一 read model。 | 可以投影 Permission Request、Ask Question 或 Feedback Request；不要求三者共享 id、生命周期、传输或存储表。 |
| Permission Request | Agent 在一个 active Prompt Turn 中通过 ACP 请求允许或拒绝某项 tool call。 | 常见于非 YOLO 模式；同一 Run 可以有多个 pending 请求。RambleDesk 保留 live request 关联并直接回包，但不另建持久历史；不生成 Feedback Package。 |
| Ask Question | Agent 为解决一个具体选择而向人类提出的结构化问题。 | 通过 Question Channel 呈现在 Session 输入框上方；回答或跳过只解除当前 live 等待，不另建持久历史，也不生成 Feedback Package。 |
| Question Channel | 把 Ask Question 从 Agent 送达人类并把答案交回的能力。 | 首选 ACP `elicitation/create` form；也可以由经验证的 Agent-specific `ask_user_question` 工具或原生桥接实现。产品不因此引入 Elicitation Request；能力归属具体 Agent Run。 |
| Session Toolset | RambleDesk 在 Agent Run 建立时提供给 Agent 的工具集合。 | 当前至少可包含 `ask_user_question`、`request_feedback` 和 `get_feedback`；通常经 ACP 的 MCP server 配置注入，但领域合同不依赖 MCP wire shape。 |
| Feedback Request | Agent 请求人类进行真实体验、判断或审阅的持久单位，用 `request_id` 标识。 | 唯一非终态是 `waiting`，终态 resolution 是 `submitted` 或 `cancelled`；没有业务超时，可以跨 Prompt Turn、Agent Run 和 App 重启。Managed Session 中由注入工具创建，Connected Session 中由 Compatibility Ingress 创建。 |
| Waiting for Feedback | Session 已有 pending Feedback Request、正在等待人类提交或取消的持久状态。 | 不要求保持 MCP request、Prompt Turn 或 Agent 进程存活；只有人类提交或明确取消才能结束该等待。 |
| Feedback Package | Launch Ramble 或 Feedback Ramble 提交后发布的不可变、位置无关反馈包，包含 manifest、正式与原始内容、Artifact Entry、digest 和 `package_purpose`。 | `launch` Package 绑定 Launch submission；`response` Package 绑定 Feedback Request。二者格式与发布合同一致，不把绝对文件路径作为内容合同。 |
| Artifact Entry | Feedback Package 中一个具名内容项，例如 Markdown、截图、录音或其他附件。 | 用 `artifact_id`、media type、size 和 digest 描述；内容可以内联或由 Artifact Locator 解析。 |
| Artifact Locator | 在一次交付中读取 Artifact Entry 内容的位置无关引用。 | 可以是受认证 URI、临时签名 URL 或工具可解析的 opaque ref；不是 Package 身份，也不得假设为本地路径。 |
| Feedback Delivery | 把一个已终态 Feedback Request 交回 Managed Session 的持久交付意图。 | 状态为 `pending` 或 `delivered`；用稳定 `delivery_id` 支持 at-least-once 重试和去重。没有可用 Agent Run 时保持 pending，不回滚 Request resolution，也不退化成本地路径或人工 continuation。 |
| Feedback Delivery Envelope | 把 Feedback Request 的终态交给 Agent 的 transport-neutral envelope。 | 必须包含 `delivery_id`、`request_id` 和 resolution；提交时还包含 `package_id`、可直接消费的反馈正文及 Artifact Entry/Locator，取消时包含原因且没有虚构 Package。 |
| Managed Feedback Resume | Feedback Request 终态后，在同一 Managed Session 中重新唤起 Agent 获取结果的动作。 | 基线实现是 Feedback Resume Prompt + `get_feedback(request_id)`；直接内联 Envelope 或长等待返回只是可替换优化，不叫 Compatibility Continuation。 |
| Ramble | 人类以语音、文字、截图等方式形成一次意图明确的输入。 | 是采集动作和编辑体验；落到何种对象由 Ramble Intent 决定。 |
| Ramble Intent | 一次 Ramble 的提交目的。 | 当前只有 Launch、Steering、Feedback 三种；不得用 UI 位置猜测 intent。 |
| Ramble Submission | 一个 Ramble 被人类正式提交后的持久事实，用 `submission_id` 标识。 | Launch/Feedback Submission 关联 Feedback Package；Steering Submission 关联已发送 Prompt。提交内容必须能从 Submission 本身或关联 Package 恢复，并保存 digest；不要求重复存同一正文，也不等于 Agent transcript。 |
| Launch Ramble | 用于创建 Session 的 Ramble。 | 不需要 Feedback Request；提交时发布 `package_purpose=launch` 的 Feedback Package，并由该 Package 派生首个 Prompt。 |
| Steering Ramble | 用于向既有 Session 追加 Prompt 的 Ramble。 | 不生成 Feedback Request 或 Feedback Package。 |
| Feedback Ramble | 为一个 Feedback Request 采集并提交判断的 Ramble。 | 必须绑定 `request_id`；提交后发布 `package_purpose=response` 的 Feedback Package。 |
| Ramble Draft | 尚未提交的结构化 Ramble 文档。canonical 真源是版本化 TipTap `document_json`，Markdown 是派生投影。 | 必须记录 Ramble Intent；Launch Draft 记录启动选择，Steering/Feedback Draft 记录目标 Session，Feedback Draft 还记录目标 Request。 |
| Action Group | 用标准 Blockquote 表达的 `@Action` 归属容器。 | 同一 Action 再次打开时创建新容器，不与旧区间合并。 |
| Tidy | 人类在当前 Editor 中手动触发的 ASR 段落整理。 | 只处理 pending 语音节点；后台文档不整理；不是 Cooking。 |
| Uncooked Feedback | Feedback Ramble 中的人类原始反馈正文；允许保留口语、重复和自我修正。 | Cooking 不得覆盖；提交后进入 Feedback Package 的 `uncooked.md`。 |
| Cooking | Feedback Ramble 提交前可选的大模型编辑步骤。 | 只做表达整理，不得编造事实、测试结果或删除负面判断；不作用于 Launch/Steering Ramble。 |
| Cooked Feedback | Cooking 生成并经人类选择提交的正式反馈正文。 | 保存为 `feedback.md`，来源必须可追溯到 `uncooked.md`。 |
| Compatibility Ingress | 让外部 Agent 把输入送入当前领域模型的过渡接入层。 | MCP 和既有原生 Adapter 属于此类；它不是产品主路径或第二套模型。 |
| Compatibility Continuation | 在 Connected Session 中提醒或帮助外部 Agent 重新读取反馈的过渡机制。 | 只服务 External Compatibility Path；不是 Managed Session 的领域对象，也不得拥有 Feedback Package 内容。 |

## 必须保持的区分

### Agent Profile、Launch Profile 与 Launch Configuration

Agent Profile 回答“用户选择的是谁”，Launch Profile 回答“这台电脑如何启动它”，Launch Configuration 回答“这个 Session 当时以什么选项启动”。`claude`、`claude-agent-acp` 或 adapter package 不得因名字相近而被当成同一可执行文件。

Launch 表单至少提供 Agent、model、工作目录、reasoning effort 和 Access Mode。可选值优先来自 Launch Preflight 读到的 ACP config options；某个 Agent 不支持的选项必须由 Launch Profile 明确隐藏或拒绝，不能接受后静默忽略。恢复已有 ACP Session 时不用启动快照强行覆盖 Agent 返回的当前 session config；只有退化为新 ACP Session 时才重用该快照。

### Access Mode 与 Permission Request

Access Mode 是 Agent Run 建立前的 Session 级执行姿态，Permission Request 是 Agent Run 中一次具体动作的 live 请求。Read Only 或 Workspace Write 可以导致多次 Permission Request；YOLO 通常减少或跳过该 channel。二者不得合并为一个“权限状态”。

### Session 与 ACP Session

`session_id` 是 RambleDesk 持久身份。`acp_session_id` 是某次 ACP 关联中的不透明协议 id。二者不得相等假设、互相回退或作为对方的兼容别名。

只保存 `acp_session_id` 不足以恢复 RambleDesk 产品状态：ACP `session/load` 还需要 cwd 和 MCP server 配置，`session/resume` 也需要恢复关联配置；Agent/Launch/Workspace、Ramble Submission、Feedback Request、Package 和 Delivery 更不属于 ACP。上述信息归 Session Record、ACP Session Link 及各自的结构化记录。

### Session 与 Agent Run

Session 是持久产品记录，Agent Run 是一次可结束的执行实例，ACP Session Link 则是可以跨 Run 恢复的 Agent 会话关联。退出 App 会结束 live Agent Run，但保留 Session Record、ACP Session Link 和 RambleDesk 自己拥有的事实；重启后是否能查看完整 Agent 历史取决于 Agent capability。恢复时必须先建立新 Agent Run 并 initialize，再对已有 ACP Session 按 capability 优先 `session/resume`，失败或不支持时再尝试 `session/load`。

### ACP Session 恢复与历史

- Agent 声明 `loadSession` 时，RambleDesk 可以用已知 `acp_session_id` 调用 `session/load`；Agent 必须通过 `session/update` 回放完整会话历史，然后才能完成 load。
- Agent 声明 `sessionCapabilities.resume` 时，RambleDesk 可以恢复上下文继续 Prompt，但协议明确不回放历史。对于不需要历史回放的 Feedback Delivery，这是首选恢复方式。
- `session/list` 只在 Agent 声明相应 capability 时用于发现会话及元数据，不返回完整历史，也不替代 RambleDesk 的 Session Record。
- RambleDesk 不承诺稳定完整历史：Agent 仍保留会话且支持 `session/load` 时可以回放；只支持 `session/resume` 时可以继续但看不到旧历史；两者都不支持时只能建立新的 ACP Session。
- `session/load` replay 只进入当前 Context View，不落一份 RambleDesk transcript。Ramble Submission、Request、Package、Delivery 和 Ramble Draft 始终从自己的结构化记录恢复。
- 若未来某类无原生 store 的 Agent 必须获得稳定历史，必须把“条件式 transcript mirror”作为独立产品能力重新决策；不能预先把它设为所有 Session 的默认成本。

### Prompt 与 Feedback Package

Prompt 是推动 Session 执行的输入；Feedback Package 是 Ramble 提交后的不可变证据。Launch Ramble 发布 `launch` Package 并从中派生首个 Prompt；Steering Ramble 只产生 Prompt；Feedback Ramble 发布绑定 `request_id` 的 `response` Package。Package 不是 Prompt 字符串的别名，三种 intent 在 UI、存储和测试中必须可区分。

### Ramble 提交幂等性

每次提交在跨进程副作用发生前固定 `submission_id` 和内容 digest：

- 同一 `submission_id` + 同一 digest 重试，返回同一个 Package、Session、Request resolution 和 Delivery 结果。
- 同一 `submission_id` + 不同 digest，返回冲突；不得覆盖已发布 Package。
- Launch 重试不得创建第二个 Session、第二个 `launch` Package 或重复发送首个 Prompt。
- Feedback 重试不得创建第二个 `response` Package、重复解决 Request 或创建新的 `delivery_id`。
- Steering 也复用相同提交规则，避免网络或进程重试重复发送 Prompt，尽管它不发布 Package。

进程启动和 Agent Prompt 无法与 SQLite 做单一原子事务，因此实现使用持久 launch/delivery intent、稳定 identity 和对账重试，不宣称底层副作用的 exactly-once。

### Permission、Ask Question 与 Feedback

Permission 是 ACP request 的语义透传，Ask Question 是短结构化问答，Feedback 是可跨 Prompt Turn 处理并产生不可变 Package 的持久流程。三者可以投影为 Attention Item 同列展示，但不得合并成一张带大量可空字段的通用请求表。

### Permission 透传边界

ACP 的 `session/request_permission` 携带 `sessionId`、tool call 和 Agent 给出的 permission options。RambleDesk 可以透传执行命令、读写文件、访问 workspace 外路径等请求，前提是 Agent/ACP adapter 确实为该动作发出 Permission Request；ACP 没有要求 RambleDesk 为所有工具主动推断或补造授权。

- 一个 active Prompt Turn 可以先后或并发产生多个 Permission Request；实现必须按 JSON-RPC request 关联分别挂起、排队、回答，不能只有一个全局 `pending_permission` 槽位。
- UI 展示 Agent 给出的 title、kind、raw input、location 和 options；`allow_once`、`allow_always`、`reject_once`、`reject_always` 的语义不得被改写成 RambleDesk 私有枚举。
- Prompt Turn 被取消时，所有尚未回答的 Permission Request 必须回 `cancelled`；Agent Run 断开后不得把旧请求作为仍可操作的 durable approval 恢复。
- YOLO 或 Agent 自己绕过 permission channel 时，RambleDesk 只能如实显示“没有收到请求”，不能宣称自己拦截了动作。文件系统 sandbox 和额外 workspace root 是单独的执行/Session 配置问题，不由 Permission UI 自动实现。

### Ask Question 与 Elicitation

Ask Question 是 RambleDesk 的领域术语；`ask_user_question` 是首选注入工具名。`elicitation/create` 只允许作为某个 Agent/Adapter 的技术输入：question-shaped 输入归一化为 Ask Question，approval-shaped 输入归一化为 Permission Request。产品、`core` 和 UI 不定义 Elicitation Request。

### Feedback Request Wait 与 Tool Ack

Feedback Request 的等待没有业务超时，且可以比发起它的 Prompt Turn 和 Agent Run 活得更久。Managed Session 中的 `request_feedback` 在持久化成功后快速返回 ack；这个 ack 不解决请求，只让 Agent 安全结束当前 Prompt Turn。因此无限等待由 RambleDesk 的持久状态保证，不依赖 MCP `tools/call` 一直占用连接。

持久的 Request 至少固定 `request_id`、`session_id`、可选的来源 ACP Session Link、Agent 提供的结构化 title/instructions/actions/context refs、`waiting | submitted | cancelled` 状态/决议及时间戳。人类打开编辑器或开始写作不把 Request 变成另一个 `in_progress` 状态；未完成的写作进度归 Ramble Draft，Request 仍是 `waiting`。

如果人类在 Request 进入 `waiting` 后退出 RambleDesk，该 Request 保持 `waiting`，ACP Session Link 保留，Agent Run 结束。下次打开时 Inbox 直接从持久数据恢复该请求；在人类作出决议前不启动 Agent Run，因为没有任何需要交付的新事实。

人类提交或取消时，RambleDesk 不做“先查 Agent Run 状态，再决定是否保存”的 check-then-act。它先在同一本地事务内固定 resolution、提交时的 Package 和 pending Delivery，再调用幂等的交付对账。因此“ACP Client 未启动”不是领域状态：App 打开时 ACP Client 组件已可用，真正可能缺失的是健康 Agent Run。

保底策略是 Managed Feedback Resume：交付对账先复用当前健康 Run；若不存在，则按 Launch Configuration 建立新 Run，优先 `session/resume`，失败或不支持再 `session/load`，最后才是 `session/new` + Recovery Prompt。恢复后发送 Feedback Resume Prompt，要求 Agent 调用 `get_feedback(request_id)` 获得 Feedback Delivery Envelope。退化到新 ACP Session 会损失 Agent 私有上下文，但不影响 Feedback Request 的无限等待和最终可交付性。直接把 Envelope 内联到 Feedback Resume/Recovery Prompt 可以减少一次工具调用，但不能取代 `get_feedback` 保底合同。

Feedback Delivery 采用稳定 `delivery_id` 的 at-least-once 语义：确认前失败可以重试，不宣称跨进程和模型上下文的 exactly-once。重复尝试必须携带同一个 envelope identity，不能重复发布 Package。

### Feedback Package 与 Artifact Locator

Feedback Package 的内容和 digest 决定“它是什么”，Artifact Locator 只决定“本次如何读取”。同一个 Package 可以从本地 blob store、远程对象存储或内联 envelope 交付，迁移存储位置不得改变 `package_id` 或 manifest 语义。

### Attention Item 与 Inbox

Attention Item 是 Inbox 的 read model，不是协议对象。Inbox 可以同时显示 Permission、Ask Question 和 Feedback，但每一项必须保留自身的操作、终态和身份字段。App 重启后只有 `waiting` Feedback Request 能从持久数据重建为 Attention Item；Permission Request 和 Ask Question 随原 Run 取消，不伪装成可回答的旧请求。Feedback 已提交但 Delivery 仍 pending 时，它已不在等待人类，应显示为 Session 的交付状态，而不是未回复 Inbox 项。

### Agent History 与 RambleDesk 事实

Agent Session History 回答“Agent 在完整会话里做过什么”；RambleDesk 的结构化记录回答“人类通过 RambleDesk 提交过什么、哪些反馈仍在等待、发布和交付了什么”。前者由 Agent 拥有，后者由 RambleDesk 拥有，不能为了 Context View 把两者合并成一份本地 transcript。

Permission Request 与 Ask Question 的 pending 状态只归属 live Agent Run；回答后不另存 RambleDesk 记录，Agent 是否把 tool call/result 写入自己的历史由 Agent 决定。Feedback Request 不同：它可以跨 Run 无限等待，因此必须持久化。

## 身份字段

| 字段 | 目标语义 | 规则 |
| --- | --- | --- |
| `session_id` | RambleDesk 的唯一持久 Session id。 | 所有 Session 路径共用；不得以进程 id、ACP id 或旧 host session id 代替。 |
| `session_kind` | Session 的创建与监督方式。 | 当前为 `managed` 或 `connected`；不是 transport 可用性状态。 |
| `agent_profile_id` | 稳定 Agent 家族 id。 | 用于用户识别和能力归属；不选择具体 executable。 |
| `launch_profile_id` | 具体启动配置 id。 | Managed Session 必须有；Connected Session 可以为空。 |
| `agent_run_id` | 一个 live Agent Run 的 runtime 关联 id。 | 不得以 OS pid 代替；用于进程、连接、Permission 和 Question 关联，不要求写入持久表。 |
| `acp_session_link_id` | RambleDesk 保存的一条 ACP Session Link id。 | 归属唯一 `session_id`；可以被多个先后发生的 Agent Run 使用。 |
| `acp_session_id` | ACP Agent 返回的不透明 Session id。 | 归属 ACP Session Link；可以跨 Agent Run load/resume，不是 RambleDesk 主键。 |
| `permission_request_id` | live Agent Run 中一个 Permission Request 的关联 id。 | 保留 ACP request 关联；不提升为跨 Run 的持久业务主键。 |
| `question_id` | 一个 Ask Question 的关联 id。 | 在所属 Agent Run 内关联展示与答案；Question Channel 可以使用自己的 wire id。 |
| `request_id` | Feedback Request 的唯一持久 id。 | 是 `response` Feedback Package 的关联与 lookup key；必须归属唯一 Session。 |
| `submission_id` | 一次 Ramble 提交的稳定幂等 id。 | 在任何外部副作用前生成；重试必须复用，不能用时间戳或进程 id 临时替代。 |
| `package_id` | Feedback Package 的位置无关持久 id。 | 不编码目录、机器、bucket 或 URL；同一提交重试及同一内容迁移存储位置后保持不变。 |
| `package_purpose` | Feedback Package 在 Session 中的用途。 | 当前为 `launch` 或 `response`；`response` 必须绑定 `request_id`，`launch` 必须绑定 Launch `submission_id`。 |
| `artifact_id` | Package 内一个 Artifact Entry 的稳定 id。 | 与 `package_id` 联合定位内容；不等于文件名或路径。 |
| `delivery_id` | 一个 Feedback Delivery 的稳定 id。 | 重试使用同一个 id；Envelope 必须携带它，让 Agent 和日志识别重复交付。 |
| `context_refs` | 可选上下文引用列表。 | 承载文件、URL、diff、截图等可读线索；不参与认证。 |

旧字段 `host_id`、`host_session_id` 只允许出现在旧数据迁移器的输入模型中。当前运行时、当前 DTO 和新表不得把它们保留为 alias 或 fallback。

## Ramble 与后处理规则

- 所有 Ramble Draft 都保存结构化 `document_json`；Markdown 只用于 Prompt 投影、提交、导出和历史展示。
- Launch Ramble 提交时把结构化内容的正式投影写入 `launch` Feedback Package，再从该 Package 构造首个 Prompt；Steering Ramble 的投影只构造后续 Prompt，不发布 Package。
- Tidy 与 Cooking 位于同一“后处理”设置页，但各自持有 provider、API Key、base URL、model、reasoning effort 和 system prompt；任一功能不得回退使用另一套配置。
- Tidy 没有自动开关、idle timer 或数量/字符阈值，只能由当前 Editor 的人工按钮触发。
- Cooking 只对 Feedback Ramble 可用，默认关闭，并使用独立的模型服务配置。
- API Key 是本机凭证，不属于 Session、Ramble、Feedback Package、日志或 Agent 协议。
- 启用 Cooking 时，`uncooked.md` 和 `feedback.md` 必须同时进入不可变 Feedback Package；关闭时两者内容可以相同。
- Cooking 失败不得丢失或锁死 Uncooked Feedback，也不得提交半成品 Feedback Package。

## 接入路径

### ACP Managed Path

`rambledesk-acp-client` 提供：

- Agent catalog、Launch Profile、Launch Preflight、Launch Configuration 的能力映射、本机探测和安装提示。
- Agent 子进程和进程树监督。
- ACP initialize、Session 创建/恢复、Prompt、cancel 和能力协商。
- ACP Permission Request 的多请求关联、排队、语义透传与回包。
- Session Toolset 的按 Run 注入、能力验收和 companion 生命周期。
- 原生或注入 Question Channel 到统一 Ask Question 体验的归一化。
- `response` Feedback Package 提交后的 pending Delivery 对账、`resume` → `load` → `new` 恢复梯子、Feedback Resume Prompt 和 `get_feedback` 保底调度。
- 当前 Context View 所需的 Live Session Event，以及 `session/load` replay 与 live update 的边界标记。

它不提供：

- Ramble Editor、Tidy、Cooking 或 Feedback Package 格式。
- Session、Feedback Request、Ask Question 或 Permission Request 的第二套领域模型。
- IDE 文件树、git/worktree 管理或内嵌终端主界面。
- 厂商私有历史库解析、多 Agent 编排或独立 daemon。
- “所有 Agent 都能无限阻塞 MCP tool”之类未经逐 Launch Profile 验收的保证。

### MCP Compatibility Ingress

MCP 继续支持外部 Agent 创建、读取和取消 Feedback Request。它只把调用映射到当前 `core` use case 和新表：

- 不再定义全局 Host Profile 或产品 Session 语义。
- 不保证恢复外部 Agent 的原上下文。
- 不拥有 listener、token path、本地 JSON API 或 ACP 生命周期。
- `get_feedback(request_id)` 返回 Feedback Delivery Envelope，不返回以绝对路径为合同的 Package；Managed Feedback Resume 也复用这份保底读取合同。
- 在过渡期可以保留 Compatibility Continuation 提示，但这只是兼容 UX，不是产品主闭环。

### 既有原生 Adapter

Pi 等既有原生 Adapter 在过渡期按 Compatibility Ingress 管理。若其 active tool call 已被验收为可长期等待，可以在终态直接返回 Feedback Delivery Envelope；这种能力只归属该 Adapter，不升级为所有 ACP Agent 的保证。

## Package 边界

| Package / 区域 | 职责 | 不应包含 |
| --- | --- | --- |
| `crates/rambledesk-core` | Session Record、Launch Configuration、ACP Session Link、Ramble Intent/Submission、Feedback Request、Feedback Package、Delivery 的领域 DTO、use case 与 ports，以及 Agent Run、Permission、Ask Question 的协议中立 runtime DTO。 | ACP/MCP/HTTP/JSON、进程管理、Agent transcript、Tauri command、厂商启动命令、绝对存储路径、旧表兼容。 |
| `crates/rambledesk-storage` | 当前数据模型的 SQLite Adapter、结构化 Draft、Session Record 及 Launch Configuration、Ramble Submission、Feedback Request/Package/Delivery，以及本地 content-addressed Artifact Store Adapter。 | ACP/MCP 协议、进程监督、Agent transcript mirror、Permission/Ask Question 历史表、旧表运行时读取、把本地路径泄漏为领域合同。 |
| `crates/rambledesk-acp-client` | ACP Client 实现、catalog、preflight、launch、Launch Configuration 能力映射、进程树、ACP Session Link、Permission 透传、Question Channel、Live Session Event 归一化、pending Delivery 对账和 Managed Feedback Resume 调度。 | Editor、Cooking、Feedback Package 格式、厂商私有 transcript parser、第二套 Session 存储、Svelte 状态。 |
| Session Tool Companion（目标 binary） | 每个 Agent Run 按需启动的 stdio MCP adapter，暴露 `ask_user_question`、`request_feedback`、`get_feedback` 并经认证 IPC 回到 RambleDesk。 | 领域真源、无限等待保证、Package 存储、本地路径合同、独立 daemon。 |
| `crates/rambledesk-local-server`（目标重写或移除） | 若保留，只提供 authenticated IPC/薄 Compatibility Ingress transport。当前 v2 routes 不属于目标合同。 | 领域规则、ACP 会话实现、MCP tool schema、旧 `/api/feedback/*`。 |
| `crates/rambledesk-mcp`（目标重写或移除） | 若未来保留，作为只调用新 Core Interface 的 External Compatibility Ingress。当前 v2 tool/install Implementation 不属于目标合同。 | Session 真源、listener、token path、ACP 生命周期、全局 Agent catalog、本地路径结果合同。 |
| `crates/rambledesk-hosts`（冻结后删除） | 只在过渡期提供旧外部工具探测；Launch Profile 知识迁入 ACP Client 后退出首发 workspace。 | 当前 Session 模型、MCP Implementation、ACP Client、完整 Adapter 流程。 |
| `packages/pi-rambledesk` | Pi Compatibility Ingress。 | 独立 Session 语义、desktop UI 状态、storage 逻辑。 |
| `packages/dsh-rambledesk` | 旧 DSH Compatibility Ingress；Phase 5 退出首发 workspace 或删除。 | v3 Managed Path、长期 Runtime 依赖。 |
| `apps/desktop` | composition root、Workbench UI、Tauri wiring、应用与托盘生命周期。 | 领域持久化真源、ACP JSON-RPC、厂商进程实现。 |
| 离线迁移工具 | 读取旧表、写入新表、输出迁移与损失报告。 | 被桌面运行时自动调用、双写、旧字段 fallback、长期兼容 API。 |

目标依赖方向：

- `rambledesk-core` 不依赖 workspace 内的协议或基础设施 crate。
- `rambledesk-storage` 和 `rambledesk-acp-client` 依赖 `rambledesk-core` 的 ports；二者不互相取得领域所有权。
- 未来重写后的 `rambledesk-mcp` 与既有 Adapter 只依赖当前 `core` Interface；当前 v2 Implementation 不满足此条。
- Session Tool Companion 是 `rambledesk-acp-client` 使用的 MCP adapter，不绕过 `core` use case 直接操作数据库。
- `apps/desktop` 负责组装 storage、ACP client、local server 和 compatibility ingress。
- ACP Agent SDK 只能出现在 `rambledesk-acp-client` 一侧，不能沿 DTO 泄漏到 Svelte 或 `core`。

## 数据代际与迁移规则

本次重构建立一个新的当前数据代际：

- 新 Session Record 及 Launch Configuration、ACP Session Link、Ramble Draft、Ramble Submission、Feedback Request、Feedback Package、Artifact Entry 和 pending Delivery 只写入新表。Agent Run、Prompt Turn、Permission Request、Ask Question 和完整 Agent transcript 不建持久表。
- 桌面应用启动和正常 use case 只打开当前 schema，不探测旧字段来改变行为。
- 不做新旧双写，不做 legacy view，不以 `COALESCE`、alias DTO 或反序列化 fallback 偷渡旧语义。
- 新表和当前 DTO 不保存绝对文件路径作为 Feedback Package 内容引用；本地实现通过 `package_id` / `artifact_id` 解析到自己的 blob store。
- 一次性迁移脚本可以把旧 request/session/draft/package 投影到新对象，并把仍可读的旧附件导入新 Artifact Store；无法读取或可靠恢复的信息可以丢弃或降级，但必须输出数量和原因。
- 迁移结果写入新表后，后续行为只按当前合同解释；旧表保留与清理由迁移操作决定，不属于应用运行时。
- 迁移器是可删除的交付工具，不是永久 Compatibility Ingress。

## 命名规则

UI 的核心名词：

- Agent
- Session
- Workspace
- Ramble
- Inbox
- Context
- Permission
- Ask Question
- Feedback Request
- Feedback Package

技术设置或诊断中允许：

- ACP Agent / ACP Client
- Agent Profile / Launch Profile
- Launch Preflight
- Launch Configuration / Access Mode
- Session Toolset / Question Channel
- Feedback Delivery Envelope
- Compatibility Ingress
- MCP

避免：

- 用“宿主（Host）”统称 Agent、Launch Profile、ACP Client 或 Session。
- 用“Provider”同时指 Agent 家族和模型服务；Agent 侧使用 Agent Profile，Tidy/Cooking 侧保留 Model Provider。
- 用 Elicitation Request 作为产品或 `core` 术语；协议输入应按语义归一化为 Ask Question 或 Permission Request。
- 把 Context View 称为第二个 Inbox、Chat 或 Editor。
- 把 Steering Ramble 描述为“生成反馈包”，或漏掉 Launch Ramble 的 `launch` Feedback Package。
- 把 Package 的绝对本地路径交给 Agent，或把路径写进 Package 身份。
- 把 Managed Session 的 Feedback Delivery 称为 Continuation。
- 把 `session/resume` 描述为历史回放，或假设所有 Agent 都支持 `session/load` / `session/list`。
- 把 transport 在线状态提升为产品全局状态。

## 合并标准

- 新 UI、DTO、表和测试以 Session 为顶层归属；Feedback Request 不再承担 Session 容器职责。
- Launch 表单优先使用 Launch Preflight 读到的 Agent config options，提供 Agent、model、工作目录、reasoning effort 和 Access Mode；用户选择作为 Launch Configuration 归属 Session Record，恢复已有 ACP Session 时不覆盖 Agent 返回的 live config。
- Launch、Steering、Feedback Ramble 在类型和提交 use case 上明确分开；Launch 与 Feedback 都发布 Feedback Package，但 `package_purpose`、关联和后续副作用不同。
- 所有 Ramble 提交以稳定 `submission_id` 幂等；Launch 重试返回同一 Package/Session，Feedback 重试返回同一 Package/Delivery，Steering 重试不重复 Prompt。
- Permission、Ask Question、Feedback 只在 Inbox read model 中汇合，不以可空字段模拟同一种持久请求。
- Permission Request 保留 Agent 给出的 tool call、raw input/location、选项与 JSON-RPC 关联，并把选择直接回给原 ACP request；多请求分别排队，取消 Prompt Turn 时全部回 `cancelled`；YOLO 模式不人为制造 Permission。
- `ask_user_question` 的长期等待能力按 Launch Profile 实测；不得从“支持 MCP”推导“支持无限阻塞 tool call”。
- Feedback Request 没有业务超时，可以跨 Prompt Turn 和 Agent Run；MCP request timeout 不得把它自动取消。
- Managed `request_feedback` 的 ack 不解决 Feedback Request；App 退出后未回复 Request 仍为 `waiting` 并在重启后回到 Inbox，但不因等待而启动 Agent Run。
- 人类提交或取消 Feedback 时，必须先在单一本地事务内固定 resolution、提交时的 Package 和稳定 pending Delivery，不以 Agent Run 在线为保存前提；已提交但未交付的项目不再算未回复 Inbox 项。
- pending Delivery 对账按健康当前 Run → 新 Run + `session/resume` → `session/load` → `session/new` + Recovery Prompt 的顺序恢复，再以 Feedback Resume Prompt + `get_feedback(request_id)` 交付；本次未成功必须在下次 App 启动时使用同一 `delivery_id` 继续对账。直接内联 Envelope 等策略只能做等价优化。
- Feedback Delivery 重试保持同一个 `delivery_id` 和 `package_id`，不得因不确定结果重复发布 Package。
- 当前运行时不存在 `host_id` / `host_session_id` alias、旧表读取、双写或新旧 fallback。
- ACP 是主路径；MCP/Pi 只能把外部输入映射到同一 `core` 合同和新表。
- `core` 不出现 ACP JSON-RPC、stdio、MCP、HTTP、Tauri command、进程树或厂商安装逻辑。
- Feedback Package、manifest、DTO 和 Agent tool result 不以绝对本地路径作为内容合同。
- Svelte 不接触 ACP wire payload；UI 只消费 Session、Live Session Event、Capability 和 Attention Item DTO。
- 退出 App 后没有 RambleDesk 启动的残留 Agent 子进程；托盘期间 live Agent Run 可以继续。
- 重启后从 Session Record/Launch Configuration、ACP Session Link、Draft、Ramble Submission、Feedback Request、Package 和 pending Delivery 恢复；Permission、Ask Question 和 Live Session Event 不作为跨 Run 状态恢复。`waiting` Request 只恢复 Inbox，pending Delivery 才触发交付对账。
- `session/load` 支持时可回放历史，`session/resume` 只恢复上下文；完整历史的可见性允许依赖 Agent capability 和保留策略，RambleDesk 不复制一份兜底 transcript。
- Launch 与 Feedback Ramble 的提交终态发布 Feedback Package；Steering/Context View 的普通发送不会生成密封包。
