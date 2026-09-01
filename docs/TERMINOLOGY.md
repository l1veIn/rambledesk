# RambleDesk 术语表

> 状态：v5 当前基线。
> 目标：固定产品语言、协议字段和 package 边界。代码、文档、UI 文案、测试命名若与本文冲突，以本文为准。

本文是 RambleDesk 的唯一术语源。其他文档只引用本文，不重新定义产品对象。

## 架构公理

1. RambleDesk 是本地 human-feedback workbench，不是智能体运行时，不内置 shell multiplexer，也不持有源码 checkout 模型。
2. 反馈请求和反馈包构成核心闭环；请求、Feedback Draft、反馈包、配置以及未来的 Session Runtime / Timeline 等业务事实只由 Backend Runtime 持有。
3. 宿主通过适配器接入 RambleDesk；适配器是完整宿主流程，不是图标、label 或单个命令。
4. `core` 只持有 application contract，不持有 HTTP、JSON、MCP、Pi、Local Integration Server、Web Access、desktop command 或宿主安装逻辑。
5. Workbench Client 通过 Application Transport Interface 访问 Backend Runtime；Transport 与设备 Capability 是两个独立边界。
6. Local Integration Server 与 Web Access 必须复用一套安全 policy/primitives，但拥有显式分离的 listener、credential、auth domain、启停生命周期和 route set。
7. Workbench Client 的 workspace snapshot（view、顺序、active view、pane 尺寸）是 client-local 状态，不是 Backend Runtime 业务事实，也不得缓存 Feedback Draft 正文。
8. MCP 是通用 MCP 适配器的一种 transport，不是全局基础设施。
9. 提交后的 continuation 不是适配器。适配器可以选择“不需要 continuation”“手动 continuation”或“原生 continuation”。
10. RambleDesk 不要求源码 checkout 路径。路径最多是适配器提供的可选 context hint。
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
| Web Client（Web 客户端） | 在浏览器中运行的 Workbench Client，通过 Web Access 的 HTTP + WebSocket Application Transport Implementation 访问 Backend Runtime。 | 当前支持默认关闭、仅 loopback 的文字反馈工作流；不据此声称已有 LAN、浏览器录音、截图或桌面原生能力。 |
| Backend Runtime（后端运行时） | 长期持有 application use cases、storage、配置以及未来 Session Runtime / Timeline 的 Rust 运行角色。 | 是业务事实唯一来源；当前由 desktop composition root 组装，不等同于 HTTP listener，也不预设一个新 crate。 |
| Application Transport（应用传输） | Workbench Client 调用 application command/query、订阅变化、等待 ready 并读取 capability manifest 的 Interface。 | Tauri IPC 与 HTTP + WebSocket 是不同 Implementation，但调用同一 Backend Runtime application Module；`capabilities` 只报告可用性，不执行设备能力。 |
| Local Integration Server（本地集成服务） | 为 Generic MCP、Pi 等 Host Adapter 提供 authenticated loopback listener、JSON API、route mounting 和 guard 的 transport Module。 | 服务宿主集成，不拥有领域语义；其启停和 route set 独立于 Web Access。 |
| Web Access（Web 访问） | 可选、默认关闭的浏览器访问能力，通过独立 loopback listener 向 Web Client 提供静态资源、HTTP 与 WebSocket。 | 当前只监听 `127.0.0.1`；关闭它不停止 Backend Runtime 或 Local Integration Server，开放 LAN 仍需要单独的安全决策。 |
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
| 适配器 | 面向一类宿主的完整接入流程：创建请求、读取反馈、处理 continuation。 | 可以由多个 package 或 transport 组成。 |
| continuation | 请求进入终态后，让原宿主继续的行为。 | 只处理终态之后；不创建请求，不发布反馈包。 |
| 宿主会话 | 宿主中的原对话、任务或运行上下文。 | 同一宿主会话可以发起多次反馈请求；不是源码 checkout。 |
| context hint | 适配器可选提供的展示/定位信息，例如标题、路径、URL、文件引用。 | 不参与认证，不是必需身份字段，不保证可恢复。 |
| Ramble | 以 TipTap Feedback Draft 为中心的自由反馈编辑流程；文字是基础输入，语音、截图等 Platform Plugin 可向同一文档贡献结构化内容。 | 不等同于录音 session，不拥有平台设备能力，也不属于适配器协议。 |
| Uncooked Feedback | 人类通过 Ramble、文字、截图形成的原始反馈正文；允许保留口语、重复和自我修正。 | 是人类原始证据，Cooking 不得覆盖；提交后保存为反馈包中的 `uncooked.md`。 |
| Feedback Draft | 当前未提交请求的可编辑正文。canonical 真源是版本化 TipTap `document_json`，`body_markdown` 是同一份文档的派生投影。 | 不得把 Markdown 当作第二真源独立维护。 |
| Action Group | 用标准 Blockquote 表达的 `@Action` 归属容器。 | 同一 Action 再次打开时创建新容器，不与旧区间合并。 |
| Tidy | 人类在当前 Editor 中手动触发的 ASR 段落整理。 | 只处理 pending 语音节点；后台文档不整理；不是 Cooking。 |
| Cooking | 提交前可选的大模型编辑步骤，把 Uncooked Feedback 整理为正式 Markdown。 | 只做表达整理，不得编造事实、测试结果或删除负面判断；不开启时不调用模型服务。 |
| Cooked Feedback | Cooking 生成并经人类选择提交的正式反馈正文。 | 保存为反馈包中的 `feedback.md`，是宿主默认读取的反馈结果；其来源必须可追溯到 `uncooked.md`。 |

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
| `host_session_id` | 宿主提供或适配器生成的会话关联 id。 | 用于把同一宿主会话的多次 request 收敛；不是认证凭据，也不证明可自动继续。 |
| `context_refs` | 可选上下文引用列表。 | 承载文件、URL、diff、截图等可读线索。 |
| `source_hint` | 可选来源提示。 | 可包含路径或标题；不得成为创建请求的硬前提。 |

结论：

- `host_id` 是宿主身份字段。
- `host_session_id` 是宿主会话关联字段。
- 同一宿主会话的多次 request 通过 `(host_id, host_session_id)` 收敛。
- RambleDesk 不理解也不要求源码 checkout 地址。

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
| 无提交后 continuation | 适配器已在 active tool call 内等待，终态直接返回。 | Pi 原生适配器。 |
| 手动 continuation | 显示恢复提示，让人类回宿主调用 `get_feedback`。 | 通用 MCP 适配器。 |
| 原生 continuation | 由宿主官方能力安全恢复原上下文。 | 未来原生适配器。 |

## Package 边界

下表描述当前代码位置；架构角色不等于新 package 规划。特别是 Backend Runtime 是当前由 desktop composition root 组装的运行角色；Web Client 与 Web Access 复用现有 `apps/desktop` 与 server Module，本文不据此虚构新 crate。

| 架构角色 | 当前映射 | 目标边界 |
| --- | --- | --- |
| Backend Runtime | 由 `apps/desktop` composition root 组装 `core`、storage、配置和运行时 controller。 | 保持单一 application Module 和业务事实来源；是否重排 crate 留给后续实现决策。 |
| Workbench Client | `apps/desktop` 中的 Svelte 工作台 UI。 | Desktop Client 与 Web Client 复用同一 UI 和 Application Transport Interface。 |
| Desktop Client / Desktop Shell | `apps/desktop`。 | Shell 只保留 desktop composition 与 Native Capability；共享 UI 不依赖 Tauri 细节。 |
| Tauri Application Transport Implementation | `apps/desktop` 的 Tauri command/event wiring。 | 实现统一 Application Transport Interface，调用同一 Backend Runtime application Module。 |
| Local Integration Server | `crates/rambledesk-local-server`。 | 继续服务 Host Adapter；不因 Web Access 启停而停止；与 Web Access 共享同一安全 policy/primitives。 |
| Web Client / HTTP + WebSocket Application Transport Implementation / Web Access | `apps/desktop` 中的共享 Svelte UI、browser composition/auth gate/HTTP Transport，以及 `crates/rambledesk-local-server` 中的独立 Web Access server Module。 | 复用同一 Backend Runtime 与安全 policy/primitives，并与 Local Integration Server 分离 listener、credential、auth domain、生命周期和 route set。 |
| Native Capability Implementation | `apps/desktop` 及 desktop-only crates。 | 与 Application Transport 分离并通过 capability manifest 暴露可用性。 |
| Browser Capability Implementation | `apps/desktop` 当前使用浏览器 file picker 与 download，原生截图、原生录音、窗口和系统路径操作明确不可用。 | 后续媒体/剪贴板能力只承诺浏览器实际允许的范围，不模拟原生或服务器文件系统语义。 |
| Platform Plugin Implementation | `apps/desktop` 的 capability registry、Desktop Tauri Implementation 与 Browser Implementation；`rambledesk-speech` 是 Desktop Speech Recognition Plugin 的内部实现。 | 每个平台本地处理设备输入；只向共享 TipTap Ramble Core 输出 SpeechEvent 或 Attachment Candidate。 |

| Package / 区域 | 职责 | 不应包含 |
| --- | --- | --- |
| `crates/rambledesk-core` | 领域 DTO、application use cases、反馈请求/反馈包合同。 | HTTP、JSON、MCP、Pi、desktop commands、host install、Local Integration Server、Web Access。 |
| `crates/rambledesk-storage` | SQLite 持久化、请求/草稿/附件 metadata、宿主会话关联、反馈包发布。 | 宿主协议、适配器安装、源码 checkout runtime 语义。 |
| `crates/rambledesk-local-server` | 实现 Local Integration Server，并提供独立可组合的 Web Access server、session auth、静态资源与 application/event routes。 | 领域规则、MCP tool schema、Pi package 代码；两个 listener 的 credential、auth domain、route set 与生命周期不得合并。 |
| `crates/rambledesk-mcp` | Generic MCP Adapter 完整方案：MCP schema、tool handler、instructions、结果/错误映射、客户端检测/安装执行引擎。 | listener、token path、JSON API、host-specific continuation、per-host 知识。 |
| `crates/rambledesk-hosts` | 宿主知识注册表（executable/marker/配置路径/ConfigFormat）、Host profile、展示元数据、默认适配器选择、continuation strategy。 | MCP implementation、Pi package、适配器安装/写入执行逻辑。 |
| `packages/pi-rambledesk` | Pi 原生适配器 package。 | MCP client 行为、desktop UI 状态、storage 逻辑。 |
| `apps/desktop` | 当前实现 Workbench Client、Desktop Client、Desktop Shell、Tauri Application Transport Implementation、composition root 和适配器安装 UX。 | 领域存储语义、host package 内部实现；共享 Workbench Client 不应直接依赖 Tauri 或 Native Capability 细节。 |

目标 Cargo 依赖方向：

| Package | 允许依赖 |
| --- | --- |
| `rambledesk-core` | 无 workspace 领域依赖。 |
| `rambledesk-storage` | `rambledesk-core`。 |
| `rambledesk-mcp` | `rambledesk-core`、`rambledesk-hosts`。 |
| `rambledesk-local-server` | `rambledesk-core`、`rambledesk-mcp`。 |
| `rambledesk-hosts` | `rambledesk-core`；宿主知识注册表与续接策略共用其类型。 |
| `apps/desktop` | `rambledesk-core`、`rambledesk-storage`、`rambledesk-local-server`、`rambledesk-hosts`、`rambledesk-mcp`、desktop-only crates。 |
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

## 命名规则

UI 文案允许：

- “适配器”
- “通用 MCP 适配器”
- “Pi 原生适配器”
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

代码与架构文档命名：

- `Adapter / 适配器` 只用于完整 host-facing 集成，例如 Generic MCP Adapter 与 Pi Native Adapter。
- Workbench Client 的应用访问 seam 称为 `Application Transport Interface`；Tauri IPC、HTTP + WebSocket 称为其 `Implementation`。
- OS / device seam 称为 `Native Capability` 或 `Browser Capability`；具体实现称为 `Capability Implementation`，不称为 Adapter。
- 组合语音、截图等单一平台设备流程的深 Module 称为 `Platform Plugin`；首期是第一方 typed composition，不承诺动态插件系统。
- `Web Access` 只表示可独立启停的浏览器访问 feature，不表示 Backend Runtime。

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
- 协议不要求源码 checkout 地址。
- Pi 被描述为 Pi 原生适配器。
- 通用 MCP 适配器明确使用手动 continuation，不承诺自动恢复原宿主上下文。
