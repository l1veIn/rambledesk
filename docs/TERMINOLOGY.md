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
7. Workbench Client 的 workspace snapshot（view、顺序、active view、pane 尺寸）是 client-local 状态，不是 Backend Runtime 业务事实，也不得缓存 Feedback Draft 正文或结构化交互状态。
8. MCP 是通用 MCP 适配器的一种 transport，不是全局基础设施。
9. 提交后的 continuation 不是适配器。反馈路径可以选择“不需要 continuation”“手动 continuation”“原生 continuation”或“托管 continuation”。
10. 外部反馈请求不要求源码 checkout 路径，路径可以是可选 context hint；托管会话必须指定 Backend Runtime 所在机器上的工作目录 `cwd`，它不建立源码 checkout 管理模型。
11. 语音识别与屏幕采集发生在输入所在的客户端设备；Platform Plugin 只把结构化转录事件或附件候选交给共享输入流程，由该流程按固定目标写入草稿正文或业务字段，不通过 Application Transport 代理设备能力。

## 核心术语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| 人类 | 使用 RambleDesk 产生真实反馈的人。 | 拥有产品判断；不拥有协议状态。 |
| 智能体 | 发起反馈请求并读取反馈包继续工作的 LLM coding actor。 | 拥有任务推理；不拥有 RambleDesk 持久状态。 |
| 宿主 | 智能体运行所在的 runtime/container，例如 Pi、Claude Code、Codex、OpenCode。 | 拥有自己的 session、tool、plugin API；不定义 RambleDesk 存储合同。 |
| 工作台 | RambleDesk 的人类反馈工作界面。涉及某一种业务交互时使用“工作台类型”，涉及其界面实现时使用“工作台视图”。 | 不实现宿主协议，不限定为桌面窗口；同一合同由 Desktop / Web Workbench Client 呈现。 |
| Workbench Type（工作台类型） | 以 `workbench.type` 与 `version` 标识的反馈合同，规定不可变输入、允许的交互状态、完整性校验和提交结果。 | 当前为随应用发布的第一方静态类型；不是设备 Capability、宿主 Adapter 或可安装插件。未指定类型的旧请求继续采用 Ramble。 |
| Workbench View（工作台视图） | 某工作台类型在客户端的专用呈现与交互实现，位于导航和反馈列之间的独立工作区域。 | 只读请求输入并修改所属 Draft 的 Interaction State；不拥有独立保存、发布、设备资源或 Agent 续接链路。 |
| 工作台容器 | 承载当前请求的共享部分：布局、状态协调、工作台视图、反馈列、提交与附件预览。 | 不解释某个类型的题目、选项或批注；通过静态注册项组合对应视图与类型规则。 |
| Workbench Registry（工作台注册表） | 当前构建可用的第一方类型及其客户端实现的静态目录。 | 不持有业务事实，不执行动态安装；面向 Agent 的工作台发现目录是这些类型的用途与合同投影，不是渲染器协议，也不是用于选择 Agent 后端的 Agent Catalog。 |
| 反馈列 | 承载自由反馈的右列：输入工具（语音、截图、剪贴板、文件、Tidy）、TipTap 正文、保存状态、提交行与 Rambelle 状态条。 | 一个客户端同时只有一个可编辑反馈列。 |
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
| 请求材料 | Agent 随请求提供的附件与上下文材料（`request_attachments`）。 | 不是人类的反馈附件，也不是 Attachment Candidate；只读展示与预览。 |
| 反馈附件 | 人类在反馈中采集或导入并持久化后的附件（`workspace.attachments`）。 | 必须先成为 Attachment Candidate 且持久化成功；与请求材料分开计数与展示。 |
| Audio Source（音频源） | Speech Recognition Plugin 内负责取得有明确 sample rate 的本地单声道 PCM 的 Interface/Implementation。 | 不执行语音识别、不传输到 Backend Runtime、不拥有 Feedback Draft。 |
| Speech Engine（语音识别引擎） | Speech Recognition Plugin 内消费本地 PCM 并产生 SpeechEvent 的识别 Implementation。 | Desktop、Browser 及未来 Mobile 实现各自在本设备运行；统一点是事件合同，不是进程、模型或 transport。 |
| 反馈请求 | 由外部反馈适配器或托管反馈入口创建、由人类处理的持久单位，用 `request_id` 标识。 | RambleDesk 的核心输入事实。 |
| 反馈包 | 人类提交反馈或持久化取消结果时发布的不可变证据，包含 manifest、类型专属结果、Markdown 正文、附件与 hash。 | 宿主消费的输出事实；取消的类型结果为空。直接批准最终总结可以完成请求而不发布反馈包，不能从终态一概推断包存在。 |
| Feedback Adapter（反馈适配器，简称适配器） | 面向一类宿主的完整反馈接入流程：创建请求、读取反馈、处理 continuation。 | 可以由多个 package 或 transport 组成；不因此拥有 Agent 会话启动与进程管理职责。 |
| continuation | 请求进入终态后，让原宿主继续的行为。 | 只处理终态之后；不创建请求，不发布反馈包。 |
| Agent Backend（智能体后端，现有文档称宿主 / Host） | 提供智能体推理、工具和会话能力的外部软件，例如 Pi、dsh、Codex。 | 不等于 RambleDesk 的 Backend Runtime，也不等于某一个 OS 进程。 |
| Agent Session（智能体会话，现有文档称宿主会话） | Agent Backend 中持续关联的一段对话与执行上下文。 | 可处理多个任务、产生多次反馈请求；任务切换不自动创建新会话。 |
| context hint | 适配器可选提供的展示/定位信息，例如标题、路径、URL、文件引用。 | 不参与认证，不是必需身份字段，不保证可恢复。 |
| Ramble | 以自由正文和体验动作为中心的默认工作台类型；文字是基础输入，语音、截图等 Platform Plugin 可向同一正文贡献内容。 | 不等同于录音 session，不拥有平台设备能力，也不属于适配器协议。 |
| Uncooked Feedback | 人类通过 Ramble、文字、截图形成的原始反馈正文；允许保留口语、重复和自我修正。 | 是人类原始证据，Cooking 不得覆盖；提交后保存为反馈包中的 `uncooked.md`。 |
| Feedback Draft（反馈草稿） | 当前未提交请求的人类输入聚合，包括正文 Document 与 Interaction State，并关联反馈附件。 | Backend Runtime 持有已保存真源；正文与交互状态同 revision/CAS 原子保存和提交，客户端只持有编辑中的投影。 |
| Draft Document（草稿正文） | Feedback Draft 中的 TipTap 富文本文档 `doc`，负责自由反馈和补充说明。 | `body_markdown` 仅是 `doc` 的派生投影；不包含或反向决定结构化答案。正文撤销不撤销工作台选择。 |
| Interaction State（交互状态） | Feedback Draft 中类型专属、可编辑、可恢复的回答、选择或批注，保存为 `workbenchState`。 | 可以未完成；不是最终结果、临时组件状态、Editor selection 或客户端布局快照。 |
| Workbench Result（工作台结果） | 提交时根据不可变请求输入解析、校验 Interaction State 后生成的结构化人类回答。 | 保存于反馈包 `manifest.workbench.result`；发布后不可变。其展示文字和锚点由服务端校验，不能从自由正文猜测。 |
| Request Outcome（请求处理结果） | 请求被提交、取消或在允许时直接批准的业务含义，与请求状态和包记录共同表达。 | 是概念边界，不新增同名 wire 字段；取消不产生结构化回答，直接批准不保证有反馈包，不能从 `completed` 推断作答内容。 |
| Action（体验动作） | Ramble 请求中希望人类执行或观察的一个动作，以 `id` 和 `instruction` 表达。 | 不泛指问答题、选择项或文稿段落；旧数据路径中出现的投影属于兼容实现，不改变这些对象的业务含义。 |
| Input Target（输入目标） | 一次文字、语音或附件输入所归属的请求及具体正文、答案或批注字段，代码中为 `InputTarget`。 | 下一次输入的位置可以切换，已经开始的采集固定原目标。`SpeechTarget` 是语音侧别名；目标关联不代表作答完成，各类型无须通过 Action 表达全部对象。 |
| Review Source（审阅原稿） | `document_review` 请求中以 `source_version` 标注来源版本、由稳定段落 id 组成的不可变材料。 | 不属于人类 Draft，不可在当前请求中编辑；换稿建立新请求。 |
| Review Annotation（审阅批注） | 指向原稿整段或单段文字范围的意见或改写建议，具有稳定 id，不设解决状态。 | 属于 Interaction State；人类编辑和提交意见，Agent 在提交后处理原稿，批注本身不表示已改稿。 |
| Review Verdict（审阅判断） | 人类对整稿显式作出的 `ready` 或 `changes_requested` 判断。 | 是类型结果，不是请求状态或执行权限；仍须经统一提交发布。 |
| Action Group | 用标准 Blockquote 表达的 `@Action` 归属容器。 | 同一 Action 再次打开时创建新容器，不与旧区间合并。 |
| Tidy | 对尚未整理的语音文本进行表达整理，可由人类手动触发或按已启用的规则自动触发。 | 处理确认前的待写入语音，或当前请求正文、答案和批注中的 pending 语音段；不处理普通键入或粘贴，不自动提交，不是 Cooking。 |
| Cooking | 提交前可选的大模型编辑步骤，把 Uncooked Feedback 整理为正式 Markdown。 | 只做表达整理，不得编造事实、测试结果或删除负面判断；不开启时不调用模型服务。 |
| Cooked Feedback | Cooking 生成并经人类选择提交的正式反馈正文。 | 保存为反馈包中的 `feedback.md`；其来源必须可追溯到 `uncooked.md`。类型专属答案以 manifest 中的 Workbench Result 为准，不由 Cooking 或正文推断。 |

工作台与反馈列的关系约束：

- CURRENT：一个请求在一个客户端上只有一个可编辑反馈列和一份完整草稿。正文、回答与批注共用 TipTap 编辑基础，各可见字段有自己的选区与撤销历史；工作台不复制采集、保存和提交实现，不保留后台隐藏 Editor。
- CURRENT：请求材料与反馈附件是两份不同的集合，分别计数、分别展示，不互相代替。
- CURRENT：工作台类型通过共享请求、草稿和反馈包合同进入同一生命周期。目录中的 `purpose` / `returns` 描述用途，`interaction` 是分类元数据，不承诺可替换的通用渲染器协议。
- CURRENT：`document_json` 是版本化 Draft envelope，包含 TipTap `doc` 与可选 `workbenchState`；字段名不意味着整份草稿都是 TipTap 节点。`body_markdown` 仅由 `doc` 派生。
- CURRENT：类型或版本未知时只读保留请求材料、正文与交互数据；不覆盖保存、不提交、不通过直接批准绕过类型完整性。未知合同不等于请求终态；显式取消仍使用请求级生命周期。
- CURRENT：发现目录提供六种正式类型：`ramble`、`questions`、`document_review`、`web_review`、`terminal`、`sort`。`single_choice` 仅保留旧合同兼容，`rating_review` 仅在开发验收构建中注册，不计入正式数量。问答的 `cancelled` 和旧单选的 `status` 不重新定义 Request Outcome；细节见[反馈协议](PROTOCOL.md)。
- CURRENT：全屏页签是工作台类型主动开启的客户端布局能力，不是新请求或新草稿。普通模板默认关闭；开启后同一请求的普通与全屏视图复用草稿及运行状态，关闭页签不代表停止后端资源。

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
| Feedback Delivery（反馈投递） | 以 `request_id` 为身份的托管提交/批准续接记录，用于排队、去重与恢复；取消不产生新的托管投递。delivered 表示续接消息已发给 Agent 或用户确认已处理，不表示后续轮次成功或任务完成；uncertain 只由用户显式重试或确认。 |

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
- **请求级整理**：当前可编辑请求的正文、答案和批注共同统计 pending 语音段，由 Rambelle 状态区手动 Tidy，或达到数量阈值时 Auto Tidy；阈值默认 `0`，表示关闭。无配置、只读、编辑锁定或已有整理操作时不触发；不建立后台 Editor 或 idle timer。
- 待确认语音在编辑、整理、写入时有独立占用状态。编辑中的文本不能被并发确认或整理；写入失败保留可重试原文。整理结果必须仍对应原 owner/段落快照，丢弃或切换后不得复活旧片段。
- 语音段保留稳定身份和 `pending` / `cleaned` metadata；确认前已整理的内容写入正文或字段时传递 `cleaned`，避免再次整理。人工修改待确认转写后重新标为 pending；已写入字段中的语音被人工改动后退出待整理统计，避免覆盖人类编辑。
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

### 反馈输入

- **Request Input Session / 请求输入会话**：客户端为一个反馈请求协调录音、待确认转写、字幕、工具台及输入完成检查的生命周期；不表示新的 Agent 会话或后端请求。
- **Input Target / 输入目标**：具有请求身份的业务写入位置，例如正文的 Action 分组、指定来源版本的一条批注意见或建议措辞；不能用临时 DOM 焦点代替。
- **Selected Target / 所选目标**：下一段语音使用的位置。正在说的段固定使用开始时捕获的目标，迟到的识别结果不跟随此选择变化。
- **Input Preparation / 输入准备检查**：结束或排空已接收输入并检查未处理内容的提交前边界；隐藏组件、关闭字幕或结束截图窗口均不代表内容已经保存。

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
- 把请求材料说成反馈附件（或反之），或把人类的反馈附件描述成 Agent 提供的材料。

代码命名遵守同一边界：宿主反馈接入称 Adapter；应用访问称 Application Transport Interface / Implementation；设备访问称 Capability Implementation；设备流程组合称 Platform Plugin。Backend Runtime 是业务运行角色，Web Access 是可独立启停的访问能力，二者不互换。
