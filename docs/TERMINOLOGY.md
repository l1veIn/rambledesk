# RambleDesk 术语表

本文是产品对象、身份和边界的唯一词汇源。代码、文档、UI 与测试命名若与本文冲突，以本文为准；其他文档引用定义，不建立第二份词汇表。

当前实现结构见[架构](ARCHITECTURE.md)，操作和支持范围见 [ACP 指南](ACP_MANAGED_SESSIONS.md)、[Web Access 矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)及[质量清单](quality/README.md)。CURRENT 表示已有实现，TARGET 表示已接受但未实现的边界；均不替代具体平台验收。

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

## 核心术语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| 人类 | 使用 RambleDesk 产生真实反馈的人。 | 拥有产品判断；不拥有协议状态。 |
| 智能体 | 发起反馈请求并读取反馈包继续工作的 LLM coding actor。 | 拥有任务推理；不拥有 RambleDesk 持久状态。 |
| 宿主 | 智能体运行所在的 runtime/container，例如 Pi、Claude Code、Codex、OpenCode。 | 拥有自己的 session、tool、plugin API；不定义 RambleDesk 存储合同。 |
| 工作台 | RambleDesk 的人类反馈工作界面；同一套工作台可以由不同 Workbench Client 呈现。 | 拥有人类反馈工作流；不实现宿主协议，不限定为桌面窗口。 |
| Workbench Client（工作台客户端） | 承载共享工作台 UI 的客户端角色；当前由 `apps/desktop` 中的 Svelte UI 实现，并由 Desktop Client 与 loopback Web Client 复用。 | 只持有 UI 投影和 client-local workspace snapshot；不拥有 Request、Feedback Draft 或 Package 的 canonical 事实。 |
| Desktop Client（桌面客户端） | 在 Desktop Shell 内运行的 Workbench Client，通过 Tauri IPC 的 Application Transport Implementation 访问 Backend Runtime。 | 是当前已实现的客户端；不把 Tauri API 暴露为共享 UI 的业务合同。 |
| Web Client（Web 客户端） | 在浏览器中运行的 Workbench Client，通过 Web Access 的 HTTP + WebSocket Application Transport Implementation 访问 Backend Runtime。 | 浏览器能力受当前设备、权限和手势约束；支持面与待验项由 Web Access 矩阵维护，不能外推为原生或 LAN 能力。 |
| Backend Runtime（后端运行时） | 长期持有 application use cases、storage、配置以及 Session Runtime / Activity 的 Rust 运行角色。 | 是业务事实唯一来源；当前由 desktop composition root 组装，不等同于 HTTP listener，也不预设一个新 crate。 |
| Application Transport（应用传输） | Workbench Client 调用 application command/query、订阅变化、等待 ready 并读取 capability manifest 的 Interface。 | Tauri IPC 与 HTTP + WebSocket 是不同 Implementation，但调用同一 Backend Runtime application Module；`capabilities` 只报告可用性，不执行设备能力。 |
| Local Integration Server（本地集成服务） | 为 Generic MCP、Pi 等 Host Adapter 提供 authenticated loopback listener、JSON API、route mounting 和 guard 的 transport Module。 | 服务宿主集成，不拥有领域语义；其启停和 route set 独立于 Web Access。 |
| Web Access（Web 访问） | 可选、默认关闭的浏览器访问能力，通过独立 loopback listener 向 Web Client 提供静态资源、HTTP 与 WebSocket。 | 默认监听 `127.0.0.1:37643`，端口可在浏览器服务设置中修改；关闭它不停止 Backend Runtime 或 Local Integration Server。LAN、TLS 与 headless 不在当前支持面。 |
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
| 反馈包 | 人类提交反馈或持久化取消结果时发布的不可变证据，包含 manifest、Markdown、附件与 hash。 | 宿主消费的输出事实；直接批准最终总结可以完成请求而不发布反馈包，不能从终态一概推断包存在。 |
| Feedback Adapter（反馈适配器，简称适配器） | 面向一类宿主的完整反馈接入流程：创建请求、读取反馈、处理 continuation。 | 可以由多个 package 或 transport 组成；不因此拥有 Agent 会话启动与进程管理职责。 |
| continuation | 请求进入终态后，让原宿主继续的行为。 | 只处理终态之后；不创建请求，不发布反馈包。 |
| Agent Backend（智能体后端，现有文档称宿主 / Host） | 提供智能体推理、工具和会话能力的外部软件，例如 Pi、dsh、Codex。 | 不等于 RambleDesk 的 Backend Runtime，也不等于某一个 OS 进程。 |
| Agent Session（智能体会话，现有文档称宿主会话） | Agent Backend 中持续关联的一段对话与执行上下文。 | 可处理多个任务、产生多次反馈请求；任务切换不自动创建新会话。 |
| context hint | 适配器可选提供的展示/定位信息，例如标题、路径、URL、文件引用。 | 不参与认证，不是必需身份字段，不保证可恢复。 |
| Ramble | 以 TipTap Feedback Draft 为中心的自由反馈编辑流程；文字是基础输入，语音、截图等 Platform Plugin 可向同一文档贡献结构化内容。 | 不等同于录音 session，不拥有平台设备能力，也不属于适配器协议。 |
| Uncooked Feedback | 人类通过 Ramble、文字、截图形成的原始反馈正文；允许保留口语、重复和自我修正。 | 是人类原始证据，Cooking 不得覆盖；提交后保存为反馈包中的 `uncooked.md`。 |
| Feedback Draft | 当前未提交请求的可编辑正文。canonical 真源是版本化 TipTap `document_json`，`body_markdown` 是同一份文档的派生投影。 | 不得把 Markdown 当作第二真源独立维护。 |
| Action Group | 用标准 Blockquote 表达的 `@Action` 归属容器。 | 同一 Action 再次打开时创建新容器，不与旧区间合并。 |
| Tidy | 对尚未整理的语音文本进行表达整理，可由人类手动触发或按已启用的规则自动触发。 | 处理确认前的待写入语音，或当前 Editor 的 pending 语音段；不自动提交，不是 Cooking，不扫描后台文档。 |
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
| Feedback Delivery（反馈投递） | 以 `request_id` 为身份的托管提交/批准续接记录，用于排队、去重与恢复；取消不产生新的托管投递。delivered 表示续接轮次成功结束或用户确认已处理，不表示任务完成；uncertain 只由用户显式重试或确认。 |

关系约束：

- CURRENT 托管路径：一个 RambleDesk Session 对应一个 Agent Session；一个 Agent Session 可执行多个 Task、
  多个 Turn，并创建多个 Feedback Request。任务拆分不能成为另建 RambleDesk Session 的理由。
- CURRENT：Generic MCP 的关联 id 由调用者提供，历史数据不能据此证明真实 Agent Session 的一一关系；
  保留已有分组，不在术语更新中合并或改写历史。托管路径由 controller 固定反馈归属，不能由模型自由选择。
- 首期托管策略为一个会话独占一个 ACP Instance；ACP 协议是否支持连接内多会话，与本产品首期是否共享实例是两件事。
  后续共享属于实例分配策略，不改变会话的一一关系。
- View / Tab 是客户端视图；关闭正式会话视图不停止实例或删除会话。未发送的 prepared 草稿关闭时清理准备资源。
  内部托管会话的 Ramble 与 Agent 页面是同一业务会话的平级视图。删除托管会话是一条直接操作，内部负责停止与清理，
  不要求人类先结束或归档。停止运行并保留历史可以是另一条操作。
- 不用“Ramble session”同时指代会话、反馈请求与 Ramble 编辑流程；分别使用完整术语。

## Tidy 与 Cooking

- 两者各自持有 provider、API Key、base URL、model、reasoning effort 和 system prompt；不得回退使用另一套配置。API Key 是本机凭证，不属于请求、反馈包、默认日志或宿主协议。
- **确认前整理**：开启“语音写入前确认”后，人类可以再开启“自动整理语音”（默认关闭）。每段转录先整理，再由人类选择何时写入；关闭确认时直接写入，不执行这个确认前 Auto Tidy。
- **编辑器整理**：当前可编辑 Editor 支持手动 Tidy，也支持未整理语音段数量达到阈值时 Auto Tidy；阈值默认 `0`，表示关闭。无配置、只读、编辑锁定或已有整理操作时不触发；不建立后台 Editor 或 idle timer。
- 待确认语音在编辑、整理、写入时有独立占用状态。编辑中的文本不能被并发确认或整理；写入失败保留可重试原文。整理结果必须仍对应原 owner/段落快照，丢弃或切换后不得复活旧片段。
- 语音段保留稳定身份和 `pending` / `cleaned` metadata；确认前已整理的内容写入 Editor 时传递 `cleaned`，避免再按 pending 段处理。人工修改待确认正文后重新标为 pending。
- Cooking 默认关闭，由人类显式配置并启用。它整理提交前的 Uncooked Feedback，不是转录、Tidy、反馈包发布或 Agent 续接。
- Cooking 不得编造事实、测试结果或删除负面判断；原稿始终保留。提交的 `uncooked.md` 与 `feedback.md` 同时进入不可变包，关闭 Cooking 时两者可相同。失败不得丢失或锁死原稿，也不得发布半成品。

## 身份字段

| 字段 | 目标语义 | 规则 |
| --- | --- | --- |
| `request_id` | 唯一持久反馈请求 id。 | 创建幂等 key，也是读取反馈包的 lookup key。 |
| `host_id` | 稳定宿主家族 id，例如 `pi`、`claude`、`codex`、`opencode`、`grok`、`generic`。 | 用于展示、host profile 匹配和 continuation strategy 选择。 |
| `host_session_id` | 宿主提供或适配器生成的会话关联 id。 | CURRENT 外部反馈合同；不保证等于 Agent 的真实 session id，不是 MCP transport session id、认证凭据或自动恢复证明。 |
| `context_refs` | 可选上下文引用列表。 | 承载文件、URL、diff、截图等可读线索。 |
| `source_hint` | 可选来源提示。 | 可包含路径或标题；不得成为创建请求的硬前提。 |

CURRENT 会话字段类别：

| 类别 | 字段方向 | 规则 |
| --- | --- | --- |
| 本地身份 | `session_id` | 暴露现有 `host_sessions.id` 的稳定身份；不会随连接重建而改变。 |
| 准备/正式生命周期 | `lifecycle: prepared / active` | prepared 是未发送的内部准备记录，不进入正式导航。首条真实用户消息接受与持久化时转为 active；缺省旧记录按 active 兼容。 |
| 管理方式 | typed `management`：`external` 或 `managed` | `managed` 分支包含 `protocol: acp`、`agent_config_id`、`cwd` 和可空的 `remote_session_id`；不增加含义不清的 `acp_manage` 布尔值，也不同时存重复的布尔值与枚举。 |
| Agent 会话绑定 | `remote_session_id` | ACP 返回的 Agent Session id，可持久化用于受能力约束的恢复；不作为 RambleDesk 主键或全局唯一 id。创建完成前允许为空。 |
| 启动实例绑定 | `runtime.instance_id` | 当前实例的运行时身份；重启可变化，不表示可恢复的进程句柄。 |
| 可用性与运行投影 | `runtime`，分别表达连接状态与执行状态 | 不与 Request 的 waiting/completed 等业务状态混用；UI 状态来自 Backend Runtime。 |
| 上下文占用 | `runtime.context_usage: { used, size }` | 仅来自 Agent usage 更新的当前上下文 token 数与容量；不代表累计消耗或费用。不持久化为历史，未上报/换实例后未知。 |
| 反馈投递 | 独立 `FeedbackDelivery` 记录 | 以 `request_id` 关联托管会话与提交/批准结果；状态为 pending / sending / delivered / uncertain / discarded，不把投递状态塞进 `management`。 |
| 删除意图 | `deleting` 投影与持久 deletion intent | 在清理前落盘，失败或重启后仍可重试；成功删除时随所属会话清理。 |
| 运行检查点 | 独立 `SessionRecovery` 记录 | 以 run/turn id 限定写入归属；记录历史事实，不能代替实时连接状态。 |

`agent_config_id` 指向可复用启动配置；`cwd` 固定在具体会话。配置编辑仅影响后续启动，不能静默改变正在运行的实例。
`AgentConfig.catalog_id` 是可选的目录身份；多连接分别保留自己的配置 ID。历史迁移只保守识别一次，未知项保留自定义身份，运行时不再通过命令或路径猜测。
配置参数与环境变量值以结构化数据保存在本地 SQLite；界面隐藏和日志脱敏不表示加密凭据库。已绑定的
`remote_session_id` 只能通过 resume/load 恢复，失败不能静默创建空白替代会话。

## 适配器与 continuation

Feedback Adapter 服务宿主反馈流程；Host Profile 提供宿主家族的标签、安装线索、默认适配器和 continuation strategy。它们不代替 AgentConfig、会话管理方式或实际安装状态。一个 `host_id` 可以同时存在外部和托管会话。

| 路径 | continuation 语义 |
| --- | --- |
| Generic MCP | 手动继续：持久请求返回后结束当前 turn，人类完成后回宿主调用 `get_feedback`。没有 blocking wait tool，不保证自动恢复原上下文。 |
| Pi / dsh 原生适配器 | 同一工具调用等待终态，结果直接返回；不需要提交后的 continuation。 |
| 托管 ACP | 使用应用内置 `feedback request/get/recover` 与会话专用入口；提交/批准由 Backend Runtime 在原会话可接收输入后续接，取消保持本地。 |
| 未来原生 continuation | 只有宿主官方能力经过原上下文恢复验收后才能声明；CLI 探针、最佳努力进程 poke 或另建 transcript 不构成保证。 |

托管反馈归属由运行时凭据固定；缺少凭据不能回退成外部会话。Agent 自带 MCP、Skills 或插件不决定 RambleDesk 会话身份。协议细节见[反馈协议](PROTOCOL.md)，托管命令见 [ACP 指南](ACP_MANAGED_SESSIONS.md)。

## 命名规则

### Agent 目录与对话

- **Agent Catalog / 智能体目录**：可选择的 Agent 定义，包含名称、分发入口、推荐版本和能力说明；不是已安装清单。
- **Agent Installation / 智能体安装**：当前机器上的程序、版本和安装位置；ACP Bridge 与厂商 Agent 可分别需要安装。
- **AgentConfig / Agent 配置**：用户保存的启动选择、后端配置与环境；可以由目录和安装结果生成，高级用户也可手动填写。
- **Conversation Content / 对话内容**：有序的用户/Agent 消息及文本、思考、工具等内容块；流式更新修改所属内容，不创建新的 RambleDesk 会话。

这些名称不改变会话、任务、轮次和反馈请求之间的关系。目录能力声明、程序安装、ACP 握手和真实模型交互分别验证。

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

代码命名遵守同一边界：宿主反馈接入称 Adapter；应用访问称 Application Transport Interface / Implementation；设备访问称 Capability Implementation；设备流程组合称 Platform Plugin。Backend Runtime 是业务运行角色，Web Access 是可独立启停的访问能力，二者不互换。
