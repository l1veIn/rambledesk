# Codeg ACP 动态配置与 Timeline 实现调研

> 调研日期：2026-08-31
>
> Codeg 基线：[`769610c626f1fc4b18c11d3e289326acf097b99f`](https://github.com/xintaofei/codeg/tree/769610c626f1fc4b18c11d3e289326acf097b99f)（v0.29.0）
> 范围：Launch 顺序、Agent 动态配置、运行状态、Timeline/窥视孔、Turn 聚合与折叠。只引用 Codeg 当前源码。

## 结论

RambleDesk 应复刻的不是一个写死了“模型 / 思考强度 / 访问权限”的 Launch 表单，而是下面这条依赖链：

```text
Workspace → Agent → 临时 ACP session 探测 → AgentOptionsSnapshot → 动态表单 → 正式 session
```

同时必须区分两类合同：

- `AgentLaunchProfile`：RambleDesk 静态维护的安装、启动命令、认证、传输方式、兼容性 quirks，以及极少数只能在进程启动时传入的选项。
- `AgentOptionsSnapshot`：在已经选定的 Workspace 下，由 Agent 的 ACP session 实时报告的模式、配置项、命令和 prompt 能力。模型、思考强度等都只是其中的普通配置项，不应由 RambleDesk 假定其必然存在。

Codeg 的 Timeline 也不是 ACP 自动提供的永久历史。它把 ACP 事件投影成当前 session 的实时视图；已结束会话的完整历史，则依靠大量 Agent 专用的本地 transcript 解析器恢复。因此，RambleDesk 可以先稳定承诺“活跃 session 的实时窥视孔”，但若不自行持久化 Timeline 或编写 Agent transcript 解析器，就不应承诺重启后仍有完整历史 Timeline。

## 1. Workspace-first Launch

Codeg 先让用户打开工作目录；会话连接的 `workingDir` 来自已选 folder，而不是在 Agent 配置之后补填。相关入口是 [`WorkspaceFolderDialog.commitRoot`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/layout/workspace-folder-dialog.tsx#L84-L190)，会话页在创建连接之前先得到 `workingDirForConnection`，见 [`ConversationTabView`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/conversations/conversation-detail-panel.tsx#L561-L590)。

Agent 选定后，前端调用 `acp_describe_agent_options`。后端使用与真实连接相同的 working directory 和 runtime environment，启动一个临时的新 session 读取 Agent 选项，见 [`acp_describe_agent_options_core`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/commands/acp.rs#L10069-L10112) 与 [`AcpManager::probe_agent_options`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/manager.rs#L2017-L2110)。探测有以下重要语义：

- 同一 Agent 的探测通过互斥锁避免并发启动多个进程。
- 最长等待 60 秒，成功或失败后都断开临时连接。
- 等待 `SelectorsReady`，并给早到事件 500ms 的收集窗口；“Agent 明确返回空配置”与“超时”是不同结果，见 [`wait_for_session_options`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/manager.rs#L2112-L2190)。

前端探测缓存也绑定 `(agent, folderPath)`，具有 30 秒 TTL、请求合并、250ms debounce 和旧响应丢弃，见 [`useAgentOptions`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/automations/use-agent-options.ts#L1-L186)。这意味着 RambleDesk 不能只按 Agent 缓存配置；切换 Workspace 或 Agent 时，必须丢弃当前 schema 和选中值并重新探测。

建议 Launch 状态机：

```text
choose_workspace
  → choose_agent
  → probing_options
  → configure_returned_options
  → launching
  → session_started | launch_failed
```

## 2. Agent profile 与动态 config schema

### 2.1 ACP 返回合同

Codeg 的前端类型定义在 [`SessionConfigOptionInfo` 与 `AgentOptionsSnapshot`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/lib/types.ts#L1200-L1256)：

```ts
type SessionConfigKindInfo =
  | { type: "select"; current_value: string; options: SelectOption[]; groups: SelectGroup[] }
  | { type: "boolean"; current_value: boolean };

interface SessionConfigOptionInfo {
  id: string;
  name: string;
  description?: string;
  category?: string;
  kind: SessionConfigKindInfo;
}

interface AgentOptionsSnapshot {
  modes: SessionModeStateInfo | null;
  config_options: SessionConfigOptionInfo[];
  available_commands: AvailableCommandInfo[];
  prompt_capabilities: PromptCapabilitiesInfo | null;
}
```

后端保留 Agent 给出的 `id/name/description/category`、select 分组和值，并把 boolean 单独编码，见 [`map_session_config_option` / `map_session_config_options`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L2595-L2711)。当前仅支持 `select` 与 `boolean`；未知 kind 会在反序列化前被过滤，避免一个新类型破坏整场会话，见 [`KNOWN_CONFIG_OPTION_KINDS` / `strip_unknown_config_options`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L2658-L2702)。

因此这里的“动态 schema”不是任意 JSON Schema。可动态的是字段数量、字段 ID、文案、分类、值与分组；控件类型目前只有 select 和 boolean。RambleDesk 应保存原始不透明值，同时对未知 kind 进行可观测的降级，而不是让整个 Launch 失败。

### 2.2 动态渲染

Codeg 的通用控件 [`SessionConfigSelector`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/chat/session-config-selector.tsx#L1-L180) 对每个返回项按 kind 渲染：select 支持 option group，boolean 渲染开关。消息输入区的 [`inlineSelectorItems` / `collapsedSettings`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/chat/message-input.tsx#L1439-L1585) 遍历所有选项；`model` 只有长列表搜索等体验优化，不是固定字段。

自动化表单也明确把 model 视为 `id/category === "model"` 的普通 config option，并在 Agent 返回 config options 时避免再重复显示旧的 mode 行，见 [`AgentConfigSection`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/automations/agent-config-section.tsx#L36-L145)。但该自动化表单当前只渲染 select；boolean 的完整支持存在于会话输入区。这是复刻时不能照搬的局部限制。

设置某项后，Codeg 把 value 当作不透明字符串发送给 Agent；Agent 响应携带的完整 `configOptions` 才是新的权威状态。Codeg 还会检查“请求值”与“实际返回值”，拒绝时保留 Agent 的实际值。相关流程见 [`set_session_config_option_inner` 与 `apply_preferred_session_options`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L6238-L6483)。因此 RambleDesk 不应只在本地乐观修改单一字段；每次设置后都应替换整份 snapshot。

### 2.3 不能动态化的兼容层

Codeg 也不是纯粹依靠 ACP schema。部分 Agent 的选项只能在启动进程时注入：

- Codex 通过 `INITIAL_AGENT_MODE` 初始化模式，见 [`apply_codex_env_policy`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L758-L803)。
- Grok 使用根进程 `--permission-mode`，因为它没有对应的 ACP mode channel，见 [`build_agent` 的 Grok Uvx 分支](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L1495-L1550)。
- Cursor 的 `--force` 和 `--model` 也是启动参数，见 [`build_agent` 的 Cursor binary 分支](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L1640-L1694)。
- 另一些 Agent（如 DeepSeek、Qoder、Antigravity）则由标准 config options 报告 model/mode/reasoning，见它们的 [`AgentRegistry` 条目](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/registry.rs#L1096-L1260)。

RambleDesk 因而需要静态 `AgentLaunchProfile`，但其职责应收窄为：安装/诊断、spawn、认证、启动期参数和兼容性规则。不要把 Agent 本可动态报告的 model/reasoning/access choices 再复制进静态 profile。启动期字段发生变化后，应以新参数重新探测。

## 3. Session 执行状态

ACP 连接层的实时状态是 `connecting | connected | prompting | disconnected | error`，见 [`ConnectionStatus`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/lib/types.ts#L1078-L1084)。Codeg 在发出用户 prompt 前切换为 `Prompting`，见 [`run_conversation_loop` 的 Prompting 事件](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L8008-L8048)，Turn 结束后回到 `Connected`，见同一流程的 [`TurnComplete`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/connection.rs#L8728-L8750)。

Codeg 左栏卡片实际用持久化业务状态 `conversation.status === "in_progress"` 显示 spinner，而不是直接展示 ACP connection status，见 [`SidebarConversationCard.isRunning`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/conversations/sidebar-conversation-card.tsx#L304-L315) 和 [spinner 渲染](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/conversations/sidebar-conversation-card.tsx#L502-L540)。其宠物状态投影还规定 `Failed > 等待授权 > prompting > Idle` 的优先级，见 [`compute_pet_state`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/pet_state_mapper.rs#L242-L276)。

RambleDesk 应将“业务 session 状态”和“实时活动状态”分开保存，再投影成左栏图标：

| 条件（从高到低） | Session 列表表现 |
| --- | --- |
| Permission / Ask Question / Feedback Request 待人处理 | 等待用户；不再显示泛化 loading |
| error | 错误提示 |
| connecting | 连接中 spinner |
| prompting | Agent 正在运行 spinner |
| connected 且无待办 | 空闲/已连接 |
| disconnected | 已停止或可恢复 |

## 4. Timeline 与窥视孔

### 4.1 实时数据模型

Codeg 为 ACP 事件加上单调 `seq` 与 `connection_id`，事件类型包含文本/思考增量、tool call/update、permission、turn complete、session/config/status/error 等，见 [`EventEnvelope` / `AcpEvent`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/types.rs#L99-L285)。`SessionState` 明确只保存当前 Turn 的 in-flight 状态、活动工具、待回答请求、配置和能力；已完成 Turn 不由它持有，见 [`SessionState`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/session_state.rs#L240-L350)。

前端 [`LiveTranscriptView`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/message/live-transcript-view.tsx#L1-L335) 把 connection 的 snapshot/liveMessage/status 投影进同一套只读 `MessageListView`，同时把 Permission、AskQuestion、PlanApproval 响应仍路由回被查看的 connection。“只读”只限制自由输入，不应让待处理请求失去操作能力。

### 4.2 窥视孔 UI

Codeg 的 [`SubAgentSessionDialog`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/message/sub-agent-session-dialog.tsx#L1-L105) 是右侧非模态 Drawer：打开 Timeline 不会阻塞主工作区，也不会停止 Agent；可以继续打开嵌套 session。任务版 [`TaskTranscriptDialog`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/tasks/task-transcript-dialog.tsx#L1-L120) 在任务仍活跃时 attach/hydrate live connection，结束后切到持久 transcript。

打开状态被提升到虚拟列表之上的 [`SessionViewerHost`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/message/session-viewer-host.tsx#L1-L150)，所以来源卡片滚出 viewport 不会关闭 Drawer；每个层级只保留一个 viewer，嵌套 viewer 由内部再创建一层 host。

RambleDesk 的最小复刻合同：

1. 从 Session 列表的运行状态或详情入口打开右侧非模态 Drawer。
2. 关闭 Drawer 只关闭视图，不取消 session。
3. Header 显示 Agent logo、session 名称和实时状态。
4. 使用统一只读 Timeline renderer；thinking、tools、permission、ask question、feedback request 都来自同一条有序事件流。
5. 待处理请求在 Timeline 中仍可操作，并跳转/关联到第二栏对应 Request。
6. Viewer 状态高于虚拟化 row；用 session/connection identity 做 attach 与 snapshot hydrate。

## 5. Turn 聚合与完成后折叠

Codeg 不会把每个工具事件当成独立 Turn。[`mergeConsecutiveAssistantTurns`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/message/message-list-view.tsx#L430-L625) 将连续 assistant turns 合并为一段回复；空 Turn 对分组透明，并汇总 tools、delegation/background polls、goal runs、耗时、模型和来源 Turn。

折叠不是“Turn 一完成就立刻收起”。[`ReplyFoldState` / `advanceReplyFold`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/message/message-list-view.tsx#L180-L325) 的规则是：

- 当前 round 始终展开。
- 当前 round settle 时不自行折叠，避免用户正在阅读的内容突然跳动。
- 用户开始下一次发送，或检测到下一次 running edge 时，才折叠之前的工作内容。
- run identity 重绑定不重置折叠 epoch。

[`CompletedTurnContent`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/message/completed-turn-content.tsx#L1-L282) 使用 [`splitTrailingAnswerParts`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/lib/adapters/ai-elements-adapter.ts#L1600-L1648) 把思考、工具调用和中间进度放进可折叠的 “Worked …”，把最后一段用户可读 answer 留在外面。纯文本回复因无法可靠区分进度与答案而保持原状；工具-only、失败或 plan card 也不会折叠成一个空壳。

在 RambleDesk 中，对等的终点不是普通自然语言 answer，而是结构化 `request_feedback`：

- 活跃 Turn：thinking、工具和中间报告保持展开。
- Turn 发出 `request_feedback`：把它视为 terminal result；Timeline 显示精简 Request 卡片/链接，完整交互仍在第二、三栏。
- 下一 round 开始：上一 round 的 thinking/tools/intermediate content 折叠为 “工作过程”。
- Turn 未产生结构化请求即失败或结束：必须保留失败原因或最后报告，不能折叠成空白。

## 6. 历史 Timeline 的真实边界

Codeg 的已完成历史并非来自 ACP 查询。`get_folder_conversation_core` 从 DB 读会话标识和摘要，然后选择 Agent 专用 parser 去解析各 Agent 自己保存的 session 文件，见 [`get_folder_conversation_core`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/commands/conversations.rs#L1100-L1280)。这也解释了 `SessionState` 为什么只负责当前 Turn。

所以 RambleDesk 有三种明确选择：

1. **首版建议：实时窥视孔。** 活跃 connection 和当前 snapshot 可查看；进程重启后的完整 Timeline 不作稳定承诺。
2. **持久化标准化 Timeline projection。** RambleDesk 自己存有序事件/Turn 投影，能提供跨 Agent 的稳定历史，但会新增存储、迁移和兼容责任。
3. **复刻 Codeg 的 Agent transcript adapters。** 不复制 transcript，但要长期维护每个 Agent 的文件位置与解析器；这不是 ACP-first 的通用能力。

当前“不重复保存 Agent transcript”的产品选择与方案 1 最一致。可以持久化 RambleDesk 自己拥有的 Request、用户回复、session identity 和必要运行摘要；Timeline 明确标注为运行观察界面，而不是永久审计日志。

## 7. 建议直接落地的接口

```ts
interface AgentLaunchProfile {
  agentId: string;
  displayName: string;
  logo: string;
  installAndDiagnose: InstallContract;
  spawn: SpawnContract;
  launchOnlyOptions: LaunchOption[];
  quirks: string[];
}

interface AgentOptionsSnapshot {
  workspace: string;
  agentId: string;
  agentVersion?: string;
  modes: SessionModeStateInfo | null;
  configOptions: SessionConfigOptionInfo[];
  availableCommands: AvailableCommandInfo[];
  promptCapabilities: PromptCapabilitiesInfo | null;
}

type SessionActivity =
  | "connecting"
  | "running"
  | "waiting_permission"
  | "waiting_question"
  | "waiting_feedback"
  | "idle"
  | "disconnected"
  | "error";
```

实现约束：

- schema cache key 至少包含 workspace、Agent 和已安装版本。
- Agent/Workspace/启动期参数改变时，取消或忽略旧 probe，并清空旧 selections。
- 表单按 Agent 返回顺序和文案渲染；category 只用于分组/图标优化，不决定字段是否存在。
- setting response 的整份 snapshot 是权威值。
- 未知 kind 可显示“当前版本暂不支持此配置”，其余配置仍可使用。
- Timeline 事件必须以 connection identity + 单调序号去重、排序；Request 事件保存其 RambleDesk request id，便于跳转。
- 运行状态是 live projection，不用它覆盖持久化 session 生命周期。
