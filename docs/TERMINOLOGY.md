# RambleDesk 术语表

> 状态：v7 当前基线，包含 ACP 托管会话。
> 目标：固定产品语言、协议字段和 package 边界。代码、文档、UI 文案、测试命名若与本文冲突，以本文为准。

本文是 RambleDesk 的唯一术语源。其他文档只引用本文，不重新定义产品对象。

**CURRENT** 表示当前实现；**TARGET** 表示已接受、尚待实现的边界。托管会话、Agent 启动配置、
ACP 实例及持久投递现为 CURRENT；具体后端支持仍以版本化实测为准，见 [使用指南](ACP_MANAGED_SESSIONS.md)。
决策和提交顺序见 [ADR 007](adr/007-acp-managed-sessions.md) 与 [ACP 提交地图](ACP_COMMIT_MAP.md)。

## 架构公理

1. RambleDesk 是本地 human-feedback workbench，不实现智能体的推理与工具执行引擎，不内置 shell multiplexer，也不持有源码 checkout 模型；Backend Runtime 托管外部 Agent 的会话与启动资源。
2. 反馈请求和反馈包构成核心闭环；请求、Feedback Draft、反馈包、配置以及 Session Runtime / Activity 等业务事实只由 Backend Runtime 持有。
3. 反馈适配器负责请求、反馈读取与 continuation；Agent Session Management 负责启动、交互和会话生命周期。两者是独立职责，可以组合使用。
4. `core` 只持有 application contract，不持有 HTTP、JSON、MCP、ACP wire/SDK、Pi、Local Integration Server、Web Access、desktop command 或宿主安装逻辑。
5. Workbench Client 通过 Application Transport Interface 访问 Backend Runtime；Transport 与设备 Capability 是两个独立边界。
6. Local Integration Server 与 Web Access 必须复用一套安全 policy/primitives，但拥有显式分离的 listener、credential、auth domain、启停生命周期和 route set。
7. Workbench Client 的 workspace snapshot（view、顺序、active view、pane 尺寸）是 client-local 状态，不是 Backend Runtime 业务事实，也不得缓存 Feedback Draft 正文。
8. MCP 是通用 MCP 适配器的一种 transport，不是全局基础设施。
9. 提交后的 continuation 不是适配器。反馈路径可以选择“不需要 continuation”“手动 continuation”“原生 continuation”或“托管 continuation”。
10. 外部反馈请求不要求源码 checkout 路径，路径可以是可选 context hint；托管会话必须指定 Backend Runtime 所在机器上的工作目录 `cwd`，它不建立源码 checkout 管理模型。
11. 语音识别与屏幕采集发生在输入所在的客户端设备；Platform Plugin 只把结构化转录事件或附件候选交给 TipTap Ramble Core，不通过 Application Transport 代理设备能力。

## 核心闭环

1. 宿主中的智能体通过适配器创建反馈请求。
2. RambleDesk 持久化请求，并在工作台展示。
3. 人类在工作台的 TipTap Feedback Draft 中检查上下文和书写反馈；当前平台的语音、截图等 Platform Plugin 可以向同一文档提供结构化输入。
4. RambleDesk 发布不可变反馈包。
5. 适配器或 continuation 让原宿主读取反馈包并继续。

## 核心术语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| 人类 | 使用 RambleDesk 产生真实反馈的人。 | 拥有产品判断；不拥有协议状态。 |
| 智能体 | 发起反馈请求并读取反馈包继续工作的 LLM coding actor。 | 拥有任务推理；不拥有 RambleDesk 持久状态。 |
| 宿主 | 智能体运行所在的 runtime/container，例如 Pi、Claude Code、Codex、OpenCode。 | 拥有自己的 session、tool、plugin API；不定义 RambleDesk 存储合同。 |
| 工作台 | RambleDesk 的人类反馈工作界面；同一套工作台可以由不同 Workbench Client 呈现。 | 拥有人类反馈工作流；不实现宿主协议，不限定为桌面窗口。 |
| Workbench Client（工作台客户端） | 承载共享工作台 UI 的客户端角色；当前由 `apps/desktop` 中的 Svelte UI 实现，并由 Desktop Client 与 loopback Web Client 复用。 | 只持有 UI 投影和 client-local workspace snapshot；不拥有 Request、Feedback Draft 或 Package 的 canonical 事实。 |
| Desktop Client（桌面客户端） | 在 Desktop Shell 内运行的 Workbench Client，通过 Tauri IPC 的 Application Transport Implementation 访问 Backend Runtime。 | 是当前已实现的客户端；不把 Tauri API 暴露为共享 UI 的业务合同。 |
| Web Client（Web 客户端） | 在浏览器中运行的 Workbench Client，通过 Web Access 的 HTTP + WebSocket Application Transport Implementation 访问 Backend Runtime。 | 当前支持仅 loopback 的 Request/Session、TipTap Draft、上传/图片粘贴、提交/下载，并提供浏览器本地 ASR pilot；真实 Chrome/Safari 麦克风、PCM 与稳定出字仍需人工验收，浏览器屏幕采集和桌面原生能力不在当前支持面。 |
| Backend Runtime（后端运行时） | 长期持有 application use cases、storage、配置以及 Session Runtime / Activity 的 Rust 运行角色。 | 是业务事实唯一来源；当前由 desktop composition root 组装，不等同于 HTTP listener，也不预设一个新 crate。 |
| Application Transport（应用传输） | Workbench Client 调用 application command/query、订阅变化、等待 ready 并读取 capability manifest 的 Interface。 | Tauri IPC 与 HTTP + WebSocket 是不同 Implementation，但调用同一 Backend Runtime application Module；`capabilities` 只报告可用性，不执行设备能力。 |
| Local Integration Server（本地集成服务） | 为 Generic MCP、Pi 等 Host Adapter 提供 authenticated loopback listener、JSON API、route mounting 和 guard 的 transport Module。 | 服务宿主集成，不拥有领域语义；其启停和 route set 独立于 Web Access。 |
| Web Access（Web 访问） | 可选、默认关闭的浏览器访问能力，通过独立 loopback listener 向 Web Client 提供静态资源、HTTP 与 WebSocket。 | 当前固定监听 `127.0.0.1:37643`；关闭它不停止 Backend Runtime 或 Local Integration Server。LAN、TLS、autostart、可配置端口与 headless 均不在当前支持面。 |
| Desktop Shell（桌面壳层） | Tauri 进程、窗口、托盘、更新器、原生权限和 desktop composition root。 | 组装 Desktop Client、Backend Runtime 与本地能力；不拥有 UI 业务事实。 |
| Native Capability（原生能力） | 由 Desktop Shell 提供的 OS / device Implementation，例如全局快捷键、系统截图、原生录音、托盘、更新器和原生对话框。 | 独立于 Application Transport；可访问的设备和权限范围不能被 Web Client 假定。 |
| Browser Capability（浏览器能力） | 由浏览器 API 提供的 device-scoped Implementation，例如受权限和用户手势约束的媒体、剪贴板、文件选择、下载和通知。 | 独立于 Application Transport；受 secure context、浏览器、权限与当前设备限制，不等价于 Native Capability，也不代表服务器文件系统。 |
| Platform Plugin（平台插件） | 在一个 Workbench Client 平台内组合设备 Capability、权限、资源和生命周期的深 Module。 | 不是 Host Adapter，也不表示可动态安装的第三方扩展；不经 Application Transport 代理设备操作，不拥有 Feedback Draft。 |
| Speech Recognition Plugin（语音识别插件） | 在当前客户端设备内组合 Audio Source、重采样、VAD、Speech Engine 与模型管理，并产生统一 SpeechEvent 的 Platform Plugin。 | 原始音频、识别 session 和模型不进入 Application Transport；平台共享事件语义，不共享同一个引擎进程。 |
| Capture Plugin（采集插件） | 在当前客户端设备上取得截图、相机、粘贴或文件输入，并返回 Attachment Candidate 的 Platform Plugin。 | 不直接编辑 TipTap，不写最终附件路径；平台可以有不同 acquisition UX。 |
| Attachment Candidate（附件候选） | Platform Plugin 交给共享 Draft 流程验证和持久化的客户端本地 bytes/Blob、MIME 与来源 metadata。 | 不是已持久化附件，也不是服务器路径；只有 application mutation 成功后才能成为 Feedback Draft 附件引用。 |
| Audio Source（音频源） | Speech Recognition Plugin 内负责取得有明确 sample rate 的本地单声道 PCM 的 Interface/Implementation。 | 不执行语音识别、不传输到 Backend Runtime、不拥有 Feedback Draft。 |
| Speech Engine（语音识别引擎） | Speech Recognition Plugin 内消费本地 PCM 并产生 SpeechEvent 的识别 Implementation。 | Desktop、Browser 与 Mobile 各自在本设备运行；统一点是事件合同，不是进程、模型或 transport。 |
| 反馈请求 | 由适配器创建、由人类处理的持久单位，用 `request_id` 标识。 | RambleDesk 的核心输入事实。 |
| 反馈包 | 请求进入终态后发布的不可变证据，包含 manifest、markdown、附件路径和 hash。 | RambleDesk 的核心输出事实；宿主继续前必须读取。 |
| Feedback Adapter（反馈适配器，简称适配器） | 面向一类宿主的完整反馈接入流程：创建请求、读取反馈、处理 continuation。 | 可以由多个 package 或 transport 组成；不因此拥有 Agent 会话启动与进程管理职责。 |
| continuation | 请求进入终态后，让原宿主继续的行为。 | 只处理终态之后；不创建请求，不发布反馈包。 |
| Agent Backend（智能体后端，现有文档称宿主 / Host） | 提供智能体推理、工具和会话能力的外部软件，例如 Pi、dsh、Codex。 | 不等于 RambleDesk 的 Backend Runtime，也不等于某一个 OS 进程。 |
| Agent Session（智能体会话，现有文档称宿主会话） | Agent Backend 中持续关联的一段对话与执行上下文。 | 可处理多个任务、产生多次反馈请求；任务切换不自动创建新会话。 |
| context hint | 适配器可选提供的展示/定位信息，例如标题、路径、URL、文件引用。 | 不参与认证，不是必需身份字段，不保证可恢复。 |
| Ramble | 以 TipTap Feedback Draft 为中心的自由反馈编辑流程；文字是基础输入，语音、截图等 Platform Plugin 可向同一文档贡献结构化内容。 | 不等同于录音 session，不拥有平台设备能力，也不属于适配器协议。 |
| Uncooked Feedback | 人类通过 Ramble、文字、截图形成的原始反馈正文；允许保留口语、重复和自我修正。 | 是人类原始证据，Cooking 不得覆盖；提交后保存为反馈包中的 `uncooked.md`。 |
| Feedback Draft | 当前未提交请求的可编辑正文。canonical 真源是版本化 TipTap `document_json`，`body_markdown` 是同一份文档的派生投影。 | 不得把 Markdown 当作第二真源独立维护。 |
| Action Group | 用标准 Blockquote 表达的 `@Action` 归属容器。 | 同一 Action 再次打开时创建新容器，不与旧区间合并。 |
| Tidy | 人类在当前 Editor 中手动触发的 ASR 段落整理。 | 只处理 pending 语音节点；后台文档不整理；不是 Cooking。 |
| Cooking | 提交前可选的大模型编辑步骤，把 Uncooked Feedback 整理为正式 Markdown。 | 只做表达整理，不得编造事实、测试结果或删除负面判断；不开启时不调用模型服务。 |
| Cooked Feedback | Cooking 生成并经人类选择提交的正式反馈正文。 | 保存为反馈包中的 `feedback.md`，是宿主默认读取的反馈结果；其来源必须可追溯到 `uncooked.md`。 |

## 会话与 ACP 术语

| 术语 | 定义与边界 |
| --- | --- |
| RambleDesk Session（RambleDesk 会话） | 工作台中持久化的会话单位，以 `host_sessions.id` 标识。托管会话可在零反馈请求时独立创建、展示，并对应一个 Agent Session。 |
| External Session（外部会话） | Agent Session 的启动与交互由外部宿主管理，RambleDesk 通过反馈适配器协作。历史会话按此方式保留，与托管会话并存。 |
| Managed Session（托管会话） | RambleDesk 通过 Agent Session Management 创建和控制的会话。它是 RambleDesk Session 的管理方式，不是另一种反馈请求。 |
| Agent Session Management（智能体会话管理） | Backend Runtime 的 application 能力：创建会话、发送输入、处理权限、取消当前执行、停止运行、删除与恢复。ACP Client 是首个协议实现。 |
| Task（任务） | 人类或 Agent 希望完成的一项工作；一个 Agent Session 可以连续处理多个任务。首期不要求建立独立 Task 存储模型。 |
| Turn（执行轮次） | Agent 接收一次输入后的一轮执行；一个任务可跨多轮，一个会话可处理多项任务。取消当前轮次不等于删除会话。 |
| ACP（Agent Client Protocol） | 工作台与 Agent 之间的交互协议。它不替代 RambleDesk 的反馈请求、草稿和反馈包合同。 |
| ACP Client（ACP 客户端） | 发起 ACP 初始化、会话和输入操作，并处理 Agent 回调的协议角色，由 RambleDesk Backend Runtime 中的协议实现承担。它不是 Workbench Client。 |
| ACP Server / Agent（ACP 服务端） | 对 ACP Client 暴露 Agent 能力的一端，可以由 Agent Backend 原生提供，也可以由 ACP Bridge 提供。此处 server 不表示必须有常驻 daemon 或网络监听端口。 |
| ACP Bridge（ACP 桥接程序） | 把某个 Agent Backend 的原生接口转换为 ACP 的外部程序，例如 `pi-acp`。上游可能称其为 adapter；RambleDesk 文档使用 bridge，避免与反馈适配器混淆。 |
| Agent Launch Configuration（Agent 启动配置，简称 AgentConfig） | 可复用的启动选择，包含后端家族、协议、启动命令/参数/环境等配置，由 `agent_config_id` 标识。同一后端可以有多个配置；它不是 Host Profile，也不是一个会话或进程。 |
| ACP Connection（ACP 连接） | 一次 ACP 协议通信通道，当前使用 stdio；断开后原连接身份失效。它不等于 Agent Session 的持久身份。 |
| ACP Instance（ACP 实例） | RambleDesk 管理的一次 ACP 启动及连接资源集合。每个托管会话独占一个实例；实例可包含桥接进程及其子进程，不保证只有一个 OS 进程。 |
| Session Runtime（会话运行状态） | Backend Runtime 根据当前连接和执行情况产生的投影，例如连接中、空闲、执行中、等待权限、断开。不得把上次落盘的 connected 状态当作重启后的事实。 |
| Session Recovery（会话恢复事实） | 最近运行及未完成轮次的持久检查点，区分 never_started、unclosed、stopped、interrupted。unclosed 不证明仍在线；启动恢复会把遗留运行与未完成轮次记为中断。 |
| Feedback Delivery（反馈投递） | 以 `request_id` 为稳定身份的终态投递记录，关联托管会话，用于排队、去重与恢复。delivered 表示续接轮次成功结束或用户确认已处理，不表示任务完成；uncertain 只由用户显式重试或确认。 |

关系约束：

- CURRENT 托管路径：一个 RambleDesk Session 对应一个 Agent Session；一个 Agent Session 可执行多个 Task、
  多个 Turn，并创建多个 Feedback Request。任务拆分不能成为另建 RambleDesk Session 的理由。
- CURRENT：Generic MCP 的关联 id 由调用者提供，历史数据不能据此证明真实 Agent Session 的一一关系；
  保留已有分组，不在术语更新中合并或改写历史。托管路径由 controller 固定反馈归属，不能由模型自由选择。
- 首期托管策略为一个会话独占一个 ACP Instance；ACP 协议是否支持连接内多会话，与本产品首期是否共享实例是两件事。
  后续共享属于实例分配策略，不改变会话的一一关系。
- View / Tab 是客户端视图；关闭视图不停止实例或删除会话。删除托管会话是一条直接操作，内部负责停止与清理，
  不要求人类先结束或归档。停止运行并保留历史可以是另一条操作。
- 不用“Ramble session”同时指代会话、反馈请求与 Ramble 编辑流程；分别使用完整术语。

## Cooking 规则

- Tidy 与 Cooking 位于同一“后处理”设置页，但各自持有 provider、API Key、base URL、model、reasoning effort 和 system prompt；任一功能不得回退使用另一套配置。
- Tidy 没有自动开关、idle timer 或数量/字符阈值，只能由当前 Editor 的人工按钮触发。
- Cooking 默认关闭，由人类在后处理设置中显式启用并配置自己的模型服务、模型和 API Key。
- API Key 是本机凭证，不属于反馈请求、反馈包、日志或宿主协议。
- 启用 Cooking 时，`uncooked.md` 和 `feedback.md` 必须同时进入不可变反馈包；关闭时两者内容可以相同。
- `feedback.md` 是宿主默认消费的正式结果，`uncooked.md` 是审计与恢复所需的原始人类证据。
- Cooking 失败不得丢失或锁死 Uncooked Feedback，也不得提交半成品反馈包。
- “Cooking”专指反馈编辑步骤，不指语音转录、反馈包发布或宿主智能体继续。

## 身份字段

| 字段 | 目标语义 | 规则 |
| --- | --- | --- |
| `request_id` | 唯一持久反馈请求 id。 | 创建幂等 key，也是读取反馈包的 lookup key。 |
| `host_id` | 稳定宿主家族 id，例如 `pi`、`claude`、`codex`、`opencode`、`grok`、`generic`。 | 用于展示、host profile 匹配和 continuation strategy 选择。 |
| `host_session_id` | 宿主提供或适配器生成的会话关联 id。 | CURRENT 外部反馈合同；不保证等于 Agent 的真实 session id，不是 MCP transport session id、认证凭据或自动恢复证明。 |
| `context_refs` | 可选上下文引用列表。 | 承载文件、URL、diff、截图等可读线索。 |
| `source_hint` | 可选来源提示。 | 可包含路径或标题；不得成为创建请求的硬前提。 |

结论：

- `host_id` 是宿主身份字段。
- `host_session_id` 是宿主会话关联字段。
- 同一宿主会话的多次 request 通过 `(host_id, host_session_id)` 收敛。
- 外部反馈合同不要求源码 checkout 地址；托管会话的 `cwd` 属于执行配置。

CURRENT 会话字段类别：

| 类别 | 字段方向 | 规则 |
| --- | --- | --- |
| 本地身份 | `session_id` | 暴露现有 `host_sessions.id` 的稳定身份；不会随连接重建而改变。 |
| 管理方式 | typed `management`：`external` 或 `managed` | `managed` 分支包含 `protocol: acp`、`agent_config_id`、`cwd` 和可空的 `remote_session_id`；不增加含义不清的 `acp_manage` 布尔值，也不同时存重复的布尔值与枚举。 |
| Agent 会话绑定 | `remote_session_id` | ACP 返回的 Agent Session id，可持久化用于受能力约束的恢复；不作为 RambleDesk 主键或全局唯一 id。创建完成前允许为空。 |
| 启动实例绑定 | `runtime.instance_id` | 当前实例的运行时身份；重启可变化，不表示可恢复的进程句柄。 |
| 可用性与运行投影 | `runtime`，分别表达连接状态与执行状态 | 不与 Request 的 waiting/completed 等业务状态混用；UI 状态来自 Backend Runtime。 |
| 反馈投递 | 独立 `FeedbackDelivery` 记录 | 以 `request_id` 关联会话与终态，状态为 pending / sending / delivered / uncertain / discarded，不把投递状态塞进 `management`。 |
| 删除意图 | `deleting` 投影与持久 deletion intent | 在清理前落盘，失败或重启后仍可重试；成功删除时随所属会话清理。 |
| 运行检查点 | 独立 `SessionRecovery` 记录 | 以 run/turn id 限定写入归属；记录历史事实，不能代替实时连接状态。 |

`agent_config_id` 指向可复用启动配置；`cwd` 固定在具体会话。配置编辑仅影响后续启动，不能静默改变正在运行的实例。
配置参数与环境变量值以结构化数据保存在本地 SQLite；界面隐藏和日志脱敏不表示加密凭据库。已绑定的
`remote_session_id` 只能通过 resume/load 恢复，失败不能静默创建空白替代会话。

## 适配器分类

### 通用 MCP 适配器

默认通用路径，面向能调用 MCP tools、但不能被外部可靠恢复原上下文的宿主。

包含：

- MCP tools：`request_feedback`、`get_feedback`、`cancel_feedback`。
- 宿主检测与配置写入执行引擎（per-host 知识来自 `rambledesk-hosts` 注册表）。
- 终态后的手动 continuation 提示；宿主提供原生交互确认工具（`ask`/`ask_choice` 类）时，可让智能体在工具调用内等待人类点选，点选后直接 `get_feedback` 继续。

不包含：

- blocking wait tool。
- 自动继续原宿主会话的产品保证。
- 把一次性 CLI 探针声明成正式能力。

流程：

1. 宿主调用 `request_feedback`。
2. 智能体结束当前 turn。
3. 人类提交或取消。
4. 人类按恢复提示回到宿主。
5. 智能体调用 `get_feedback(request_id)` 并继续。

### Pi 原生适配器

Pi 原生适配器是 `packages/pi-rambledesk`，通过本地 JSON API 工作。

包含：

- Pi tools：`request_ramble_feedback`、`get_ramble_feedback`。
- 调用本地 JSON API：`/api/feedback/request|get|wait|cancel`。
- 在 Pi tool call 内等待终态。

Pi 原生适配器不需要提交后的 continuation，因为 Pi 已经在工具调用中等待，终态反馈会直接返回原 Pi 流程。

### dsh 原生适配器

`packages/dsh-rambledesk` 是 CURRENT 的 dsh 原生反馈适配器，通过本地 JSON API 在同一 tool call
内等待并返回反馈，也提供中断后的恢复读取。它与 dsh ACP 启动配置承担不同职责，不能互相替代名称。

### 未来原生适配器

只有当宿主提供可靠、已验收的原上下文保留/恢复方式时，才允许新增原生适配器。

合格形式：

- 宿主 package/plugin/extension 能在 active tool call 内等待。
- 宿主提供 continuation registration API。
- 宿主 resume API 被证明会继续目标上下文，而不是创建相邻 transcript。

不合格形式：

- 只能向某个 CLI conversation 发文本，但原可见宿主不继续。
- 最佳努力进程 poke。
- 无安装模型、无失败模型的一次性探针。

## continuation

| 类型 | 含义 | 使用场景 |
| --- | --- | --- |
| 无提交后 continuation | 适配器已在 active tool call 内等待，终态直接返回。 | Pi、dsh 原生适配器。 |
| 手动 continuation | 显示恢复提示，让人类回宿主调用 `get_feedback`。 | 通用 MCP 适配器。 |
| 原生 continuation | 由宿主官方能力安全恢复原上下文。 | 未来原生适配器。 |
| 托管 continuation | Backend Runtime 在目标 Agent Session 可接收输入后投递反馈续接消息。 | ACP 托管路径；使用持久投递记录，结果不明时需显式人工处理。 |

## Package 边界

下表描述当前代码位置；架构角色不等于新 package 规划。特别是 Backend Runtime 是当前由 desktop composition root 组装的运行角色；Web Client 与 Web Access 复用现有 `apps/desktop` 与 server Module，本文不据此虚构新 crate。

| 架构角色 | 当前映射 | 目标边界 |
| --- | --- | --- |
| Backend Runtime | 由 `apps/desktop` composition root 组装 `core`、storage、配置和运行时 controller。 | 保持单一 application Module 和业务事实来源；是否重排 crate 留给后续实现决策。 |
| Agent Session Management / ACP Client | `core/sessions` 提供 application 能力；`crates/rambledesk-acp` 实现 stdio driver；Desktop 与 Web 共用管理入口。 | ACP wire/SDK 与进程资源留在实现库，Backend Runtime 组装并持有生命周期。 |
| Workbench Client | `apps/desktop` 中的 Svelte 工作台 UI。 | Desktop Client 与 Web Client 复用同一 UI 和 Application Transport Interface。 |
| Desktop Client / Desktop Shell | `apps/desktop`。 | Shell 只保留 desktop composition 与 Native Capability；共享 UI 不依赖 Tauri 细节。 |
| Tauri Application Transport Implementation | `apps/desktop` 的 Tauri command/event wiring。 | 实现统一 Application Transport Interface，调用同一 Backend Runtime application Module。 |
| Local Integration Server | `crates/rambledesk-local-server`。 | 继续服务 Host Adapter；不因 Web Access 启停而停止；与 Web Access 共享同一安全 policy/primitives。 |
| Web Client / HTTP + WebSocket Application Transport Implementation / Web Access | `apps/desktop` 中的共享 Svelte UI、browser composition/auth gate/HTTP Transport，以及 `crates/rambledesk-local-server` 中的独立 Web Access server Module。 | 复用同一 Backend Runtime 与安全 policy/primitives，并与 Local Integration Server 分离 listener、credential、auth domain、生命周期和 route set。 |
| Native Capability Implementation | `apps/desktop` 及 desktop-only crates。 | 与 Application Transport 分离并通过 capability manifest 暴露可用性。 |
| Browser Capability Implementation | `apps/desktop` 当前实现浏览器 file picker、image paste、download 与本地 sherpa-onnx WASM ASR pilot；系统截图、全局快捷键、tray、updater、原生窗口和系统路径操作明确不可用。 | Browser ASR 自动化已覆盖模型/runtime/recognizer 合同，但真实 Chrome/Safari 麦克风、PCM 与稳定出字仍需人工验收；Browser screen capture 延后，不模拟原生或服务器文件系统语义。 |
| Platform Plugin Implementation | `apps/desktop` 的 capability registry、Desktop Tauri Implementation 与 Browser Implementation；`rambledesk-speech` 是 Desktop Speech Recognition Plugin 的内部实现。 | 每个平台本地处理设备输入；只向共享 TipTap Ramble Core 输出 SpeechEvent 或 Attachment Candidate。 |

| Package / 区域 | 职责 | 不应包含 |
| --- | --- | --- |
| `crates/rambledesk-core` | 领域 DTO、application use cases、反馈请求/反馈包、托管会话与投递合同。 | HTTP、JSON、MCP、ACP wire/SDK、Pi、desktop commands、host install、Local Integration Server、Web Access。 |
| `crates/rambledesk-acp` | ACP SDK、stdio、能力协商、协议事件与所属进程资源。仅依赖 core 领域合同。 | SQLite、Tauri、HTTP 路由、反馈适配器实现与产品持久化规则。 |
| `crates/rambledesk-storage` | SQLite 持久化、请求/草稿/附件 metadata、会话/配置/活动/投递/恢复事实、反馈包发布与所属文件清理。 | 宿主协议、适配器安装、源码 checkout runtime 语义。 |
| `crates/rambledesk-local-server` | 实现 Local Integration Server，并提供独立可组合的 Web Access server、session auth、静态资源与 application/event routes。 | 领域规则、MCP tool schema、Pi package 代码；两个 listener 的 credential、auth domain、route set 与生命周期不得合并。 |
| `crates/rambledesk-mcp` | Generic MCP Adapter 完整方案：MCP schema、tool handler、instructions、结果/错误映射、客户端检测/安装执行引擎。 | listener、token path、JSON API、host-specific continuation、per-host 知识。 |
| `crates/rambledesk-hosts` | 宿主知识注册表（executable/marker/配置路径/ConfigFormat）、Host profile、展示元数据、默认适配器选择、continuation strategy。 | MCP implementation、Pi package、适配器安装/写入执行逻辑。 |
| `packages/pi-rambledesk` | Pi 原生适配器 package。 | MCP client 行为、desktop UI 状态、storage 逻辑。 |
| `packages/dsh-rambledesk` | dsh 原生适配器 package。 | ACP Client、desktop UI 状态、storage 逻辑。 |
| `apps/desktop` | 当前实现 Workbench Client、Desktop Client、Desktop Shell、Tauri Application Transport Implementation、composition root 和适配器安装 UX。 | 领域存储语义、host package 内部实现；共享 Workbench Client 不应直接依赖 Tauri 或 Native Capability 细节。 |

目标 Cargo 依赖方向：

| Package | 允许依赖 |
| --- | --- |
| `rambledesk-core` | 无 workspace 领域依赖。 |
| `rambledesk-storage` | `rambledesk-core`。 |
| `rambledesk-acp` | `rambledesk-core`；ACP SDK、stdio 与 OS 进程管理依赖位于此处。 |
| `rambledesk-mcp` | `rambledesk-core`、`rambledesk-hosts`。 |
| `rambledesk-local-server` | `rambledesk-core`、`rambledesk-mcp`。 |
| `rambledesk-hosts` | `rambledesk-core`；宿主知识注册表与续接策略共用其类型。 |
| `apps/desktop` | `rambledesk-core`、`rambledesk-acp`、`rambledesk-storage`、`rambledesk-local-server`、`rambledesk-hosts`、`rambledesk-mcp`、desktop-only crates。 |
| `packages/pi-rambledesk` | 不参与 Cargo workspace；运行时调用 Local Integration Server `/api`。 |

## Host Profile

`rambledesk-hosts` 的基本单位是 Host Profile。

Host Profile 描述：

- `host_id`
- label / icon
- 默认适配器
- continuation 模式
- 安装入口

当前 profile：

| Host | 默认适配器 | continuation 模式 |
| --- | --- | --- |
| `generic` | 通用 MCP 适配器 | 手动 continuation |
| `claude` | 通用 MCP 适配器 | 手动 continuation |
| `codex` | 通用 MCP 适配器 | 手动 continuation |
| `opencode` | 通用 MCP 适配器 | 手动 continuation |
| `cursor` | 通用 MCP 适配器 | 手动 continuation |
| `gemini` | 通用 MCP 适配器 | 手动 continuation |
| `antigravity` | 通用 MCP 适配器 | 手动 continuation |
| `grok` | 通用 MCP 适配器 | 手动 continuation |
| `inspector` | 通用 MCP 适配器 | 手动 continuation |
| `reasonix` | 通用 MCP 适配器 | 手动 continuation |
| `pi` | Pi 原生适配器 | 无提交后 continuation |
| `dsh` | dsh 原生适配器 | 无提交后 continuation |

上表是反馈适配器的默认选择，不是某个会话的管理方式。AgentConfig 独立描述 ACP 启动方式；
同一 `host_id` 可以同时有外部会话和托管会话。

## 命名规则

### Agent 管理扩展（2026-09-04）

- **Agent Catalog / 智能体目录**：可选择的 Agent 定义，包含名称、分发入口、推荐版本和能力说明；不是已安装清单。
- **Agent Installation / 智能体安装**：当前机器上的程序、版本和安装位置；ACP Bridge 与厂商 Agent 可分别需要安装。
- **AgentConfig / Agent 配置**：用户保存的启动选择、后端配置与环境；可以由目录和安装结果生成，高级用户也可手动填写。
- **Conversation Content / 对话内容**：有序的用户/Agent 消息及文本、思考、工具等内容块；流式更新修改所属内容，不创建新的 RambleDesk 会话。

上述能力按 [Codeg 移植地图](CODEG_ADOPTION_PLAN.md) 扩展，不改变反馈适配器及会话/轮次/请求的定义。

UI 文案允许：

- “适配器”
- “通用 MCP 适配器”
- “Pi 原生适配器”
- “dsh 原生适配器”
- “Agent 配置”“托管会话”“停止运行”“删除会话”
- “检测到的 Coding 工具”
- “手动继续”
- “桌面客户端”与“Web 客户端”
- “Web 访问”
- “平台插件”

UI 文案避免：

- 将 transport 可用性提升为产品全局状态。
- 在 titlebar 或 sidebar 放全局 transport 指示器。
- 用 “Adapter” 指代宿主图标或宿主 label。
- 用 “Adapter / 适配器”指代 Application Transport 或 Native / Browser Capability；它们分别称为 Interface 与 Implementation。
- 把 Backend Runtime 称为“Web Server”，或用“关闭 Web”暗示停止 Backend Runtime / Local Integration Server。
- 把 Browser Capability 描述成 Native Capability 的等价实现，或把浏览器文件选择描述成服务器工作目录选择。
- 把 Platform Plugin 描述成 Host Adapter，或把第一方、静态装配的 Platform Plugin 暗示为任意第三方动态插件。
- 把 Ramble 描述成录音 session，或把 Speech Engine 描述成 Backend Runtime 的跨客户端共享服务。
- 把 Agent 配置称为“ACP Client 配置”而混淆协议角色，或把 ACP Bridge 称为 RambleDesk 的反馈适配器。
- 把会话、任务、执行轮次、反馈请求、Tab 和 ACP 实例混为同一个对象。

代码与架构文档命名：

- `Adapter / 适配器` 只用于完整 host-facing 集成，例如 Generic MCP Adapter 与 Pi Native Adapter。
- Workbench Client 的应用访问 seam 称为 `Application Transport Interface`；Tauri IPC、HTTP + WebSocket 称为其 `Implementation`。
- OS / device seam 称为 `Native Capability` 或 `Browser Capability`；具体实现称为 `Capability Implementation`，不称为 Adapter。
- 组合语音、截图等单一平台设备流程的深 Module 称为 `Platform Plugin`；首期是第一方 typed composition，不承诺动态插件系统。
- `Web Access` 只表示可独立启停的浏览器访问 feature，不表示 Backend Runtime。
- `Agent Session Management` 表示会话管理能力；`ACP Client` 是协议实现；`ACP Instance` 是受控启动资源，不使用笼统的“ACP base”作为领域对象名。

## 合并标准

- “适配器”只有一个产品含义：完整 host-facing 集成流程。
- `core` 不包含 JSON、HTTP、MCP、Pi、Local Integration Server、Web Access 或 desktop command 逻辑。
- Backend Runtime 是唯一业务事实来源；Workbench Client 只保存 client-local workspace snapshot，不缓存 canonical Feedback Draft。
- Application Transport 与 Native / Browser Capability 保持独立，Transport Implementation 不执行设备能力。
- Speech Recognition Plugin 与 Capture Plugin 在当前客户端设备运行；Application Transport 不传输实时音频、识别 session 或设备权限。
- Ramble Core 的 canonical 输入面是 TipTap Feedback Draft；Platform Plugin 只能通过 SpeechEvent 或 Attachment Candidate 贡献内容。
- Local Integration Server 与 Web Access 必须复用同一安全 policy/primitives，同时分离 listener、credential、auth domain、启停生命周期和 route set。
- 本地 JSON API 位于 `rambledesk-local-server`。
- MCP 是薄适配层，不持有 listener、token path 或 JSON API。
- `rambledesk-hosts` 只持有 host profile 和 strategy 选择，不实现完整适配器。
- 外部反馈协议不要求源码 checkout 地址；托管会话单独验证执行目录。
- Pi 被描述为 Pi 原生适配器。
- 通用 MCP 适配器明确使用手动 continuation，不承诺自动恢复原宿主上下文。
