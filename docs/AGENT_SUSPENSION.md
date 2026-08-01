# Agent 挂起、唤醒与 RambleDesk 接入

> 状态：Architecture direction  
> 日期：2026-08-01  
> 目的：记录 RambleDesk 在“向人类请求反馈后长期等待”场景中的运行时边界与跨 Agent 工具接入策略。

## 1. 问题不是超时，而是挂起语义

人类反馈的返回时间没有可靠上限。用户可能几分钟后回复，也可能正在工作、离线或睡眠八小时。因此，Agent 发出反馈请求后，正确状态不是“持续执行一个很长的工具调用”，而是：

1. 反馈请求已经可靠落盘；
2. 当前 Agent turn 明确结束或进入宿主提供的 suspended 状态；
3. 等待期间没有模型推理、定时轮询或长期占用的 MCP 请求；
4. 人类提交或取消后，由外部事件唤醒对应任务；
5. Agent 恢复后按 `request_id` 读取不可变 Feedback Package。

这里的核心约束是：

> **等待人类必须是零推理、零轮询、零长期工具调用的纯粹等待。**

延长 MCP timeout 只能推迟连接失败，不能提供这一语义。即使连接能够保持八小时，它仍然把 transport 生命周期误当成业务生命周期，也无法可靠跨越客户端退出、休眠、网络重置或工具调用上限。

## 2. 两个平面

RambleDesk 将接入拆成数据面与控制面：

```text
数据面（跨宿主一致）

Agent / Skill
     │
     ├── CLI ───────────────┐
     ├── MCP（可选）────────┤
     └── Local HTTP API ────┤
                            ▼
                    RambleDesk Local Server
                    Request / Draft / Package
                            │
                            ▼
                      durable storage

控制面（宿主相关）

Feedback terminal event
          │ request_id only
          ▼
     WakeupAdapter
          │
          ├── Codex resume / thread wake
          ├── Claude Code continuation mechanism
          ├── Pi triggerTurn
          ├── OpenCode prompt_async
          ├── Gemini resume mechanism
          └── notification / manual resume fallback
```

数据面回答“事实是什么”；控制面回答“哪个 Agent 任务现在应该继续”。两者不得合并成一条长期阻塞调用。

## 3. 推荐分层

### 3.1 Local Server：唯一运行时事实服务

本地服务持有统一的 application services 和 storage，负责：

- 创建、查询、取消 Feedback Request；
- 保存草稿与附件；
- 发布不可变 Feedback Package；
- 在请求进入终态时产生 terminal event；
- 根据凭据和订阅关系把 `request_id` 交给 WakeupAdapter。

服务不感知 Skill 文本，也不包含 Codex、Claude Code 或 Pi 的工作流判断。

### 3.2 CLI：最通用的客户端入口

CLI 面向任何能够执行本地命令的 Agent 工具，建议提供稳定、机器可读的命令：

```text
rambledesk feedback request --json < input.json
rambledesk feedback get <request_id> --json
rambledesk feedback cancel <request_id> --json
rambledesk integration register --host <host> --continuation <opaque-id>
```

CLI 通过 loopback API 调用 Local Server，不直接读写 SQLite。这样 CLI 是一个稳定协议客户端，而不是第二套业务实现。

### 3.3 MCP：可选的结构化 Agent 入口

MCP 仍然有价值：工具可发现、schema 明确、对支持良好的客户端接入自然。但它只负责短调用：

- `request_feedback`：创建并返回 durable handle；
- `get_feedback`：恢复或诊断时读取当前状态；
- `cancel_feedback`：取消；
- `wait_for_feedback`：仅作为兼容性或测试能力，不再作为默认长期等待路径。

MCP 不承担“让宿主进程长期休眠并在未来开启新 turn”的职责。该能力属于宿主或宿主适配器。

### 3.4 Skill：工作流合同

Skill 告诉 Agent 如何使用上述能力：

1. 收集必要上下文并创建请求；
2. 保存 `request_id` 与 continuation identity；
3. 明确停止当前工作，不主动轮询；
4. 被唤醒后调用 `get`；
5. 验证 terminal state 和 package identity 后继续工作。

Skill 本身不是 daemon，不能保持计时器、网络连接或后台进程。它也不能凭空赋予宿主“持久唤醒”能力。

### 3.5 WakeupAdapter：唯一的宿主差异层

WakeupAdapter 封装各 Agent 工具的恢复机制。每个适配器只需要实现小接口：

```text
register(request_id, continuation_identity, credentials?)
wake(request_id)
revoke(request_id)
health()
```

它传递的最小载荷是 `request_id`，不复制反馈正文和附件。恢复后的 Agent 必须回到数据面读取 canonical package。

## 4. 为什么是 Local Server + CLI + Skill

### 仅 CLI + Skill

优点是覆盖面最大、调试简单、无需 MCP。缺点是 CLI 进程退出后没有事件源，单靠 Skill 无法在八小时后唤醒已经结束的 Agent turn。因此它适合“创建/读取”数据面，不足以单独完成持久等待。

### Local Server + Skill

Local Server 可以持续观察终态并触发回调，是完整方案的运行时基础。但如果没有 CLI、MCP 或 HTTP 客户端合同，Skill 会与内部 API 细节耦合，安装和诊断体验也较差。

### MCP-only

MCP 是优秀的数据面适配器，但普通 tool call 的 timeout、连接与进程生命周期并不等同于宿主级 suspend/resume。不同客户端对 task、sampling、server notification 和工具取消的支持也不一致，因此不能把长期等待的正确性押在一个开放 MCP 调用上。

### 组合结论

采用：

- **Local Server** 作为事实与事件源；
- **CLI** 作为最低公分母和安装/诊断入口；
- **MCP** 作为支持客户端的可选原生入口；
- **Skill** 作为统一工作流；
- **WakeupAdapter** 作为唯一宿主相关层。

这不是为每个 coding agent 重写 RambleDesk，而是在稳定内核外提供很薄的 continuation adapter。

## 5. 生命周期

```text
Agent active
  │
  ├─ request feedback
  │    ├─ persist Request
  │    ├─ register continuation identity
  │    └─ return request_id
  │
  ├─ end current turn / host suspend
  │
  │       no model inference
  │       no polling
  │       no open MCP call
  │
  ▼
Human edits and submits
  │
  ├─ publish immutable Package
  ├─ commit terminal state
  └─ emit terminal(request_id)
          │
          ▼
     WakeupAdapter.wake
          │
          ▼
Agent resumed
  ├─ get feedback(request_id)
  ├─ verify package
  └─ continue original task
```

顺序要求：Package 发布和终态事务成功后才能唤醒；唤醒可以重复，结果读取必须幂等。若唤醒失败，请求仍然保持 completed，用户或 Agent 可手动恢复。

## 6. 能力分级与降级

每个宿主接入应声明能力，而不是伪装成完全一致：

| 等级 | 宿主能力 | 行为 |
|---|---|---|
| A | 可持久注册 continuation，并可从外部触发新 turn | 完整自动挂起/唤醒 |
| B | 可发异步消息，但是否开启新 turn 由宿主决定 | 尽力自动恢复，并保留 completed inbox |
| C | 只能执行 CLI/MCP，不能外部唤醒 | 创建后结束；通知人类手动恢复 |
| D | 连本地命令/API 都不可用 | 仅提供人工复制的 request/package 流程 |

首轮调研时应分别验证 Codex、Claude Code、Pi、OpenCode 和 Gemini 的真实等级，包括进程重启、机器休眠、八小时空闲、重复事件和凭据过期，而不是只验证一次短等待。

## 7. 不变量与禁止项

- Request、Draft、Package 不依赖某个 MCP session、CLI process 或 Agent turn 存活。
- continuation identity 是投递地址，不是业务主键；业务主键始终是 `request_id`。
- WakeupAdapter 不携带 canonical feedback payload。
- CLI 不直接访问数据库。
- Skill 不启动无限轮询，也不要求模型“继续耐心等待”。
- `wait_for_feedback` 不得被文档描述成八小时等待的默认保证。
- 任何 timeout 只结束一次 invocation attempt，不取消 Feedback Request。
- 自动唤醒失败不得丢失 completed 结果；Inbox 和手动 resume 永远可恢复。
- 宿主凭据必须局部保存、最小授权，并可单独撤销。

## 8. 建议实施顺序

1. 固化 CLI JSON 合同，并确保它只调用 Local Server。
2. 增加 terminal event/outbox，保证进程重启后仍可重试投递。
3. 定义 WakeupAdapter 接口、continuation registration 和 delivery attempt 诊断。
4. 先实现一个明确支持持久唤醒的宿主适配器，并加入重启/长等待测试。
5. 对其他宿主逐一实测，按 A–D 分级；无法自动唤醒时明确降级。
6. 将 MCP `wait_for_feedback` 调整为兼容/测试路径，更新产品文档的默认流程。
7. 发布统一 Skill：优先选择宿主原生 adapter，其次使用 CLI，MCP 仅作为可用的数据面入口。

## 9. 待调研问题

- Codex App Server / desktop task 是否提供稳定的 continuation 注册与外部唤醒 API？
- Claude Code 的 hooks、resume/session 与后台 command 能否在原进程退出后可靠恢复？
- Pi 的持久等待和 `triggerTurn` 的身份、鉴权、重启语义是什么？
- OpenCode `prompt_async`、Gemini resume 等机制是否允许第三方本地服务安全触发？
- 各宿主如何表达“当前 turn 已结束但任务仍等待外部事件”，以及 UI 如何避免把它显示成失败？
- 适配器分发应由 RambleDesk 安装器、独立插件还是各宿主的 Skill/extension 负责？

这些问题决定 WakeupAdapter 的具体实现，不改变本文的数据面和持久化边界。
