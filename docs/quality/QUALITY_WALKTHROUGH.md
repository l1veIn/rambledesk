# 全局职责与质量阅读地图

> CURRENT：2026-09-10，阅读对象为 `v0.4.0-rc.3` / `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce`
> 之上的本轮实现，代码已提交为 `27b50f1` 与 `5d57526`。本文件解释实际代码如何分工；阶段状态、commit 与结束条件由
> [全局质量计划](../PROJECT_QUALITY_PLAN.md) 的实施账本负责，不在这里另建一份账本。

RambleDesk 的职责是把人的结构化反馈、原始输入与交付结果可靠地接到外部或托管 Agent 的工作流。
桌面和 Web 共享业务合同，设备能力由各平台提供；RambleDesk 不成为另一个 Agent runtime。
完整产品边界与术语仍以 [架构](../ARCHITECTURE.md)、[术语](../TERMINOLOGY.md) 为准。

## 按问题找 owner

| 要理解或修改的事情 | 先读哪里 | 这里负责到哪里 |
| --- | --- | --- |
| Client 如何装配、启动和退出 | [App](../../apps/desktop/src/App.svelte)、[Startup](../../apps/desktop/src/lib/workbench/startupController.ts) | 组合现有 owner、订阅和 capability；Startup 发布恢复事实，dispose 后迟到 ready 不再启动轮询 |
| 当前 request、提交阶段和 UI 锁 | [Workspace Session](../../apps/desktop/src/lib/workbench/workspaceSession.ts)、[Draft Session](../../apps/desktop/src/lib/workbench/draftSession.ts) | 持有事实并提供订阅投影；dirty、terminal、feedbackResult、interactionLocked 不独立复制写入 |
| 编辑保存和版本冲突 | [Draft Controller](../../apps/desktop/src/lib/workbench/draftController.ts)、[单 Editor 决策](../adr/004-single-editor-structured-draft.md) | 等待完整保存过程，保留新编辑与失败；结构化 document 是草稿真源，后端 revision/CAS 仲裁 |
| Markdown 转换和编辑器扩展 | [Feedback Editor Extensions](../../apps/desktop/src/lib/feedbackEditorExtensions.ts)、[Rich Editor](../../apps/desktop/src/lib/editor/RichFeedbackEditor.svelte) | 复用同一 schema；每个 Editor 和短期转换拥有自己的 parser；事务只向 UI 投影工具栏状态 |
| 打开、切换、关闭与后台刷新 | [Workspace Navigation](../../apps/desktop/src/lib/workbench/workspaceNavigationController.ts)、[Transition](../../apps/desktop/src/lib/workspace/workspaceTransition.ts)、[Navigation](../../apps/desktop/src/lib/workbench/navigationController.ts) | 目标意图、scope 候选、保存/加载/接受和失败恢复；后台刷新让位于后续用户导航 |
| 录音事实和提交前输入准备 | [Ramble Session](../../apps/desktop/src/lib/workbench/rambleSession.ts)、[Ramble Controller](../../apps/desktop/src/lib/workbench/RambleSessionController.svelte)、[Voice](../../apps/desktop/src/lib/workbench/voiceRambleSession.ts) | 麦克风与 request 归属、停止和排空、ready/pending-speech/failed；组件语言变化不销毁输入 owner |
| 待审语音与文档写入 | [Speech Draft Queue](../../apps/desktop/src/lib/speech/speechDraftQueue.ts)、[Draft Operations](../../apps/desktop/src/lib/workbench/draftOperationsController.ts) | 前者持有未确认 transcript、整理/编辑和本地恢复；后者串行执行已接受的文档操作，固定 request/Action 并选择前台或后台写入 |
| 附件取得、写入和释放 | [Attachment Controller](../../apps/desktop/src/lib/workbench/attachmentController.ts)、[Attachment Previews](../../apps/desktop/src/lib/workbench/attachmentPreviews.ts)、[Candidate 合同](../../apps/desktop/src/lib/capabilities/capturePlugin.ts) | 候选到持久化、文档引用、保存与收据；预览 URL 有自己的读取归属和释放周期 |
| Cooking、提交、批准和取消 | [Cooking](../../apps/desktop/src/lib/workbench/cookingController.ts)、[Publisher](../../apps/desktop/src/lib/workbench/publisherController.ts)、[Submission](../../apps/desktop/src/lib/workbench/submissionController.ts) | Cooking 返回独立变体；调用者持有终态动作、等待保存、应用服务端结果和后续读取错误 |
| Agent 首发、正式会话与关页 | [Prepared Draft](../../apps/desktop/src/lib/agents/draftManagedSessionController.ts)、[Managed Session](../../apps/desktop/src/lib/agents/managedSessionController.ts)、[Workbench Actions](../../apps/desktop/src/lib/workbench/managedSessionActions.ts) | prepared 资源、接纳不明、输入保留和一次晋升；Client 关页与后端会话停止/删除保持不同动作 |
| 业务命令和设备差异 | [Application Transport](../../apps/desktop/src/lib/application/applicationTransport.ts)、[Capabilities](../../apps/desktop/src/lib/capabilities/workbenchCapabilities.ts)、[Desktop 装配](../../apps/desktop/src-tauri/src/lib.rs) | Transport 承载共享应用合同；Capability 承载设备、窗口和平台操作，不复制后端业务实现 |
| 后端事实、发布与恢复 | [Feedback Application](../../crates/rambledesk-core/src/feedback.rs)、[Session Application](../../crates/rambledesk-core/src/sessions/application.rs)、[SQLite](../../crates/rambledesk-storage/src/sqlite)、[Recovery](../../crates/rambledesk-core/src/sessions/recovery_runtime.rs)、[Delivery](../../crates/rambledesk-core/src/sessions/delivery.rs) | 结构化草稿、终态、包与接纳的持久事实；会话恢复、续接投递和删除由后端仲裁 |
| 设置与响应式界面 | [Settings](../../apps/desktop/src/lib/settings/SettingsPanel.svelte)、[Section 能力规则](../../apps/desktop/src/lib/workspace/settingsCapabilitySections.ts)、[Shell](../../apps/desktop/src/lib/workbench/WorkbenchShell.svelte)、[Tabs](../../apps/desktop/src/lib/workspace/WorkspaceTabStrip.svelte) | 设置定位/标题复用同一元数据；Shell 管呈现与焦点，导航 owner 管接受目标，不让布局组件保存业务草稿 |

## 五处值得保留的设计

### 1. 状态入口让正确读取成为常规用法

`WorkspaceSession` 内部保留事实，对外发布 `project(facts)`。`submitting` 从 submit stage 得出，UI 通过
`$workspaceSession.interactionLocked` 读取锁。`DraftSession.dirty` 同样由当前文档与已接受文档比较得出。
调用方不必同步修改互相依赖的布尔量，也不必记住普通 getter 隐藏了 Svelte 的订阅依赖。

`RambleSession.connectVoice` 复用麦克风原始 store，`$rambleSession` 给标题栏、操作面板和输入 controller
提供一致投影。语言变化只重建需要更新的 UI，Ramble owner 保持挂载；这同时减少接线和设备生命周期错误。
真实 App 测试验证语言切换后没有 cancel microphone，后续 partial 仍能显示。

`RichFeedbackEditor` 也由 Tiptap 事务更新 Undo/Redo 和格式按钮的布尔投影；文档与 selection 仍由 Tiptap
持有。直接替换 EditorState 时显式更新同一投影，因此输入、光标变化和历史重置都能通知工具栏。

阅读重点是“哪些值由谁写”，而不是 store、class 或 runes 哪个语法更新。

### 2. 完整意图承担等待顺序

`workspaceNavigation.activateView` 收进 scope 查询、目标选择和 transition 接受。候选 scope 在保存和加载
完成前不成为可见事实，因此失败时无需在多个入口异步切回旧 rail；迟到 refetch 也不能覆盖新的用户意图。
App 只请求打开或关闭，单 Editor 的保存/卸载/加载顺序继续由现有 transition 保证。

保存广播回到当前 Client 时，导航 owner 先核对 request、revision 和结构化文档。相同文档只协调服务端
事实，保留当前 Editor、selection 与 Undo 历史；读取期间新输入仍留在本地。确实改变的远端文档才走
原有加载流程。[真实 App 事件流测试](../../apps/desktop/src/App.editorRefresh.test.ts) 覆盖保存后 Undo/Redo，
补上无事件预览 transport 无法证明的窗口，详见 [导航验收](NAVIGATION_ACCEPTANCE.md)。

`prepareFeedback` 则承诺结束录音并完成已接纳的输入写入。独立 exit flight 先等正在启动的麦克风，再停止它，
同时排空期间新接受的 clipboard import。返回值直接区分 ready、待审语音和失败。Publisher、Cooking 预览、
批准/取消不再各自询问若干 getter 后拼接等待顺序。

两处接口减少了调用者需要记住的时序，同时保留能证明失败恢复的具体测试。

### 3. 已有深模块继续承担它擅长的职责

本轮保留 prepared controller 的主体。它已经处理临时资源、未知首发接纳、后续文字和晋升；重建一个通用
任务引擎没有已证实的收益。新增真实 App 组合测试让未知首发、关闭、显式重查、晋升和正式关页串成同一条旅程。

输入也保留两个队列：Speech Draft Queue 管“人尚未确认的话”，Document Queue 管“已接受的文档操作”。
二者失败恢复和生命周期不同，通过 preparation 连接即可；没有合并成一个含义模糊的总队列。
同样保留 Application Transport 与平台 Capability 的边界，以及结构化 Draft 和单 Editor 决策。

可观察的改进是：进入一个模块后能完成自己的问题，跨模块只需要少量有业务含义的动作。

### 4. 成功结果与后续界面读取分开

Cooking 的输出带 requestId、savedRevision 和 original，是独立变体；原始结构化 Draft 不被整理结果覆盖。
Publisher 检查来源后才提交，后端返回成功时立即接受终态。

之后的包读取或导航刷新失败，只产生读取错误。它不能把已经发布的反馈重新标为可提交，也不能从客户端字符串
虚构一个成功包。批准/取消遵守同样顺序。真实模型、HTTP/SQLite 和包哈希验证进一步检查了原稿/变体/来源的一致性。
详读 [反馈链示范](../FEEDBACK_FLOW_WALKTHROUGH.md)。

### 5. 局部所有权消除全局注册增长

长历史的性能复测暴露了一个生命周期问题：每次 Markdown 转换和 Editor 创建都会向 marked 的全局
单例注册 tokenizer，闭包保留旧 manager；销毁 Editor 并不能撤销这些全局注册。一次请求的 `breaks: true`
也会改变后续默认转换。不断增加的解析工作与跨请求选项串扰来自同一个共享可变实例。

[feedbackEditorExtensions](../../apps/desktop/src/lib/feedbackEditorExtensions.ts) 现在向每个 Markdown
扩展实例和短期转换 manager 注入私有 `Marked`。表格、任务清单、附件和 Action schema 继续使用同一组
扩展；只有注册的归属改变。`marked@17.0.6` 与现有 Tiptap 间接依赖版本一致，局部类型适配也说明了
Tiptap 声明中的 callable singleton 与实际所用实例方法之间的差异。

[生命周期测试](../../apps/desktop/src/lib/feedbackEditorExtensions.lifecycle.test.ts) 使用真实依赖：反复
转换及创建/销毁 Editor 不增加全局 tokenizer，邻近存活 Editor 的注册不增长，`breaks` 不影响下一次
转换。有限的私有实例会随自己的 owner 失去引用，不需要新增全局缓存或依赖外部清理顺序。

同一 `managed-history-v3-output-control`、`final-json` 输出模式、1200 × 820 Chrome 场景的 30 轮结果中，
进入历史的 P95 从 567.6 ms 降至 117.9 ms，HTTP 分页后可见的 P95 从 546.5 ms 降至 83.2 ms。
原始结果分别见 [修复前](evidence/managed-history-final-output.json) 和
[修复后](evidence/managed-history-parser-fixed.json)。这是该场景的实测延迟改善；长期 heap 和资源释放
仍按 [性能验收](PERFORMANCE_ACCEPTANCE.md) 的独立观察判定。

### 6. 等待也需要明确的所有者

[HTTP Session](../../apps/desktop/src/lib/application/httpApplicationSession.ts) 中的 ready 等待表达的是
“这一轮连接恢复何时结束”。构造或断线时创建它；恢复过程中的所有调用等待同一个结果。
成功、失败、认证撤销都必须结束等待。

真实 Chrome / Safari 操作发现，旧实现每次连接尝试又创建一个等待，把断线期间排队的保存遗留在旧对象上。
修复删除了这一行重复创建，没有增加重试管理器，也没有让 transport 擅自重放写操作。
[组合回归](../../apps/desktop/src/lib/application/httpApplicationSessionReconnect.test.ts) 验证等待确实结束，
原来的 DraftController 能在重新认证后把自己的原稿保存到 r3。

这与最初反馈链的优雅之处相同：每一个“尚未完成”都有一个能结束它的所有者。
对象数量更少只是结果；真正的价值是，读者能说清谁在等什么，以及由谁兑现或拒绝这次等待。

## 证据和当前边界

| 结论范围 | 当前记录 |
| --- | --- |
| 输入、保存、终态与异步归属 | [输入](INPUT_ACCEPTANCE.md)、[终态](TERMINAL_ACCEPTANCE.md)；有先红后绿的真实组件/owner 组合，设备边界受控 |
| 导航与 prepared 前端组合 | [导航](NAVIGATION_ACCEPTANCE.md)；保存失败、迟到结果、关闭与首发晋升合同通过 |
| HTTP、SQLite、包与后端生命周期 | [反馈夹具](FEEDBACK_ACCEPTANCE.md)、[后端](BACKEND_ACCEPTANCE.md)；各条结果按实际测试范围解释 |
| 真实 Cooking | [本轮模型证据](evidence/cooking-live.json) 为 passed；真实模型 + Controller/Publisher + HTTP/SQLite，原稿、变体与发布来源一致；不代替浏览器/原生交互验收 |
| 真实 Agent | [Agent 验收](AGENT_ACCEPTANCE.md) 记录完整闭环未通过；模型参与、权限/IPC/初始化失败与清理结果分别保留 |
| 响应式、键盘与焦点 | [响应式](RESPONSIVE_ACCEPTANCE.md) 记录组件合同和浏览器视口实测；Windows、真实手机及用户确认继续按总计划登记 |
| 性能与资源 | [性能](PERFORMANCE_ACCEPTANCE.md)；基线、测量入口和已经运行的样本分开，没有数据支持的优化不实施 |

这是一份现有职责的阅读地图和局部设计示范，不是完成声明。全计划仍受平台必验项、真实 Agent 闭环、
手机 dogfooding 与用户交互确认约束；任何测试计数或文档数量都不能替代这些结果。
