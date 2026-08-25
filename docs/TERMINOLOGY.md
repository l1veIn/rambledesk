# RambleDesk 术语表

> 状态：v2 当前基线。
> 目标：固定产品语言、协议字段和 package 边界。代码、文档、UI 文案、测试命名若与本文冲突，以本文为准。

本文是 RambleDesk 的唯一术语源。其他文档只引用本文，不重新定义产品对象。

## 架构公理

1. RambleDesk 是本地 human-feedback workbench，不是智能体运行时，不内置 shell multiplexer，也不持有源码 checkout 模型。
2. 核心事实只有两类：反馈请求和反馈包。
3. 宿主通过适配器接入 RambleDesk；适配器是完整宿主流程，不是图标、label 或单个命令。
4. `core` 只持有 application contract，不持有 HTTP、JSON、MCP、Pi、本地服务、desktop command 或宿主安装逻辑。
5. 本地服务是 transport 边界，独立于 `core` 和 `mcp`。
6. MCP 是通用 MCP 适配器的一种 transport，不是全局基础设施。
7. 提交后的 continuation 不是适配器。适配器可以选择“不需要 continuation”“手动 continuation”或“原生 continuation”。
8. RambleDesk 不要求源码 checkout 路径。路径最多是适配器提供的可选 context hint。

## 核心闭环

1. 宿主中的智能体通过适配器创建反馈请求。
2. RambleDesk 持久化请求，并在工作台展示。
3. 人类在工作台中检查上下文、截图、录音、书写反馈，然后提交或取消。
4. RambleDesk 发布不可变反馈包。
5. 适配器或 continuation 让原宿主读取反馈包并继续。

## 核心术语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| 人类 | 使用 RambleDesk 产生真实反馈的人。 | 拥有产品判断；不拥有协议状态。 |
| 智能体 | 发起反馈请求并读取反馈包继续工作的 LLM coding actor。 | 拥有任务推理；不拥有 RambleDesk 持久状态。 |
| 宿主 | 智能体运行所在的 runtime/container，例如 Pi、Claude Code、Codex、OpenCode。 | 拥有自己的 session、tool、plugin API；不定义 RambleDesk 存储合同。 |
| 工作台 | RambleDesk 桌面 UI。 | 拥有人类反馈工作流；不实现宿主协议。 |
| 本地服务 | 桌面进程内的 authenticated loopback server。 | 拥有 auth、listener、JSON API、route mounting 和 guard；不拥有领域语义。 |
| 反馈请求 | 由适配器创建、由人类处理的持久单位，用 `request_id` 标识。 | RambleDesk 的核心输入事实。 |
| 反馈包 | 请求进入终态后发布的不可变证据，包含 manifest、markdown、附件路径和 hash。 | RambleDesk 的核心输出事实；宿主继续前必须读取。 |
| 适配器 | 面向一类宿主的完整接入流程：创建请求、读取反馈、处理 continuation。 | 可以由多个 package 或 transport 组成。 |
| continuation | 请求进入终态后，让原宿主继续的行为。 | 只处理终态之后；不创建请求，不发布反馈包。 |
| 宿主会话 | 宿主中的原对话、任务或运行上下文。 | 同一宿主会话可以发起多次反馈请求；不是源码 checkout。 |
| context hint | 适配器可选提供的展示/定位信息，例如标题、路径、URL、文件引用。 | 不参与认证，不是必需身份字段，不保证可恢复。 |
| Ramble | 工作台内的自由反馈采集模式，尤其是语音、文字、截图驱动的反馈。 | 属于人类工作流，不属于适配器协议。 |
| Uncooked Feedback | 人类通过 Ramble、文字、截图形成的原始反馈正文；允许保留口语、重复和自我修正。 | 是人类原始证据，Cooking 不得覆盖；提交后保存为反馈包中的 `uncooked.md`。 |
| Cooking | 提交前可选的大模型编辑步骤，把 Uncooked Feedback 整理为正式 Markdown。 | 只做表达整理，不得编造事实、测试结果或删除负面判断；不开启时不调用模型服务。 |
| Light cleanup | 可选的轻度转写整理：去掉语气词、修正断句，不改变意思。可独立于 Cooking 开启。 | 发生在语音写入正文时；不是 Cooking，不生成正式反馈结构。失败时保留原始转写。 |
| Cooked Feedback | Cooking 生成并经人类选择提交的正式反馈正文。 | 保存为反馈包中的 `feedback.md`，是宿主默认读取的反馈结果；其来源必须可追溯到 `uncooked.md`。 |

## Cooking 规则

- Cooking 默认关闭，由人类在通用设置中显式启用并配置模型服务、模型和 API Key。
- API Key 是本机凭证，不属于反馈请求、反馈包、日志或宿主协议。
- 启用 Cooking 时，`uncooked.md` 和 `feedback.md` 必须同时进入不可变反馈包；关闭时两者内容可以相同。
- `feedback.md` 是宿主默认消费的正式结果，`uncooked.md` 是审计与恢复所需的原始人类证据。
- Cooking 失败不得丢失或锁死 Uncooked Feedback，也不得提交半成品反馈包。
- “Cooking”专指反馈编辑步骤，不指语音转录、反馈包发布或宿主智能体继续。
- Light cleanup 默认关闭，可独立于 Cooking 开启，并共用同一套模型服务、模型和 API Key。

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

| Package / 区域 | 职责 | 不应包含 |
| --- | --- | --- |
| `crates/rambledesk-core` | 领域 DTO、application use cases、反馈请求/反馈包合同。 | HTTP、JSON、MCP、Pi、desktop commands、host install、local server。 |
| `crates/rambledesk-storage` | SQLite 持久化、请求/草稿/附件 metadata、宿主会话关联、反馈包发布。 | 宿主协议、适配器安装、源码 checkout runtime 语义。 |
| `crates/rambledesk-local-server` | loopback listener、auth token、Host/Origin guard、本地 JSON API、route mounting。 | 领域规则、MCP tool schema、Pi package 代码。 |
| `crates/rambledesk-mcp` | Generic MCP Adapter 完整方案：MCP schema、tool handler、instructions、结果/错误映射、客户端检测/安装执行引擎。 | listener、token path、JSON API、host-specific continuation、per-host 知识。 |
| `crates/rambledesk-hosts` | 宿主知识注册表（executable/marker/配置路径/ConfigFormat）、Host profile、展示元数据、默认适配器选择、continuation strategy。 | MCP implementation、Pi package、适配器安装/写入执行逻辑。 |
| `packages/pi-rambledesk` | Pi 原生适配器 package。 | MCP client 行为、desktop UI 状态、storage 逻辑。 |
| `apps/desktop` | 工作台 UI、Tauri composition root、本地 command wiring、适配器安装 UX。 | 领域存储语义、host package 内部实现、唯一事实状态。 |

目标 Cargo 依赖方向：

| Package | 允许依赖 |
| --- | --- |
| `rambledesk-core` | 无 workspace 领域依赖。 |
| `rambledesk-storage` | `rambledesk-core`。 |
| `rambledesk-mcp` | `rambledesk-core`、`rambledesk-hosts`。 |
| `rambledesk-local-server` | `rambledesk-core`、`rambledesk-mcp`。 |
| `rambledesk-hosts` | `rambledesk-core`；宿主知识注册表与续接策略共用其类型。 |
| `apps/desktop` | `rambledesk-core`、`rambledesk-storage`、`rambledesk-local-server`、`rambledesk-hosts`、`rambledesk-mcp`、desktop-only crates。 |
| `packages/pi-rambledesk` | 不参与 Cargo workspace；运行时调用本地服务 `/api`。 |

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

UI 文案避免：

- 将 transport 可用性提升为产品全局状态。
- 在 titlebar 或 sidebar 放全局 transport 指示器。
- 用 “Adapter” 指代宿主图标或宿主 label。

## 合并标准

- “适配器”只有一个产品含义：完整 host-facing 集成流程。
- `core` 不包含 JSON、HTTP、MCP、Pi、本地服务或 desktop command 逻辑。
- 本地 JSON API 位于 `rambledesk-local-server`。
- MCP 是薄适配层，不持有 listener、token path 或 JSON API。
- `rambledesk-hosts` 只持有 host profile 和 strategy 选择，不实现完整适配器。
- 协议不要求源码 checkout 地址。
- Pi 被描述为 Pi 原生适配器。
- 通用 MCP 适配器明确使用手动 continuation，不承诺自动恢复原宿主上下文。
