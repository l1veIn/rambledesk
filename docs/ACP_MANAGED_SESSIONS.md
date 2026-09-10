# ACP 托管会话

本文描述当前源码的使用方式与合同，不代表已发布安装包包含全部行为。术语见 [TERMINOLOGY.md](TERMINOLOGY.md)，架构决策见 [ADR 007](adr/007-acp-managed-sessions.md)，实测结果与未验项统一记录在[质量清单](quality/README.md)。

RambleDesk 通过 ACP 管理本机外部智能体的会话；智能体负责推理、文件读取和工具执行。一个会话有独立的 **Agent 页面**与 **Ramble 页面**：前者用于对话，后者用于体验、记录和提交反馈。[外部 Generic MCP、Pi、dsh 适配器](COMPATIBILITY.md)继续独立工作，不与托管会话混用身份或续接方式。

## 开始使用

1. 在「设置 → Agents」选择智能体。目录中的 npm 入口支持安装到应用自有目录，其他入口提供安装引导；自定义 ACP 入口通过「添加智能体」进入同一列表。
2. 按智能体自己的方式完成认证。命令、参数、环境变量与完整诊断放在高级设置；已有 Agent 凭据无须重复填写。安装、认证、ACP 握手、实际回复与反馈闭环是不同证据。
3. 新建会话，选择智能体和工作目录并输入任务。Web 中的目录是 **Backend Runtime 所在机器上的现有绝对目录**，不是浏览器设备上的路径。
4. 选择齐全后自动预连接，读取实际模型、模式、思考强度等选项；等待期间仍可编辑。Agent 没有提供的选项不显示，也不妨碍使用默认配置。连接失败保留输入。
5. 第一条真实任务被接受并持久化后，草稿沿用同一 ACP session 和 tab 转为正式会话，进入侧栏。没有反馈请求的正式会话仍正常显示。
6. Agent 发起 Ramble 请求后，在 Ramble 页面体验并提交。RambleDesk 等待当前轮次结束，再续接原 Agent 会话；“查看 Agent”打开或聚焦其唯一 Agent tab。

桌面与 Web 调用同一个 application 服务。切换客户端不会另建 Agent 会话；Web Access 的启停不拥有 Agent 生命周期。

## 连接与能力协商

设置页和新手引导共用一个检测状态卡片与折叠的高级设置。检测先发现启动入口与依赖，再进行 ACP 握手；Claude Code / Codex CLI 本体与 ACP Bridge 是不同入口，缺少 bridge 时应安装连接组件。程序缺失、运行时缺失和握手失败不等于认证失败。

用户选定项目后，真实会话读取 Agent 提供的选项，不另外发送测试 prompt。只有实际任务得到回复，才证明当次模型可用。`initialize.authMethods` 声明支持的认证方法，不证明已经登录；只有明确认证证据才提示登录或 API Key。未知错误保留未知，已回收的实例不能继续显示在线。

能力以当前连接的协商和运行时更新为准，不能由 Agent 名称、包版本或目录标签推断：

- 模型、模式及其他配置归一为 `SessionConfiguration.options`，通过 `{config_id, value}` 修改。ID 是连接内的不透明句柄，选项值原样回传；ACP 层保留实际 setter，UI 不选择协议方法，也不从名称推断权限含义。
- 同一连接中选项暂时消失再出现仍保留句柄；重新连接后重新协商，旧句柄不可用于新连接。Agent 确认的值和后续更新是事实来源。
- 普通检测结果只在同一个 Backend Runtime 的客户端连接内存中复用；修改启动配置或切换 runtime 后失效，不持久化为能力真值。缓存中的成功检查不等于仍有在线实例。
- 恢复、内容输入与交互能力分别验证。握手或能力声明不能代替真实调用，也不能证明工具沙箱允许反馈命令访问本地 IPC。

登录或配置修复不自动重发任务。发送确认丢失时先查询同一持久会话，状态未知时阻止再次发送；明确失败的消息可恢复至输入框，由用户重试。

## 草稿、运行与恢复

`prepareManagedSession({ agent_config_id, cwd })` 建立 `prepared` 记录并尝试连接，不发送 prompt、不进入正式列表；失败返回同一快照，`startManagedSession` 可以重试。首条真实用户消息的接受与持久化原子地转为 `active`；内部工作流说明不算用户任务。旧记录缺少 lifecycle 时按 active 兼容。

发送前更换智能体、目录或启动配置，会保留输入、回收旧预连接并重新准备。关闭未发送 tab 调用 `discardPreparedSession`，迟到的准备结果也必须清理。客户端可保存草稿文本与选择，不保存 prepared ID、令牌或运行实例；应用重启清理未发送资源，重新打开草稿时重新连接。

切换 tab 保留控制器和连接，只挂载当前视图。关闭正式会话 tab、取消当前轮次、停止 Agent 并保留历史、删除会话是不同动作。每个正式会话独占一个 ACP 实例，bridge 及其子进程属于该实例。

已绑定 `remote_session_id` 的恢复必须使用同一 ID 的 resume/load；不支持或恢复失败时保留原身份与原因，不能静默新建空白会话。重启不自动重发最后一条任务；尚未绑定远端身份的首次连接失败才可重试创建。

## 反馈命令与身份

托管 Agent 使用应用自带的无界面 `feedback` 命令，经实例私有 IPC 访问控制器，再由控制器调用会话专用 HTTP JSON API。HTTP URL/token 留在控制器内存中，不传给 Agent，不写入 token 文件。

| 运行时环境 | 含义 |
| --- | --- |
| `RAMBLEDESK_COMMAND` | 应用可执行文件的绝对路径；无需另装 CLI。 |
| `RAMBLEDESK_MANAGED_SESSION=1` | 标识托管启动，反馈不得回退为外部会话。 |
| `RAMBLEDESK_MANAGED_PI_ACTIVE=1` | 兼容旧 Pi 插件，阻止其注册外部反馈工具。 |
| `RAMBLEDESK_FEEDBACK_CHANNEL` | 实例独占的 Unix socket / Windows 本地命名管道地址。 |

这些值属于运行实例，不写入用户启动配置或会话活动。Unix socket 位于权限 0700 的临时目录；Windows 管道拒绝远程客户端。停止、删除或替换实例会撤销通道与凭据，恢复时重新签发。服务器从受控凭据确定归属，调用者不能指定另一个宿主会话；通道缺失、过期或访问受限均明确失败，不回退外部全局 API。

命令入口与 JSON 格式见[内置工作流](../crates/rambledesk-acp/src/feedback_workflow.md)：

- `feedback request --input <file|->`：提交结构化请求，`-` 表示标准输入。
- `feedback get --request-id <id>`：读取原请求及其反馈包。
- `feedback recover`：恢复当前归属中的请求，可显式给出 request ID。
- `feedback skip --reason user_opt_out|task_finished|request_cancelled`：确认用户明确授权的本轮例外。

每条真实用户 prompt 和反馈续接 prompt 都前置运行时工作流说明，不进入用户消息历史，不修改全局 Skills；准备阶段不发送说明。默认所有用户结果，包括只读分析和总结，都通过 Ramble 交接。正常结束且未尝试反馈时，公共驱动只追加一次交接提醒；已尝试失败或提醒后仍未交接，则保留回答并显示错误。用户取消不触发提醒，模型不能自行猜测用户已批准任务完成。读取批准或取消终态可确认本轮结束；普通反馈后的下一轮仍需交接。

Agent 必须用自己的执行工具调用命令，bridge 必须保留运行时环境，沙箱必须允许本地 IPC。RambleDesk 不宣告 ACP 客户端文件系统或终端执行能力。托管路径不注入 MCP server 或 Pi 托管扩展；Agent 原有 MCP、Skills 和插件仍按其自身配置加载。

当前随应用提供的 Pi/dsh 插件识别托管标记并跳过外部反馈注册。全局安装的旧 dsh 插件可能仍注册旧工具或指引，应用不会擅自升级它；不能把运行时要求使用统一命令等同于旧工具已被禁用。

## 对话与待处理交互

对话按真实 turn 展示输入、过程和最终回答。运行时默认展开过程，结束后收起；用户显式选择优先。历史分页保留阅读位置，收起的工作详情不挂载重内容，超长 turn 保留继续加载与重试入口。最终回答、错误、取消和待处理交互不会因过程折叠而消失；时间来自真实轮次标记，缺失时不估算。显示预算与来源见 [CODEG_PORTS.md](CODEG_PORTS.md#structured-session-activity)，实际资源表现见[质量清单](quality/README.md)。

待处理输入统一为 `SessionInteraction` 的 `permission`、`question`、`plan`，以匹配的响应类型提交，不转换成普通聊天文字或冒用权限 option ID。错误类型或无效值不消耗请求。当前支持会话内的 ACP form elicitation，以及 Grok/ Cursor 的问答与计划扩展，具体方法和实现边界见 [ACP crate](../crates/rambledesk-acp/README.md)。

表单支持扁平字符串、布尔、数值与选择数组，并校验必填项、原始选择值、类型、边界和唯一性。不支持的约束（例如 pattern、format）保留可见卡片与拒绝/取消入口，但不能接受。URL 与会话建立前的认证 elicitation 不在此界面范围内。计划批准必须显式决定；Grok 的继续规划响应不携带修订意见，意见应在下一条消息中提交。取消、断线、轮次结束与迟到请求均需释放待答资源。

Agent 输入框发送文本，附件在 Ramble 中添加；已有富内容历史和底层类型化 prompt 接口仍保留。上下文控件只显示 Agent `usage_update` 的 `used`、`size`，不代表累计 token 或费用；没有有效上报时隐藏，更换实例后等待新值，不从历史条数估算。

## 启动配置

目录是可连接入口，不是本机已安装数量。自动发现依次检查应用管理的版本、PATH/常见目录、当前 npm 全局前缀；不扫描 npx 缓存或猜测浏览器服务。未发现时可在高级设置指定入口。

DeepSeek 推荐入口为 **DeepSeek (DSH)**，应用管理安装包含所需 ACP 与 DSH 依赖，无需另启 `dsh web`。可以复用已有 `.dsh` 认证，但不承诺共享全部配置、插件与历史。已有自定义入口保留，不自动覆盖；当前包版本与启动参数以设置页和目录实现为准。

命令栏只填一个可执行名称或路径，参数分别填写，不进行 shell 展开；环境变量每行 `KEY=VALUE`，值中的 `=` 保留。目录身份由 `AgentConfig.catalog_id` 显式关联，运行时不靠路径或标签猜测。同一配置可用于不同项目，工作目录属于会话。

配置与环境值保存到本机 SQLite，界面遮盖和诊断脱敏不等于加密凭据库。运行中的实例继续使用启动时配置；修改后须停止并重新启动才应用新值。已有会话引用的配置不能删除或改为另一个后端标识。

## 反馈投递与删除

请求终态与投递状态分别持久化。提交或批准时，终态与待投递记录一起保存；取消只保存取消终态和反馈包，不创建托管投递、不续接 Agent。外部原生适配器在 tool call 内等待时不会额外收到一次托管续接。

| 投递状态 | 含义 |
| --- | --- |
| `pending` | 等待原会话可接收输入；忙碌、停止或断开时不改投其他会话。 |
| `sending` | 已取得本次执行权，正在续接或保存执行结果。 |
| `delivered` | 续接轮次成功结束，或用户确认已处理；不等于整个任务完成。 |
| `uncertain` | 无法确定已处理多少内容，只能由用户明确重试或确认。 |
| `discarded` | 删除等操作撤销投递，不再续接。 |

执行已有结果但保存失败时，只重试保存结果，不重发 prompt；重启将遗留 `sending` 标为 `uncertain`。删除先保存意图、封锁输入与反馈、撤销凭据，再回收本实例及所属记录；失败保留状态供重试。项目目录、Agent 自己的历史和用户独立启动的服务不在删除范围内。

## 验证入口

[后端探针](ACP_BACKEND_PROBE.md)说明如何验证真实模型、命令、双会话隔离与原 ID 恢复。当前真实 Agent 闭环的失败阶段及待验项以[质量清单](quality/README.md)为准；旧 MCP 路径成功、握手、fixture 和浏览器操作不互相替代。

不调用模型的 UI/application 预览可先构建 Web，再设置 `RAMBLEDESK_MANAGED_PREVIEW=1`，运行 `cargo run -p rambledesk-local-server --example managed_preview`。它使用临时数据库与测试 Agent，不访问用户数据库；另见[开发预览](../apps/desktop/src/dev/README.md)。正常退出清理自有资源。
