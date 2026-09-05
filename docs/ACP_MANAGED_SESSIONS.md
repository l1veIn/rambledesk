# ACP 托管会话

> 状态：CURRENT，体验重设计已实现，Windows 自动化与隔离浏览器验收已完成；不表示此前发布的安装包包含这些行为。
> 更新：2026-09-05。当前验证环境为 Windows；本轮尚未完成统一命令路径的真实 Agent 模型闭环、Linux/macOS 与签名安装包验收。
> 术语见 [TERMINOLOGY.md](TERMINOLOGY.md)，架构见 [ADR 007](adr/007-acp-managed-sessions.md)，进度见 [体验重设计计划](ACP_EXPERIENCE_REDESIGN_PLAN.md)。

RambleDesk 通过 ACP 管理设备上的外部智能体会话，智能体负责推理、文件读取和工具执行。一个业务会话可以有独立的 **Agent 页面**和 **Ramble 页面**：前者用于对话，后者用于体验、记录和提交反馈。外部客户端仍可通过 Generic MCP、Pi、dsh 等适配器请求反馈；这些外部接入与托管 ACP 会话分别路由。

## 开始使用

1. 在「设置 → Agents」查看设备上的智能体、安装状态和版本。目录中的 npm 入口支持安装到 RambleDesk 自有目录，其他入口提供安装引导。自定义 ACP 入口通过「添加智能体」进入同一列表，命令、参数和环境变量放在详情的高级设置中。
2. 按智能体自己的方式完成认证，或保存其支持的密钥、地址等连接字段。完成后即可选择启动，不再需要额外点击“使用此智能体”或“启用”。安装、认证、ACP 握手和完整反馈闭环是不同证据。
3. 点击新建会话，立即进入草稿 tab，选择智能体和工作目录并输入任务，可复用最近选择。桌面可浏览选择目录；Web 使用 **Backend Runtime 所在机器上的现有绝对目录**。
4. 选择齐全后自动预连接，加载 Agent 实际提供的模型、模式、思考强度等选项；等待期间仍可编辑任务。没有对应选项的 Agent 不显示虚构控件。连接失败保留输入，修复后可重试。
5. 发送第一条真实任务后，草稿沿用同一 ACP session 和同一 tab 转为正式会话，并从任务生成后备标题。正式会话才进入侧栏；有对话但没有 Ramble 请求的正式会话仍正常显示。
6. Agent 发起 Ramble 请求后，在 Ramble 页面体验、记录和提交反馈。RambleDesk 等待当前轮次结束，再续接原 Agent 会话。“查看 Agent”入口打开或聚焦该会话唯一的 Agent tab。

桌面与 Web 调用同一个 application 服务。切换客户端不会创建另一个 Agent 会话；Web Access 的启停不拥有 Agent 生命周期。

## 两种视图与草稿生命周期

Agent tab 承载对话、运行状态、动态选项、权限与恢复。Ramble 会话页和 Task Preview 聚焦反馈请求，通过可信 `managed_session_id` 显示来源入口，并按需读取投递和删除状态；浏览请求不加载 Agent 历史或全部 Agent 配置。左侧 Ramble 会话的默认点击语义保持不变。

`prepareManagedSession({ agent_config_id, cwd })` 建立 `lifecycle: prepared` 的内部记录并尝试连接。失败也返回同一 prepared snapshot，`startManagedSession` 可以重试。准备阶段不发送 prompt，不进入正式导航列表。首条真实用户消息的接受与持久化将其原子转为 `active`；内部工作流说明不算用户任务，也不参与命名。旧记录缺少 lifecycle 时按 active 兼容。

发送前切换智能体、目录，或返回时发现所选连接已修改，会保留输入并回收旧预连接，再建立新连接。关闭未发送 tab 调用 `discardPreparedSession`；迟到的 prepare 结果仍须清理。发送结果暂时无法确认时，先读取同一会话状态，不能通过重发或另建会话猜测成功。

tab 切换保留草稿控制器和连接，界面只挂载当前视图。客户端可保存草稿文本与选择，但不保存 prepared session ID、令牌或运行实例；应用重启清理未发送的准备资源，重新打开草稿时建立新预连接。已转正会话关闭 tab 只关闭视图，继续运行。

## 统一托管反馈命令

生产托管路径使用应用自带的无界面 `feedback` 命令，通过 local-server 的会话专用 HTTP JSON API 工作，不再按 Agent 选择 HTTP MCP、stdio companion 或 Pi 托管扩展。智能体原有 MCP、Skills 与原生插件仍由其自身配置机制加载；它们不决定 RambleDesk 的托管会话身份。

| 运行时环境变量 | 作用 |
| --- | --- |
| `RAMBLEDESK_COMMAND` | 应用可执行文件的绝对路径；发布应用自带该命令，不要求用户另装 CLI。 |
| `RAMBLEDESK_MANAGED_SESSION=1` | 标识托管启动，旧反馈出口不得回退为外部会话。 |
| `RAMBLEDESK_MANAGED_PI_ACTIVE=1` | 兼容已安装的旧 Pi 插件入口，阻止其注册外部反馈工具；不加载旧扩展。 |
| `RAMBLEDESK_FEEDBACK_URL` | 当前运行时的 `/agent-feedback` 基地址。 |
| `RAMBLEDESK_FEEDBACK_TOKEN` | 固定绑定该本地会话的私有凭据。 |

私有环境只属于运行实例，不写入用户启动配置或会话活动。停止、删除、替换实例时撤销，恢复时重新签发。服务器从凭据确定归属，调用者不能传入另一个宿主会话 ID；缺少或失效的身份不能回退到外部全局 API。

当前随应用提供的 Pi/dsh 插件识别托管标记并跳过外部反馈注册；旧 Pi 插件通过兼容标记处理。已全局安装的旧 dsh 插件没有环境退出开关，可能仍注册外部工具或附加旧指引。运行时明确要求使用通用命令，但不会擅自升级或修改全局插件，因此不能宣称旧 dsh 工具已被物理禁用；此组合仍需真实模型验收。

命令入口为 `feedback request --input <file|->`、`feedback get --request-id <id>`、`feedback recover`。request 接受结构化 JSON；`--input -` 从标准输入读取，recover 可省略 request ID。三者分别调用 `/agent-feedback/request`、`/agent-feedback/get`、`/agent-feedback/recover`。详细说明由[内置工作流](../crates/rambledesk-acp/src/feedback_workflow.md)提供。

ACP v1 没有统一 system prompt 或任意工具注册字段。RambleDesk 在每条真实用户 prompt 和反馈续接 prompt 前置运行时上下文文本，说明何时请求体验、如何调用命令、创建请求后结束当前轮次，以及收到续接后读取反馈并继续；这部分不进入用户消息历史。prepare 本身不发送引导 prompt，也不修改用户全局 Skills。

Agent 需要通过自己的执行工具调用该程序，bridge 需要保留运行时环境，进程还须能够访问会话 API。ACP 握手、MCP capability 或安装检测不能独立证明这些条件已满足。RambleDesk 当前不宣告 ACP 客户端文件系统或终端执行能力。

## Agent 页面与能力边界

- 对话按真实 turn 组织用户输入、工作过程和最终回答。运行中默认展开过程，每轮结束自动收起工作详情；用户显式选择的展开/收起状态优先并保持。历史过程默认收起，收起时不挂载重内容；最终回答、错误、取消和待处理权限仍有明确展示。
- 复制读取实际回答文本；完成时间和工作耗时来自持久化的真实 turn 标记。缺少起止边界时不推算耗时。历史分页保留轮次内容和阅读锚点，避免将跨页片段冒充完整轮次。
- 模型、模式、思考强度及其他配置由 Agent 提供并确认。支持能力协商允许的图片、UTF-8 文本附件和资源引用，不提供未实现的音频输入。
- 上下文控件仅显示 Agent `usage_update` 上报的 `used`、`size` 及其百分比。它是当前上下文占用，不是累计输入/输出 token、费用或整段任务消耗；当前未接入这些其他统计。没有上报或容量无效时隐藏，重启或换实例后等待新上报，不从历史条数估算。

按需挂载和视图分离已经实现；实际速度或内存改善仍需同环境的性能对照，不能从源码结构直接宣称测量结果。

## 启动配置与身份

命令栏只填写一个可执行程序名称或路径，每个参数单独一行，不进行 shell 展开。环境变量每行使用 `KEY=VALUE`，值中的 `=` 保留。工作目录属于会话，同一配置可服务不同项目。

目录连接通过 `AgentConfig.catalog_id` 显式关联。迁移只对历史配置做一次保守识别，无法确认的保留为自定义智能体；运行时不再靠命令路径、宿主标签或参数猜身份。同一目录入口的多个连接作为不同选项展示。选择历史停用的连接即表示本次启动意图：保留其自定义参数与环境，仅恢复可启动状态，不要求经过第二个启用步骤。

配置参数和环境值保存在本机 SQLite；界面隐藏和诊断脱敏不等于加密凭据库。已有 Agent 凭据文件或应用启动环境中的密钥，无须重复填写。正式会话运行中的实例继续使用启动时配置，修改后显示变化提示；停止后再次启动应用新配置。已有会话引用的配置不能删除或改为另一个后端标识。

## 反馈投递与资源收尾

请求终态与投递状态分别持久化。托管请求提交、批准或取消时，终态与待投递记录一起保存；外部原生适配器在 tool call 内等待时不会额外收到一次托管续接。

| 状态 | 含义与操作 |
| --- | --- |
| `pending` | 等待原会话可接收输入；忙碌、停止或断开时不发往其他会话。 |
| `sending` | 已取得本次投递执行权，执行续接轮次或等待其结果落盘。 |
| `delivered` | 续接轮次成功结束，或用户确认已处理；不表示整个任务完成。 |
| `uncertain` | 发送或执行中断，无法确定已处理多少内容；仅由用户明确重试或确认。 |
| `discarded` | 删除等流程撤销投递，不再续接。 |

续接已有结果但暂时无法保存时，只重试保存同一次执行结果，不重新发送 prompt。应用重启将遗留 sending 标为 uncertain，避免盲目重放。

一个正式会话独占一个 ACP 实例，bridge 与多个子进程仍属于该实例。关闭正式视图、取消当前轮次、停止 Agent 并保留历史、删除会话是不同动作。删除先保存意图、封锁输入与反馈、撤销凭据，再回收本实例和所属记录；失败保留删除状态供重试。项目目录、Agent 自己的历史文件和用户独立启动的服务不属于删除范围。

已绑定 `remote_session_id` 的恢复必须使用该 ID 的 resume/load；失败保留原身份和原因，不能静默创建空白会话。重启不自动重发最后一条任务。尚未成功绑定远端身份的首次连接失败可以重试创建。

## 验证范围与开发入口

2026-09-04 的社区 DeepSeek ACP 0.8.0、官方 dsh 0.1.2-rc.1 曾完成真实双项目模型反馈闭环、正常重启后的原 ID 恢复和删除隔离。这些报告测试的是当时的 MCP 托管路径。随后 Pi 的离线扩展与 Codex 的安装/握手记录也属于前一阶段证据，**不能据此宣称本轮统一命令路径已经完成真实模型验收**。版本、报告与限制见 [ACP_BACKEND_PROBE.md](ACP_BACKEND_PROBE.md) 和 [Codeg 历史验收](CODEG_ACCEPTANCE.md)。

本轮已完成 Windows 自动化测试与真实 HTTP/ACP 夹具的浏览器操作验收，覆盖命令分派、HTTP 会话隔离与撤销、prepared 生命周期、草稿竞态、视图查询边界、轮次展示和真实 usage 映射；详细结果见[本轮计划](ACP_EXPERIENCE_REDESIGN_PLAN.md)。尚未完成新路径真实 Agent 模型闭环、原生 Desktop 完整人工流程、Linux/macOS 实跑、签名安装包以及性能对照，不将夹具当作这些验收。

`cargo run -p rambledesk-local-server --example managed_loop` 是需显式授权模型调用的探针入口，配置和历史证据见后端记录。该示例本身也分派共享反馈命令，当前路径重跑不复用历史报告中的 MCP 结论。

不调用模型的隔离 UI fixture 可先构建 Web，再设置 `RAMBLEDESK_MANAGED_PREVIEW=1` 并运行 `cargo run -p rambledesk-local-server --example managed_preview`。其临时数据库、测试 Agent 与浏览器令牌仅用于 UI/application 接线，不访问用户数据库。也可使用[开发预览](../apps/desktop/src/dev/README.md)。预览进程正常退出时清理自有资源。
