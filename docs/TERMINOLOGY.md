# RambleDesk 术语表

> 状态：待验收草案。
> 目标：固定产品语言和 package 边界，防止后续实现把 MCP、宿主、项目、继续机制混成一团。

本文是 RambleDesk 的术语合同。验收后，代码、文档、UI 文案和测试命名若与本文冲突，以本文为准。

## 1. 架构公理

1. RambleDesk 是本地 human-feedback workbench，不是 agent runtime，不内置 tmux，不持有宿主项目模型。
2. RambleDesk 的核心事实只有两类：反馈请求和反馈包。
3. 宿主通过适配器接入 RambleDesk；适配器可以是 MCP、Pi package、未来宿主 extension 等完整流程。
4. `core` 只持有 application contract，不持有 HTTP、JSON、MCP、Pi 或宿主安装逻辑。
5. 本地服务是 transport 边界，应独立于 `core` 和 `mcp`。
6. MCP 是通用适配器的一种 transport，不再是全局基础设施。
7. 提交后的继续机制不是适配器。适配器可以选择“不需要继续机制”“手动继续”或“原生继续”。
8. RambleDesk 不要求项目路径。路径最多是适配器提供的可选上下文 hint。

## 2. 核心闭环

1. Agent 所在宿主通过适配器创建反馈请求。
2. RambleDesk 持久化请求，并在工作台展示。
3. 人类在工作台中使用、检查、截图、录音、书写反馈，然后提交或取消。
4. RambleDesk 发布不可变反馈包。
5. 适配器或继续机制让原宿主读取反馈包并继续。

## 3. 核心术语

| 术语 | 定义 | 边界 |
| --- | --- | --- |
| 人类 | 使用 RambleDesk 产生真实反馈的人。 | 拥有产品判断；不拥有协议状态。 |
| Agent | 发起反馈请求并读取反馈包继续工作的 LLM coding actor。 | 拥有任务推理；不拥有 RambleDesk 持久状态。 |
| 宿主 | Agent 运行所在的 runtime/container，例如 Pi、Claude Code、Codex、OpenCode。 | 拥有自己的 session/tool/plugin API；不定义 RambleDesk 存储合同。 |
| 工作台 | RambleDesk 桌面 UI。 | 拥有人类反馈工作流；不实现宿主协议。 |
| 本地服务 | 桌面进程内的 authenticated loopback server。 | 拥有 auth、listener、JSON API、route mounting；不拥有领域语义。 |
| 反馈请求 | 由适配器创建、由人类处理的持久单位，用 `request_id` 标识。 | RambleDesk 的核心输入事实。 |
| 反馈包 | 请求进入终态后发布的不可变证据，包含 manifest、markdown、附件路径和 hash。 | RambleDesk 的核心输出事实；Agent 继续前必须读取。 |
| 适配器 | 面向一类宿主的完整接入流程：创建请求、读取反馈、处理继续。 | 可以由多个 package/transport 组成；不是图标、label 或单个 resume 命令。 |
| 继续机制 | 请求进入终态后，让原宿主继续的行为。 | 只处理终态之后；不创建请求，不发布反馈包。 |
| 宿主会话 | 宿主中的原对话、任务或运行上下文。 | 同一宿主会话可以发起多次反馈请求；不等于项目。 |
| 上下文 hint | 适配器可选提供的展示/定位信息，例如标题、路径、URL、文件引用。 | 不参与认证，不是必需身份字段，不保证可恢复。 |
| Ramble | 工作台内的自由反馈采集模式，尤其是语音、文字、截图驱动的反馈。 | 属于人类工作流，不属于适配器协议。 |

## 4. 身份字段

| 字段 | 目标语义 | 规则 |
| --- | --- | --- |
| `request_id` | 唯一持久反馈请求 id。 | 创建幂等 key，也是读取反馈包的 lookup key。 |
| `host_id` | 稳定宿主家族 id，例如 `pi`、`claude`、`codex`、`opencode`、`generic`。 | 用于展示、host profile 匹配和继续策略选择。 |
| `host_session_id` | 宿主提供或适配器生成的会话关联 id。 | 用于把同一宿主会话的多次 request 收敛；不是认证凭据，也不证明可自动继续。 |
| `context_refs` | 可选上下文引用列表。 | 承载文件、URL、diff、截图等可读线索。 |
| `source_hint` | 可选来源提示。 | 可包含路径或标题；不得成为创建请求的硬前提。 |

当前遗留字段收敛规则：

| 现有字段/类型 | 目标 |
| --- | --- |
| `agent` | 重命名/收敛为 `host_id`。 |
| `session_id` | 重命名/收敛为 `host_session_id`。 |
| `ProjectInput` | 移除，或拆成 `host_session_id` + `context_refs/source_hint`。 |
| `project_id` | 产品层废弃；不要引入新的“项目”身份。 |
| `project_root_path` | 产品层废弃；如仍需要展示，降级为 `source_hint` 或 `context_refs`。 |

结论：

- `host_id` 和 `agent` 重复，保留 `host_id`。
- `session_id` 不是 `project_id`；它是宿主会话关联。
- 同一会话的多次 request 通过 `(host_id, host_session_id)` 收敛。
- RambleDesk 不理解也不要求本地项目地址。

## 5. 适配器分类

### 5.1 通用 MCP 适配器

默认兼容路径，面向能调用 MCP tools、但不能被外部可靠恢复原上下文的宿主。

包含：

- MCP tools：`request_feedback`、`get_feedback`、`cancel_feedback`。
- 宿主 MCP client 配置写入。
- 终态后的手动继续提示。

不包含：

- MCP tool surface 上的 `wait_for_feedback`。
- 自动继续原宿主 session 的产品保证。
- 把 CLI resume 探针伪装成正式能力。

流程：

1. 宿主调用 `request_feedback`。
2. Agent 结束当前 turn。
3. 人类提交或取消。
4. 人类按恢复提示回到宿主。
5. Agent 调用 `get_feedback(request_id)` 并继续。

### 5.2 Pi 原生适配器

Pi 原生适配器是 `packages/pi-rambledesk`，不走 MCP。

包含：

- Pi tools：`request_ramble_feedback`、`get_ramble_feedback`。
- 调用本地 JSON API：`/api/feedback/request|get|wait|cancel`。
- 在 Pi tool call 内等待终态。

Pi 原生适配器不需要提交后的继续机制，因为 Pi 已经在工具调用中等待，终态反馈会直接返回原 Pi 流程。

### 5.3 未来原生适配器

只有当宿主提供可靠、已验收的原上下文保留/恢复方式时，才允许新增原生适配器。

合格形式：

- 宿主 package/plugin/extension 能在 active tool call 内等待。
- 宿主提供 continuation registration API。
- 宿主 resume API 被证明会继续目标上下文，而不是创建相邻 transcript。

不合格形式：

- 只能向某个 CLI session 发文本，但原可见宿主不继续。
- 最佳努力进程 poke。
- 无安装模型、无失败模型的一次性 e2e 探针。

## 6. 继续机制

| 类型 | 含义 | 使用场景 |
| --- | --- | --- |
| 无提交后继续 | 适配器已在 active tool call 内等待，终态直接返回。 | Pi 原生适配器。 |
| 手动继续 | 显示恢复提示，让人类回宿主调用 `get_feedback`。 | 通用 MCP 适配器。 |
| 原生继续 | 由宿主官方能力安全恢复原上下文。 | 未来原生适配器。 |

“wakeup” 在产品语言中废弃。历史文档可保留；新增代码和文档使用“继续机制 / continuation”。

## 7. Package 边界

| Package / 区域 | 目标职责 | 不应包含 |
| --- | --- | --- |
| `crates/rambledesk-core` | 领域 DTO、application use cases、反馈请求/反馈包合同。 | HTTP、JSON、MCP、Pi、desktop commands、host install、local server。 |
| `crates/rambledesk-storage` | SQLite 持久化、跨请求会话关联、反馈包发布。 | 宿主协议、适配器安装、本地项目 runtime 语义。 |
| `crates/rambledesk-local-server` | loopback listener、auth token、Host/Origin guard、本地 JSON API、route mounting。 | 领域规则、MCP tool schema、Pi package 代码。 |
| `crates/rambledesk-mcp` | 通用 MCP 适配器薄层：MCP schema、tool handler、structured result。 | listener、token path、JSON API、host-specific resume。 |
| `crates/rambledesk-hosts` | 宿主目录、host profile、展示元数据、默认适配器选择、继续模式声明。 | MCP implementation、Pi package、完整适配器实现。 |
| `packages/pi-rambledesk` | Pi 原生适配器 package。 | MCP client 行为、desktop UI 状态。 |
| `apps/desktop` | 工作台 UI、Tauri composition root、本地 command wiring、适配器安装 UX。 | 领域存储语义、host package 内部实现。 |

目标 Cargo 依赖方向：

| Package | 允许依赖 |
| --- | --- |
| `rambledesk-core` | 无 workspace 领域依赖。 |
| `rambledesk-storage` | `rambledesk-core`。 |
| `rambledesk-mcp` | `rambledesk-core`。 |
| `rambledesk-local-server` | `rambledesk-core`、`rambledesk-mcp`。 |
| `rambledesk-hosts` | `rambledesk-core` 仅在继续策略需要读取通用状态类型时允许；否则应尽量无领域依赖。 |
| `apps/desktop` | `rambledesk-core`、`rambledesk-storage`、`rambledesk-local-server`、`rambledesk-hosts`、desktop-only crates。 |
| `packages/pi-rambledesk` | 不参与 Cargo workspace；运行时调用 `rambledesk-local-server` 的 `/api`。 |

说明：

- 本地 JSON API 属于 `rambledesk-local-server`，不属于 `core`，也不属于 `mcp`。
- MCP 是挂在本地服务上的适配器 transport。它应复用同一套 application call path，但不需要通过 HTTP 调自己。
- `rambledesk-hosts` 是能力表/策略表，不是适配器实现仓库。

## 8. Host Profile

`rambledesk-hosts` 的基本单位是 Host Profile。

Host Profile 描述：

- `host_id`
- label / icon
- 默认适配器：通用 MCP 适配器、Pi 原生适配器、未来原生适配器
- 继续模式：无提交后继续、手动继续、原生继续
- 安装入口：MCP config、Pi package install、未来 host extension install

当前 profile 结论：

| Host | 默认适配器 | 继续模式 |
| --- | --- | --- |
| `generic` | 通用 MCP 适配器 | 手动继续 |
| `claude` | 通用 MCP 适配器 | 手动继续 |
| `codex` | 通用 MCP 适配器 | 手动继续 |
| `opencode` | 通用 MCP 适配器 | 手动继续 |
| `cursor` | 通用 MCP 适配器 | 手动继续 |
| `gemini` | 通用 MCP 适配器 | 手动继续 |
| `inspector` | 通用 MCP 适配器 | 手动继续 |
| `pi` | Pi 原生适配器 | 无提交后继续 |

## 9. 命名规则

| 当前 / 含糊命名 | 推荐命名 |
| --- | --- |
| `rambledesk-adapters` crate | `rambledesk-hosts` |
| `AdapterPresentation` | `HostProfile` 或 `HostPresentation` |
| `adapter_presentation` | `host_profile` / `host_presentation` |
| `known_adapter_presentations` | `known_host_profiles` |
| `WakeupRouter` | `ContinuationRouter` |
| `WakeupAdapter` | `ContinuationStrategy` |
| `GenericWakeupAdapter` | `ManualContinuationStrategy` |
| `WakePayload` | `ContinuationPayload` |
| `WakeResult` | `ContinuationResult` |
| `WakeReason` | `ContinuationReason` |
| `mcp_setup.rs` | `generic_mcp_install.rs` |
| `detect_mcp_clients` | `detect_generic_mcp_hosts` |
| `install_mcp_clients` | `install_generic_mcp_hosts` |
| `RAMBLEDESK_MCP_PORT` | `RAMBLEDESK_LOCAL_SERVER_PORT`，旧名兼容 |
| `RAMBLEDESK_MCP_TOKEN` | `RAMBLEDESK_LOCAL_SERVER_TOKEN`，旧名兼容 |

UI 文案允许：

- “适配器”
- “通用 MCP 适配器”
- “Pi 原生适配器”
- “检测到的 Coding 工具”
- “手动继续”

UI 文案避免：

- “MCP 服务”作为全局连接概念。
- “MCP 已连接”作为 titlebar/sidebar 状态。
- “Wakeup adapter”。
- 用 “Adapter” 指代宿主图标或宿主 label。

## 10. 废弃术语

| 术语 | 替代 |
| --- | --- |
| MCP service | 通用 MCP 适配器；讨论协议细节时说 MCP transport。 |
| Wakeup | 继续机制 / continuation。 |
| Host wakeup adapter | 继续策略 / continuation strategy。 |
| Adapter presentation | Host Profile / Host Presentation。 |
| Project / 项目 | 宿主会话、上下文 hint、context refs。 |
| Project root path / 项目路径 | `source_hint` 或 `context_refs`。 |
| Claude/Codex/OpenCode CLI resume 专用适配器 | 在原生继续被证明前，走通用 MCP 适配器。 |

## 11. 合并标准

基于本文做结构重构后，合并前必须满足：

- “适配器”只有一个产品含义：完整 agent-facing 集成流程。
- `core` 不包含 JSON/HTTP/MCP/Pi/local-server 逻辑。
- 本地 JSON API 位于独立本地服务 package。
- MCP 是薄适配层，不持有 listener、token path 或 JSON API。
- `rambledesk-hosts` 只持有 host profile 和策略选择，不实现完整适配器。
- 新协议不要求项目地址；现有 `project_*` 字段被标为遗留或完成迁移。
- Pi 被描述为 Pi 原生适配器，不被描述为 wakeup 实现。
- 通用 MCP 适配器明确使用手动继续，不承诺自动恢复原宿主上下文。
