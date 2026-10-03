# RambleDesk 架构

本文描述当前实现的运行边界和所有权；词汇以[术语表](TERMINOLOGY.md)为准。用户旅程见[产品文档](PRODUCT.md)，构建方法见[开发指南](DEVELOPMENT.md)，平台支持与待验项见[质量清单](quality/README.md)。历史 ADR 记录决策，不能把其中的目标状态直接当成当前支持承诺。

## 运行时拓扑

```text
外部 Host Adapters ── MCP / Local JSON ── Local Integration Server ─┐
Desktop Client ────── Tauri Application Transport ─────────────────┤
Web Client ────────── Web Access HTTP + WebSocket ─────────────────┤
                                                                  ▼
                         Backend Runtime / ApplicationCommandFacade
                         core + storage + config + session controllers
                           └─ AcpSessionDriver → owned ACP Instance → Agent Backend
                                                └─ feedback command → private IPC relay
                                                     → /agent-feedback/* → application

Desktop / Browser Capability → 当前客户端 Platform Plugin
                             → SpeechEvent / Attachment Candidate → Draft 正文 / 工作台字段
```

`apps/desktop` 是完整产品的 composition root。每个 desktop 进程创建一份 Backend Runtime/application facade，由 Tauri state、该进程的 Local Integration Server 和可选 Web Access 共用；不是跨进程全局单例。

Local Integration Server 的独立 loopback listener 承载 `/api`、`/mcp`、托管会话专用 `/agent-feedback/*`，并保留旧 `/mcp-managed` 兼容入口。它还公开无凭证、无反馈数据的 `/web-review/bridge.js` 静态桥接脚本，供待评审项目自行托管；不提供 Workbench 应用静态资源、application routes 或 WebSocket。MCP SSE 不是 Web Client 事件流。

Web Access 默认关闭，使用另一 listener、credential、auth domain、route set 和启停生命周期；默认 `127.0.0.1:37643`。它复用同一 application module，停止它不停止 Backend Runtime 或 Local Integration Server。设置支持下次启动使用的新端口及可选随 Desktop 启动，均不改变 loopback 边界。

CLI 的 `serve` 仍可组装 SQLite 与 Local Integration Server，供开发诊断；它不是完整的 headless Web/Agent 产品。应用内置 `feedback` 命令只是已有托管运行时的客户端，不启动 UI、后端或业务数据库。

## 模块与依赖

| 位置 | 当前职责与边界 |
| --- | --- |
| `crates/rambledesk-core` | Feedback、工作台类型与结果、workspace、Agent Session、Activity、Delivery、Recovery、Deletion 的 DTO、application use cases 与 ports；工作台提交规则仅处理类型化领域值。不依赖 HTTP/JSON/MCP/ACP SDK、Tauri、SQLite、宿主安装或文件路径推断。 |
| `crates/rambledesk-storage` | SQLite、migrations、草稿 JSON envelope 编解码、附件元数据、会话事实、发布对账及所属文件清理；依赖 core，不持有宿主协议或另一套提交领域规则。 |
| `crates/rambledesk-acp` | 官方 ACP SDK、stdio driver、能力协商、权限回调、实例与进程树生命周期。依赖 core 和 feedback-client；不持有 SQLite、HTTP 路由或 Tauri UI，也不承接客户端文件/终端执行能力。 |
| `crates/rambledesk-feedback-client` | 桌面/CLI 共用的反馈命令、输入验证、本地 IPC relay/client 和 scoped HTTP 客户端；依赖 core。relay 由 ACP 实例拥有，不拥有产品会话或推理。 |
| `crates/rambledesk-local-server` | Local Integration 与独立 Web Access 的 listener、路由、认证、安全策略、静态资源与事件服务；依赖 core、mcp，不实现领域规则。 |
| `crates/rambledesk-mcp` | Generic MCP schema、handlers、instructions、结果映射及客户端检测/安装执行引擎；依赖 core、hosts，不持有 listener 或 JSON API。 |
| `crates/rambledesk-hosts` | Host Profile、程序/配置知识、标签、默认适配器、continuation strategy；依赖 core，不执行安装或宿主协议。 |
| `crates/rambledesk-speech` | Desktop 本地语音实现与资源；不成为跨客户端音频服务。 |
| `packages/pi-rambledesk` / `packages/dsh-rambledesk` | 外部宿主原生反馈适配器，通过本地 JSON API 等待和恢复；不属于 ACP Client 或 Cargo 领域层。 |
| `apps/desktop/src-tauri` | 装配上述模块、Tauri commands/events、窗口、托盘、更新、权限及原生能力。 |
| `apps/desktop/src` | 共享 Workbench Client、Tauri/HTTP Application Transport、设备能力与客户端状态；共享 UI 不直接依赖 Tauri 细节。 |
| `crates/rambledesk-cli` | `serve` / `smoke` / `self-test` 诊断及共用 feedback 命令分派。 |
| `web/` | 独立官网，不是 Web Access 的业务客户端。 |

Cargo 依赖的精确清单由 [check-terminology.mjs](../scripts/check-terminology.mjs) 固化；前端组合根、模块依赖和预览注入约束见[开发指南](DEVELOPMENT.md#前端边界与测试)。跨模块编排留在组合根，不能让 core 反向依赖实现库。

## 业务事实与客户端投影

| 事实 | 所有者 |
| --- | --- |
| 不可变 Request 输入、Draft 的正文与交互状态、revision、附件元数据 | Backend Runtime / SQLite |
| 不可变反馈包 | 发布目录与 SQLite 结果记录共同对账 |
| Session、AgentConfig、Activity、Delivery、Recovery、Deletion intent | Backend Runtime；持久事实与实时连接投影分开 |
| 当前连接、执行状态、上下文 `used/size` | 当前运行实例；不把历史 connected 或累计 token 当作当前事实 |
| 打开的 view、顺序、active view、pane 尺寸 | 每个客户端的 workspace snapshot，不保存第二份 Draft 正文或答案 |
| phone/tablet/desktop mode、抽屉和 rail 布局 | 客户端 shell layout owner；布局不创造后端业务状态 |
| Editor selection、未保存编辑、当前设备录音与预览资源 | 当前 Workbench Client 的对应 owner |

事件只提示重新查询。通知、UI store、浏览器缓存和路径 hint 都不是额外业务事实源。响应式抽屉、焦点与安全区约束见[响应式工作台](WEB_ACCESS_SUPPORT_MATRIX.md)；物理手机能力与 CSS 视口适配分别验收。

## Application Transport 与恢复合同

Interface 暴露 typed command/query、变化订阅、ready barrier 与 capability manifest。Tauri IPC 和 Web HTTP + WebSocket 调用同一 application facade；manifest 只报告能力可用性，不执行设备操作。

Web Transport 保留以下顺序与失效合同：

1. 每次 Backend Runtime 启动生成唯一、进程内稳定的 `runtime_generation`。WebSocket ready 带 generation；客户端只接受当前 connection epoch 的 ready，替换 active generation 并使旧投影和请求失效。
2. HTTP snapshot/query 是带 generation/resource revision 的可丢弃投影；WebSocket 只传 readiness 与轻量 invalidation，不传 canonical Request/Draft/Package。
3. 修改状态的 command 等待当前 ready；envelope 的 `expected_runtime_generation` 映射到 `X-RambleDesk-Runtime-Generation`。服务端在领域副作用之前比较 command、session 与当前 generation，不匹配返回 HTTP `409` / typed `stale_generation`。Draft CAS 不能替代此检查。
4. 旧 socket、epoch、generation 的响应一律丢弃；同代 response revision 不能倒退。fetch 期间收到更高 revision 的 invalidation，完成后仍须再次 refetch。
5. 断线、撤销或连接失败结束对应等待，不能把保存永远挂在一个已失效的 ready Promise 上。恢复时重新认证/连接，确认 ready 后 refetch；不静默重放结果不明的 mutation。
6. 认证 gate 不卸载 dirty Editor。重新认证后保留本地编辑并按 revision/CAS 保存；冲突显示失败并保留原文，不自动合并、覆盖或丢弃。没有 sequence replay、ring buffer 或 multiplex 协议。

## Feedback Draft 与输入所有权

每个 Workbench Client instance 最多一个可编辑反馈正文 `RichFeedbackEditor`。正文和可见业务字段共用 `TiptapInput` 编辑基础；每个字段的 Editor 只是该字段的界面投影，不拥有独立草稿或录音会话。不建立 per-request/hidden Editor，也不让 session 持有字段 Editor handle。SQLite 中版本化 `document_json` 保存整个 Draft envelope，其中 TipTap `doc` 与可选 `workbenchState` 是不同字段。`body_markdown` 是同一次保存从 `doc` 生成的投影，不包含结构化答案。历史 Markdown 迁移、导出和展示不能成为第二套正文状态。见 [ADR 004](adr/004-single-editor-structured-draft.md)、修订其客户端作用域的 [ADR 005](adr/005-shared-workbench-transport-capabilities.md)与扩展草稿聚合边界的 [ADR 008](adr/008-typed-human-feedback-workbenches.md)。

当前正文通过 Editor transaction 修改；工作台视图通过类型合同修改 Interaction State，两者由同一草稿队列以同一 revision/CAS 保存。后台 Active Ramble 对固定 request owner 使用 JSON transformation，并保留 `workbenchState`。Draft controller 合并并发保存等待，等待中的调用共享成功/失败结果；成功后排空保存期间的新编辑，失败不自动重复提交。发布前等待包含最新正文与答案的 snapshot/revision，旧请求结果不能污染新请求。

多个客户端可以编辑同一 Draft，但只能由 Backend Runtime revision/CAS 仲裁。跨客户端 refetch、自动保存回执和导航不得静默清空未保存编辑；冲突保留正文和可见错误。接受当前保存的投影也不能无故重置编辑器 Undo 历史。

### 共享输入的客户端分层

共享输入按目标身份、草稿变换、设备会话与视图分层。录音器、截图窗口和保存队列由组合根注入，输入组件不创建第二份会话或草稿。

| 模块 | 职责 |
| --- | --- |
| `lib/domain/inputTarget.ts` | 与采集方式无关的 `InputTarget`、Action 身份与最小写入上下文；解码旧目标、固定目标快照并比较身份。 |
| `lib/workbenchFields.ts` | 从完整草稿调用本类型字段适配器，统一保留 envelope 的替换、追加与附件清除；业务身份、锚点和字段规则留在各类型。 |
| `lib/inputTextWriteback.ts` | 普通粘贴与附件引用的幂等回执，不产生语音来源信息。 |
| `lib/speech` | 转写队列、语音回执与来源范围、逐段目标固定、整理批次。`SpeechTarget` 是通用目标的语音侧别名，保留旧恢复记录形状。 |
| `lib/input` | 请求级下一输入位置，以及工具栏、字段附件标签和 `WorkbenchTextField`。复用语音入口，不管理录音设备。 |
| `lib/editor/TiptapInput.svelte` | 正文和业务字段共用的 TipTap 生命周期、可编辑状态与公共呈现；不负责请求路由、保存或设备采集。 |
| `lib/workbench` | 问答导航、批注生命周期等业务交互；组合输入、保存与提交准备。 |
| `lib/backgroundDraftWriter.ts` | 所有后台草稿变换共用一个 revision/CAS 重试循环；冲突时重新读取完整草稿再应用。 |

`WorkbenchTextField` 通过 `TiptapInput` 编辑字段的普通文本投影，并组合工具栏、附件引用、字数限制、焦点选择与行内语音来源标识。字段的选区与撤销历史限于当前目标，切换或隐藏字段不会让后台写入依赖 Editor。类型 definition 提供输入验证、草稿解码、完成条件和字段适配器，`definitions/registry.ts` 统一解析定义与能力；共享调用者不逐类型维护业务。通用 `workbench_field` 目标包含类型、版本、字段、业务对象与来源版本；旧问题和批注目标继续按原合同解释。后端独立验证完成条件。

| 允许的跨模块依赖 | 具体用途与限制 |
| --- | --- |
| `lib/workbench → lib/input` | 业务视图复用输入工具栏和字段组件；输入模块不反向调用工作台视图或控制器。 |
| `lib/input → lib/speech` | 复用录音入口、共享会话上下文和来源标识，不另建录音器或转写队列。 |
| `lib/input → lib/editor` | 复用 TipTap 输入基础及纯文本投影，不导入反馈正文控制器或建立第二份草稿。 |
| `lib/input → lib/domain` | 选择、固定和传递与设备无关的输入目标。 |
| `lib/input → lib/(root)` | 使用已有合同、附件 URL、字段附件展示辅助及 i18n/preferences，不调用后台保存控制器。 |
| `lib/(root) → lib/domain` | 纯草稿变换共享目标身份；根目录代码不依赖输入 UI。 |
| `lib/preview → lib/workbench` | 预览 transport 读取纯 definition 示例元数据，视图仍通过懒加载回调加载。 |
| `lib/speech → lib/workbench/definitions/registry` | 写回与整理检查当前类型是否可编辑；只读取无 UI 的类型注册与规则，不导入业务视图，也不建立反向语音依赖。 |

这些方向及逐条理由由 `architecture/frontendBoundaries.test.ts` 固定。`lib/speech` 不反向依赖输入工具栏；普通输入的领域校验不依赖语音队列、来源信息或整理状态。

## 第一方工作台组合

工作台专用区域与通用反馈列是同一个请求的两个输入区域。八种正式类型各自提供独立视图，当前目录见[协议](PROTOCOL.md#工作台发现与类型合同)；单项方案选择使用一道关闭自定义回答的问答题，旧 `single_choice` 通过薄兼容层复用同一问答视图并保留旧保存/结果合同。共享容器负责布局、加载、锁定、反馈列和提交协调；类型视图负责自身业务对象与交互，不直接调用 Application Transport、设备采集或发布入口。

视觉反馈的请求级 controller 读取持久背景，并在统一提交准备阶段生成 PNG。宿主提供生成附件持久化能力：与用户附件共用 CAS 队列，先保存草稿、上传、回灌新 revision，再把合成附件 ID 写入类型状态；最后由统一 publisher 保存并发布。生成附件不自动插入正文，不能单独被认作人类反馈。差异评审只使用请求内保存的 diff 快照及结构化批注，不访问当前磁盘 diff 或引入新的评审流程控制器。

Backend Runtime 的类型目录提供发现、schema、输入验证和结果解释；客户端静态注册项选择相应视图与状态规则。它们共享稳定的 `type/version` 合同，分属服务端领域规则与客户端呈现职责。注册表随应用构建，不是动态插件框架；目录 `interaction` 字符串只描述交互分类，不参与宿主协议或权限。

Rust 的 `workbenches/registry.rs` 是唯一类型声明入口，由静态宏生成模块组合、Kind/Data/State/Result、解码和 DTO 导出清单。每个业务模块拥有发现元数据、schema/example、输入与草稿校验、结果投影、完成条件及可选运行校验。存储只负责 envelope 编解码与 CAS，再调用领域定义；新增普通类型不修改数据库或传输分发。

前端的 `lib/workbench/definitions/registry.ts` 是唯一注册入口；各 definition 拥有输入识别、状态解码、完成条件、布局、字段适配器、可选控制器和示例。`RegisteredWorkbench.svelte` 按 definition 懒加载视图并注入 `WorkbenchViewContext`：公共 `WorkspaceHeader` 与 `RequestContextPanel` 固定在业务滚动区域上方，后者将 `request.what_happened` 呈现为「情况说明」，并组合请求材料标签。说明与附件共用限高滚动区域，无附件时不占位。业务视图通过 host 更新状态与引用正文，不持有保存队列或 publication state。全屏按 `layout.expanded` 主动开启，普通模板与排序默认关闭；开启后宿主提供 `openExpanded`，同一请求的普通/全屏页签共用草稿与运行状态，左侧会话导航保持可用。

`workbenchLifecycle.ts` 按请求持有可选 controller，统一工作台、任务页签和原生控制台的提交准备；视图只 attach/detach。终端控制器处理已接受按键排空、实际停止、最终输出和多轮记录合并，卸载视图不停止后端 PTY。发布前，后端类型的 `pre_publish` 校验真实请求所属资源状态，终端创建与发布共享生命周期锁，避免检查后又开启会话；客户端的 stopped 草稿不是运行完成的证明。可保存草稿和可发布结果有各自规则。

普通新增体验由 `pnpm workbench:new` 生成三份业务代码与用例，仅修改两份注册文件；示例和 playground 按统一约定发现。详见[新增教程](workbench/adding-a-workbench.md)及[验收记录](workbench/framework-validation.md)。测试类型 `rating_review` 只在 Rust 的 `workbench-fixtures` 与前端开发开关下注册，默认生产类型目录不包含它。

持久草稿的 JSON envelope 由 `crates/rambledesk-storage/src/workbench_result.rs` 解码；repository 在同一事务快照中取出正文与交互状态，再调用 `core/workbenches/draft.rs` 的纯提交策略。core 接收已解析的类型化交互状态与正文投影，不读取 `document_json`、解释 TipTap JSON 或拥有存储 codec；Backend Runtime 对整个 Draft 的业务所有权不因编解码所在模块而改变。

`WorkbenchSpec` 与 `WorkbenchPackage` 先根据外层 `type/version` 决定是否解释数据。未知合同的输入和已发布结果使用格式无关的 `serde-value` 保留，不能因形状类似当前类型而丢掉嵌套字段或补入当前默认值。JSON 解析和 wire 回归位于 storage/transport 边界；保留未知值只服务历史读取，不放宽创建、编辑或提交验证。

创建未知类型或不支持的版本时由服务端拒绝。已有请求在客户端无法安全解释时进入只读：保留原请求、Draft 与已发布结果，禁止覆盖保存、提交和批准，不把未知数据降成空状态。该兼容状态与终态、提交中锁定分别表达；取消仍是显式请求操作。省略 `workbench` 的旧请求继续使用完整可编辑的 Ramble 路径。

提交冻结整份草稿，再由服务端根据不可变输入校验完整性、解析稳定 id 与文稿锚点，形成 Workbench Result。自由正文可以为空，但不能代替类型必填判断。Cooking 只处理正文，结构化状态不交给模型改写。文稿审阅的原稿属于请求输入；批注、段落标记与改写建议属于 Draft，提交后由 Agent 据此修改原稿。具体合同见[文稿审阅](workbench/document-review.md)。

Action 仍表示 Ramble 体验动作。旧路径将部分问答或方案数据投影成 Action 的做法是兼容桥接，不是新类型必备的领域实体；文稿段落、批注与文本锚点各自保持业务含义。

设备输入位于 Application Transport 之外。Platform Plugin 组合本地能力、权限与资源，只返回 `SpeechEvent` 或 `AttachmentCandidate`；实时 PCM、识别 session、模型与权限不进入后端 Transport。浏览器选文件是读取客户端文件，不是选择服务器 `cwd`；不可用能力明确禁用。Browser ASR 当前是本地 pilot，浏览器屏幕采集尚未交付，具体设备支持见[矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)。

Action 用带 `actionId` / `actionIndex` 的标准 Blockquote；语音段有稳定 `speechSegmentId` 和 `pending` / `cleaned` 状态。请求级 Tidy 与数量阈值 Auto Tidy 共同统计正文、答案和批注中的未整理语音，入口位于右下角 Rambelle 状态区；待确认语音也可启用逐段自动整理，默认关闭并仍等待人类确认。两条路径都保留编辑/整理/写入占用、owner 与原文校验，已整理语音写入时传递 `cleaned` metadata。普通键入、粘贴文字和附件引用不进入语音整理批次。Tidy 与 Cooking 独立配置，规则见[术语表](TERMINOLOGY.md#tidy-与-cooking)。快捷键只发送语义事件，不持有 Editor。

附件候选先由当前目标 owner 验证和持久化，成功后才成为 Draft 引用。过期目标、发布后的写入及丢失响应不能借本地乐观状态伪装成功；预览资源由实际使用者持有，在最后一个使用者结束后释放，不能以面板切换代替整个生命周期。

## 会话与客户端生命周期

反馈输入由客户端的请求级会话统一组合：正文、问答自定义回答、批注意见和建议措辞通过各自的输入工具栏和明确目标共享录音器、字幕、悬浮工具台与待确认队列。每段语音和每次采集固定开始时的目标；附件仍归请求所有，并在发起的正文或字段旁引用。右下角 Rambelle 只呈现左侧头像、右侧状态气泡及有待整理语音时出现的全局整理按钮；位置切换和录音控制由输入框工具栏承担，字幕保留在桌面悬浮窗中。领域写回与组件解耦，提交前统一排空语音、剪贴板和附件，再复核输入状态并冻结草稿。详见[请求级共享输入](workbench/shared-input.md)。

托管路径中一个 RambleDesk Session 对应一个 Agent Session，每个会话独占一个 ACP Instance；实例可包含 bridge 和子进程。配置、权限、输入、反馈、停止、恢复和删除都使用共享 application 合同。协议握手不证明模型或 Agent 执行工具可用，支持版本见 [ACP 指南](ACP_MANAGED_SESSIONS.md)。

新会话先创建内部持久 `prepared` 记录，准备不发 prompt；首条真实用户消息被接受并落盘时转为 `active`。导航不展示 prepared；准备失败可用原 ID 重试，关闭/变更/重启回收准备资源，不能通过草稿 discard 删除 active 会话。前端草稿 Tab 原位转为正式 Tab，只恢复用户输入，不恢复准备凭据或身份。

项目目录浏览走共享 `browseProjectDirectories` application command：core 定义只读合同与 provider，local-server 提供按宿主 OS 权限运行的目录实现，由组合根同时注入 Tauri 与已认证的 HTTP application facade，不向 Agent MCP 暴露。每次仅列直接子目录，返回规范绝对路径、父目录、主目录和根位置。弹窗的浏览状态独立于草稿 cwd，确认才调用草稿选择操作；关闭或后续导航使旧异步结果失效。新会话引导卡片只通过现有 controller 编辑正文，不建立第二份输入状态，也不调用发送命令。

正式会话的 Ramble / Agent 是平级视图；Ramble 预览按可信 `managed_session_id` 查询状态，不为状态卡加载全部 Agent 历史。Agent 时间线按 turn 展示，折叠过程延迟挂载；上下文占用只来自当前实例真实 usage 更新。

关闭正式 Tab、刷新/关闭浏览器或 Transport 断线不隐式 submit、cancel、archive Request，也不停止 Backend Runtime 中的 Agent Session。浏览器关闭会结束该客户端的录音、Editor 和设备资源；它不能保证本地 Active Ramble 跨页面存活。未保存或保存中的 Draft 使用浏览器标准 `beforeunload` 确认，人类选择离开后按浏览器行为结束；不在 unload 尝试保证异步保存。桌面关闭窗口/托盘合同与浏览器离页处理分开。

重启不信任旧 connected 状态；未完成轮次由检查点记为中断。存在 `remote_session_id` 时恢复必须 resume/load 原会话，失败不静默新建。删除先持久化 intent，再停止并清理所属资源，失败可重试；文件清理与发布共用锁，旧 publication plan 不能复活已删除数据。

## 发布与托管 continuation

```text
Submit(request_id, expected_revision)
  → 检查非终态与 revision
  → 写入临时包、flush 与 hash
  → 原子发布
  → 持久化终态与包元数据；托管提交同时入 outbox
  → 通知 waiters / 对应 continuation
```

包已发布但数据库更新失败时，启动恢复按 manifest/request_id 对账，不生成第二份包。提交、批准与取消都必须幂等；批准最终总结可以完成请求而不发布包。外部协议的状态与包合同见 [PROTOCOL](PROTOCOL.md)。

生产 ACP 使用应用内置 `feedback request/get/recover/skip`。每次运行绑定会话专用 credential，停止/删除时撤销；它不能访问 Generic MCP 或其他会话。命令子进程只持有私有 IPC 能力，HTTP credential 留在运行时 relay 内存。入口注入可信会话归属，不注入 MCP server 或 Pi 扩展，不修改用户全局 Skills。

每条用户/续接 prompt 前置工作流上下文，但用户 Activity 不保存注入说明。Agent 工具和 bridge 环境传播仍须实测。托管提交/批准与 outbox 入队原子提交，worker 仅在原会话可接收输入时续接；取消不创建新投递。旧取消投递保留历史并停止派发。delivered 不表示任务完成，uncertain 不自动重试，只由人类显式处理。

outbox worker 在投递前通过原 `start_session` 生命周期初始化或恢复离线会话，保留 `remote_session_id`，不依赖前端视图挂载；只读状态查询不启动会话。初始化任务按会话独立执行，并受生命周期锁、中断 epoch、删除意图和 shutdown 管理。忙碌或待答交互继续等待，启动失败只记入对应会话并保留 pending，不轮询反复启动。同一 runtime 的 `stopManagedSession` 记录已入队请求的边界，旧请求不能自动重启；新提交或显式连接可恢复投递。该内存边界不改变应用重启时 pending / sending / uncertain 的持久恢复合同。

Agent 会话标题栏的「内置会话指令」只读展示 `SessionRuntime.builtin_instructions`。该字段在连接成功时从 driver 取得，与 ACP 实际发送的工作流上下文共用同一源文本，不由前端重新拼装。停止连接后保留最近连接的文本并标明其范围；新应用实例和不提供该能力的连接可以省略该字段，不据此推断历史回合使用过哪些规则。查看或复制指令不产生用户 Activity，也不发送 prompt。工作台选型由 Agent 依据本次需要的人类输入决定，具体用途与接入条件见[工作台目录](PROTOCOL.md#工作台发现与类型合同)；混合任务先处理阻塞下一步的输入。

## 数据布局

默认业务数据与可迁移资料库分开：

```text
<local-data>/RambleDesk/
├── state/feedback.sqlite3
├── settings.json
└── library/                    # 可由资料库设置或 RAMBLEDESK_LIBRARY_DIR 改变
    ├── drafts/<request-id>/
    └── feedback/<timestamp>-<request-id>/
        ├── feedback.md
        ├── uncooked.md
        ├── manifest.json
        └── attachments/
```

其他模型/缓存由各能力模块管理。数据库、资料库与 Local Integration token 的开发覆盖变量见[开发指南](DEVELOPMENT.md#本地运行)。Web credential 使用独立的 Tauri `app_local_data_dir()`，按应用 identifier 隔离，不属于可迁移 library、通用配置或业务 SQLite。

## 安全边界

### Local Integration Server

反馈 API 只绑定 loopback，使用持久 256-bit hex bearer、constant-time 比较和 Host/Origin guard。Host 只允许 `127.0.0.1` / `localhost`；无 Origin 只按非浏览器本机调用放行，有 Origin 必须 exact-match allowlist。`/web-review/bridge.js` 只公开固定静态代码并校验 listener Host，不携带凭证或反馈数据，不放宽反馈 API 认证。通用 body limit 为 96 MiB。Unix token 文件 `0600`，非 Unix 不宣称等价 ACL；不能把它直接复用为 Web credential。关联字段和 context hint 均不承担认证。

### Web Access

- 独立 listener 使用 same-origin 静态资源、HTTP 与 WebSocket，不开放宽泛 CORS。两类 listener 共用安全 policy/primitives，但不共享 credential 或生命周期。静态请求严格检查 Host，bootstrap、受保护 API 和 WebSocket 还检查 exact Origin。
- 独立 256-bit durable token 由 Desktop 的 Web security module 持有；不返回通用 UI，仅设置页的原生 clipboard command 可复制。macOS/Linux 使用 `app_local_data_dir()/auth/web-access.token`；Windows 保留 Credential Manager。
- Unix `auth` 目录 `0700`、文件 `0600`，原子创建/轮换并拒绝 symlink、非普通文件、错误归属和损坏内容。自动加载失败不生成替代 token；显式 Refresh token / Confirm 可修复损坏的自有普通文件，不绕过路径与归属检查。
- 旧 macOS Keychain / Linux Secret Service 条目不读取、不自动迁移、不删除。升级首次启用创建新 token，需重新复制认证；后续同一运行时有效 cookie 仍可恢复。文件权限不等于加密，也不宣称排除 OS 备份。
- durable token 经 same-origin `POST /api/auth/session` bootstrap 换取受限 session。响应写入 `HttpOnly; SameSite=Strict; Path=/api/auth/session` cookie，刷新/新标签页/浏览器重启可凭有效 cookie 恢复；HTTP 使用 session bearer。
- 内存 session 的 idle/absolute TTL 为 30 分钟/12 小时；浏览器 session 为 30 天/180 天。HTTP 请求或新 WebSocket 认证刷新 idle，不延长 absolute。停止服务、Runtime 重启或轮换 token 撤销 session；stop/rotate 关闭已有 socket。
- session token 仅在服务端、当前 JavaScript 内存及上述 cookie；token 不进入 URL、sessionStorage、localStorage、IndexedDB、SQLite、日志、诊断或应用备份/导出/反馈包。bootstrap 成功清空 durable token 输入。
- WebSocket 同时提供 `rambledesk-events` 与 `rambledesk-session.<base64url-no-pad-session-token>` subprotocol；只回显前者，不用 query token，不记录 credential-bearing protocol。
- HTML `no-store`，仅 build manifest 标记的 fingerprinted asset 使用 immutable cache。SPA fallback 不覆盖 `/api/**`、`/assets/**`、扩展名路径、traversal 或 encoded separator。
- 每 listener 每分钟最多 8 次 bootstrap、同时 16 个 application HTTP request / 8 个 socket、最多 32 个 session。超限返回 `429` 或 `503`；multipart 使用 20 MiB 附件上限加 64 KiB metadata allowance，其余 JSON 同样有界。
- 服务端保存 session credential 的固定长度 SHA-256 hash，完整 constant-time 扫描。认证是 request admission lease：已入场 mutation 在随后 stop/expiry 时仍返回真实结果，不自动重放；撤销限制后续请求和 socket。

LAN、TLS、独立 Web deployment/headless Backend Runtime 和浏览器屏幕采集均不在当前支持面。未来扩展须重新定义并验证设备暴露、认证与文件边界，不能从现有浏览器 API 或 CLI 推导支持。存储选择、自动化和真实平台操作的证据分别见 [Web Access 支持矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)、[ADR 005](adr/005-shared-workbench-transport-capabilities.md)及[质量清单](quality/README.md)。
