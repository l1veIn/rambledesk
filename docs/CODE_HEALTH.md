# RambleDesk 模块健康盘点与拆分策略

> 状态：2026-09-08 盘点，基线 commit `d631ea3`（v0.4.0-rc.2）。
> 口径：`wc -l` 全文件行数，含测试、数据与注册表文件；阈值 >700 行。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文只描述代码结构与拆分策略，不改变任何协议。

## 结论

- **20 个文件超过 700 行，其中 5 个超过 1000 行；12 个是前端文件、8 个是 Rust 文件。** 前端占比高的
  原因是 Rust 侧已有 800 行硬门禁（`scripts/check-rust-module-size.mjs`，在 CI 三个 job 与 release
  validate 中执行），前端完全没有门禁。
- **行数超线不等于上帝文件。** 真正需要拆的是 7 个逻辑密度高的文件（A 类）；`i18n.ts` 是字典数据
  （B 类）；`feedback.rs`、`sqlite.rs`、`diagnostics.rs` 已经是模块化 facade（C 类）；其余是测试套件（D 类）。
- **最高优先级不是最大的文件，而是 `App.svelte` 的 shell 状态。** 它同时是移动端响应式适配的唯一落点，
  而纯布局数学（`components/navigation/railResize.ts`，62 行）已经有单元测试，抽取风险最低。
- **建议顺序：先抽 Workbench Shell（第一刀）→ 再做响应式适配 → 再按本表逐项拆分。**
  不要在响应式之前做全量重构，也不要在 2498 行的 `App.svelte` 里直接做响应式。

## 一、全景

| 文件 | 行数 | 类别 | 主要症状 | 优先级 |
| --- | --- | --- | --- | --- |
| `apps/desktop/src/App.svelte` | 2498 | A | 63 个顶层函数 + 90 个状态变量；script 2120 行、模板 356 行 | **P0** |
| `apps/desktop/src/lib/SettingsPanel.svelte` | 2120 | A | 8 个设置域、30 个 handler；模板约 1270 行 | P1 |
| `lib/application/httpApplicationTransport.ts` | 1116 | A | 会话类约 510 行 + transport 类 + 操作表混在一个文件 | P1 |
| `apps/desktop/src/ScreenshotOverlay.svelte` | 1045 | A | 40+ 个指针/选区/标注/工具栏 handler | P2 |
| `lib/i18n.ts` | 931 | B | 920 行单一字典常量 | P3 |
| `lib/application/httpApplicationSession.test.ts` | 1160 | D | 单个 describe 覆盖全部重连状态机 | P2 |
| `lib/workbench/navigationController.test.ts` | 896 | D | 单个 describe 覆盖 40+ 个控制器方法 | P2 |
| `lib/workbench/attachmentController.test.ts` | 831 | D | 单个 describe 覆盖截图/附件全流程 | P3 |
| `crates/rambledesk-core/src/feedback.rs` | 799 | C | 已有 7 个子模块，余下是类型 + 应用服务 impl + 错误码 | P2 |
| `crates/rambledesk-storage/src/sqlite/tests/workspace.rs` | 791 | D | 工作区场景测试 | P3 |
| `crates/rambledesk-local-server/tests/managed_application.rs` | 764 | D | 托管会话 API 测试 | P3 |
| `crates/rambledesk-storage/src/sqlite/tests/requests.rs` | 738 | D | 请求场景测试 | P3 |
| `crates/rambledesk-storage/src/sqlite/tests/deliveries.rs` | 727 | D | 投递场景测试 | P3 |
| `crates/rambledesk-local-server/tests/application_api.rs` | 725 | D | application HTTP 测试 | P3 |
| `lib/workspace/ArchivedSessionsWorkspaceView.svelte` | 720 | A | 搜索/高亮 + 加载 + 会话与请求操作混在一个视图 | P2 |
| `lib/agents/draftManagedSessionController.test.ts` | 717 | D | 草稿生命周期测试 | P3 |
| `lib/workbench/RambleSessionController.svelte` | 705 | A | script 681 行、模板仅 19 行：控制器塞进了组件 | P2 |
| `apps/desktop/src-tauri/src/diagnostics.rs` | 703 | C | 已有 4 个子模块，余下是导出编排 + 打包 | P3 |
| `lib/workbench/navigationController.ts` | 702 | A | 单个工厂内部 28 个函数、对外暴露 15 个方法 | P1 |
| `crates/rambledesk-storage/src/sqlite.rs` | 701 | C | 已拆 17 个子模块，余下是 facade + row mapping | P3 |

## 二、A 类：逻辑上帝文件

### 1. `App.svelte`（2498 行）— P0

**症状**：组件同时是组合根、路由、会话恢复、Agent 生命周期、提交/取消、引导和布局状态的所有者。
80 个 import 语句横跨 12 个目录，63 个顶层函数按职责可分为六组：

| 组 | 代表函数 | 目标位置 |
| --- | --- | --- |
| 布局与 shell 状态 | `navigationWidths`、`hostRailWidth`、`requestRailWidth`、resizing 标志 | `lib/workbench/WorkbenchShell.svelte` + `shellLayout.ts`（第一刀） |
| 工作区 Tab 与视图路由 | `activateWorkspaceTab`、`closeWorkspaceTab`、`openSessionView`、`loadWorkspaceTarget` | 已有 `workspaceShell.ts` / `agentViewRouting.ts` 可继续加深 |
| 启动与会话恢复 | `startWorkbench`、`restoreInitialWorkspaceSnapshot`、`refreshSessionViewRecovery` | `lib/workbench/startupRecovery.ts` |
| 托管会话与 Agent | `openAgentSession`、`openNewManagedSession`、`archiveSessionFromUi`、`deleteManagedSessionFromUi` | 已有 `agents/` 控制器，补齐 UI 动作编排 |
| 反馈提交 | `approveFeedback`、`cancelFeedback`、`openFeedbackPackage`、`routeDraftOperation` | `lib/workbench/submissionController.ts` |
| 引导与剪贴板 | `startOnboardingSession`、`importClipboardNow`、`openGithubReleases` | 独立小模块 |

**策略**：不追求一次性拆完。先做第一刀（见第八节），之后每遇到一次功能改动就顺手把对应组移出，
让 `App.svelte` 逐步收敛为「装配 + snippet 传参」。目标：6 个月内降到 600 行以内。

### 2. `SettingsPanel.svelte`（2120 行）— P1

**症状**：30 个 handler 覆盖 8 个互不相关的域（Web Access、语音模型、数据存储、Host、Pi、dsh、MCP、
通知与声音）。script 只有 783 行，模板约 1270 行——体积主要来自平铺的各个设置区块。

**策略**：按域拆成 section 组件，`SettingsPanel` 只保留导航与当前 section 的状态。

```text
lib/settings/
├── SettingsPanel.svelte          # 导航 + 路由，目标 <300 行
├── WebAccessSection.svelte       # 状态刷新、启停、令牌复制
├── SpeechSection.svelte          # 模型下载/删除、热词、设备
├── DataStorageSection.svelte
├── HostsSection.svelte           # Host 开关、安装、配置复制
├── PiSection.svelte / DshSection.svelte / McpSection.svelte
└── NotificationSection.svelte    # 弹窗通知、声音
```

每个 section 的异步动作抽到同目录的 `*.actions.ts`，让「拉取状态 → 渲染 → 触发动作 → 失败重试」
有单元测试，而不是只能靠组件测试。**这是本表里收益第二高的拆分**，因为它能顺带消掉大量
重复的 refresh/error 处理。

### 3. `httpApplicationTransport.ts`（1116 行）— P1

**症状**：三类职责混在一个文件——命令与投影表（33-145 行）、`HttpApplicationSession` 类
（232-742，约 510 行，含重连状态机与租约/撤销错误）、`HttpApplicationTransport` 类（1058 行起）、
以及资源键与规范化辅助函数（742-1057）。

**策略**：按「数据 / 会话 / 传输」三刀拆，保持 `lib/application/` 下的公开导出不变。

- `httpApplicationOperations.ts`：`HTTP_APPLICATION_OPERATIONS`、命令集合、资源键与投影键函数。
- `httpApplicationSession.ts`：会话类 + 四个错误类型 + 重连调度。
- `httpApplicationTransport.ts`：只剩 transport 类与装配。

配套把 `httpApplicationSession.test.ts`（1160 行）按状态机阶段拆成 3-4 个测试文件。

### 4. `ScreenshotOverlay.svelte`（1045 行）— P2

**症状**：40+ 个 handler 覆盖指针交互、选区缩放、标注增删与样式、撤销重做、工具栏拖拽与布局、
滚动截图、固定与取消。script 903 行，模板与样式仅 142 行——逻辑密度极高。

**策略**：把可脱离 DOM 的交互状态机抽成纯模块，组件只保留渲染与事件转发。

```text
lib/capture/overlay/
├── annotationModel.ts      # 标注增删、选中、撤销重做
├── pointerInteraction.ts   # 按下/移动/抬起、选区与标注缩放
├── toolbarLayout.ts        # 位置计算、拖拽、溢出面板
└── scrollCapture.ts        # 滚动拼接
```

注意：`ScreenshotOverlay` 只在 Desktop Client 存在（Web Access 明确不支持系统截图），
所以拆分不得把它变成浏览器能力。

### 5. `navigationController.ts`（702 行）— P1

**症状**：`createNavigationController` 一个工厂 702 行、内部 28 个函数，混合 inbox 投影、Host 会话事实
刷新、请求列表分页与搜索、作用域选择、重命名/置顶/归档。`navigationController.test.ts` 896 行，
与实现同构地膨胀。

**策略**：按资源拆成四个内聚模块，工厂只做组合与共享 store：

- `navigation/inbox.ts`：投影与通知跟踪。
- `navigation/hostSessions.ts`：事实刷新、重命名、置顶、归档。
- `navigation/requestList.ts`：分页、搜索、过滤器。
- `navigation/scope.ts`：作用域选择与代际（generation）校验。

保持 `NavigationState` 与返回接口不变，测试随之按模块拆分。

### 6. `ArchivedSessionsWorkspaceView.svelte`（720 行）— P2

**症状**：搜索匹配与高亮、归档数据加载、会话展开/选择、请求详情加载、解档与删除动作全在一个视图里。

**策略**：抽 `workspace/archiveSearch.ts`（匹配、排序、高亮）与 `workspace/archiveActions.ts`
（加载、解档、删除），视图只负责渲染与选择状态。

### 7. `RambleSessionController.svelte`（705 行）— P2

**症状**：script 681 行、模板仅 19 行——这是把控制器写进了组件。内容覆盖语音评审快捷键、
ramble 启动/恢复/语音录制、剪贴板捕获、控制台命令与状态广播。

**策略**：这是最典型的「组件即控制器」反模式。把状态机与副作用抽成
`lib/workbench/rambleSession.ts` 和 `lib/workbench/voiceCapture.ts`，组件退化为挂载点。
注意它与 `RambleConsole.svelte` 的状态广播耦合，抽取时要保留现有事件时序。

## 三、B 类：数据与注册表文件

### `i18n.ts`（931 行）— P3

单一 `chinese` 字典常量占 920 行。**这不是上帝文件，不要按逻辑拆分。** 行数来自翻译条目数量，
拆分不会提升可读性，只会增加 import。

**策略（可选，仅当翻译改动频繁造成冲突时）**：按域拆成 `i18n/settings.ts`、`i18n/workbench.ts`、
`i18n/agents.ts` 等，再由 `i18n.ts` 合并导出，保持 `t()` 接口不变。若未来新增前端行数门禁，
`i18n.ts` 应显式列入白名单并写明理由。

## 四、C 类：已模块化的 facade（逼近门禁）

这三个文件已经做过模块化，剩余体积来自类型定义、trait 实现和编排代码。**不要为了行数而拆**，
它们离 800 行门禁只差 1-99 行，需要的是「提前腾挪」而不是重构：

| 文件 | 行数 | 现状 | 建议 |
| --- | --- | --- | --- |
| `crates/rambledesk-core/src/feedback.rs` | 799 | 已拆 7 个子模块；余下为 `FeedbackApplication` impl（254-561）、错误码与错误类型（561-799） | 下一次触碰该文件时，把错误码与 `ApplicationError` 移到 `feedback/error.rs`，立即腾出约 240 行余量 |
| `crates/rambledesk-storage/src/sqlite.rs` | 701 | 已拆 17 个子模块；余下为 facade、`FeedbackRepository` impl、row mapping | 把 row mapping 移到 `sqlite/row_mapping.rs`（该模块已存在，把 576-695 的函数移进去） |
| `apps/desktop/src-tauri/src/diagnostics.rs` | 703 | 已拆 4 个子模块；余下为导出编排与 zip 打包 | 把 zip 打包与 README 生成移到 `diagnostics/package.rs` |

## 五、D 类：测试文件

测试超线通常不是架构问题，但会拖慢编译与审查。按被测场景拆分即可，不需要设计讨论。

**前端（4 个）**：`httpApplicationSession.test.ts`（1160）、`navigationController.test.ts`（896）、
`attachmentController.test.ts`（831）、`draftManagedSessionController.test.ts`（717）。
四个文件都只有一个 `describe`，按状态机阶段或资源拆成 2-4 个文件，与实现拆分同步进行。

**Rust（5 个）**：`sqlite/tests/workspace.rs`（791）、`managed_application.rs`（764）、
`sqlite/tests/requests.rs`（738）、`sqlite/tests/deliveries.rs`（727）、`application_api.rs`（725）。
同样按场景切分，注意 `sqlite/tests/mod.rs` 需要同步登记新模块。

## 六、观察名单（600-700 行，下一个功能就可能越线）

| 文件 | 行数 | 文件 | 行数 |
| --- | --- | --- | --- |
| `apps/desktop/src-tauri/src/web_access.rs` | 697 | `crates/rambledesk-local-server/src/lib.rs` | 691 |
| `apps/desktop/src-tauri/src/shortcuts.rs` | 693 | `apps/desktop/src/lib/preferences.ts` | 684 |
| `crates/rambledesk-speech/src/model.rs` | 692 | `apps/desktop/src/lib/RichFeedbackEditor.svelte` | 667 |
| `crates/rambledesk-local-server/tests/managed_feedback.rs` | 692 | `crates/rambledesk-local-server/src/application_api.rs` | 661 |
| `crates/rambledesk-local-server/tests/web_access_server.rs` | 674 | `apps/desktop/src-tauri/src/commands.rs` | 656 |
| `crates/rambledesk-storage/tests/structured_activity_runtime.rs` | 660 | `apps/desktop/src-tauri/src/pi_install.rs` | 648 |
| `apps/desktop/src/lib/agents/agentCatalogController.test.ts` | 659 | `crates/rambledesk-speech/src/speech_engine.rs` | 656 |
| `crates/rambledesk-local-server/tests/http_security.rs` | 643 | `crates/rambledesk-core/src/sessions/application.rs` | 634 |
| `apps/desktop/src/lib/application/httpApplicationTransport.test.ts` | 642 | `crates/rambledesk-storage/src/sqlite/request_ops.rs` | 623 |
| `crates/rambledesk-core/src/sessions/prompts.rs` | 618 | `apps/desktop/src-tauri/src/lib.rs` | 607 |

其中 `web_access.rs`（697）、`shortcuts.rs`（693）、`speech/model.rs`（692）、
`local-server/src/lib.rs`（691）距离 800 行门禁只剩约 100 行，属于「下一次改动前先腾挪」的对象。

## 七、门禁建议

现状：

- Rust：`check-rust-module-size.mjs`，`apps` + `crates` 下 `.rs` 限 800 行，已在 CI 三个 job 与
  release validate 中执行。
- 前端：**没有任何行数门禁**，而 20 个超标文件中 12 个是前端。

建议分两步，**都不要现在做**：

1. 第一刀与响应式落地后，新增 `scripts/check-frontend-module-size.mjs`，扫描 `apps/desktop/src` 的
   `.svelte` 与 `.ts`，初始阈值设 700，与 `check:rust-size` 一起挂进 CI。
2. 首次接入时把 A 类中尚未拆完的文件写入显式豁免清单（附理由与目标行数），要求只减不增；
   拆完一个就删一条。数据文件（`i18n.ts`）走白名单，不走豁免清单。

先拆后加门禁的原因：现在加会立刻让 CI 变红，且豁免清单会一次性长到失去约束力。

## 八、第一刀：抽 Workbench Shell

**为什么是它**：`App.svelte` 的 shell 部分是移动端响应式适配的唯一落点；纯布局数学已经抽到
`components/navigation/railResize.ts`（62 行，已有 `railResize.test.ts`）；shell 的三个子组件
（`HostSessionRail` 239 行、`RequestListPane` 226 行、`NavigationResizeHandle` 149 行）和持久化
（`lib/uiPreferences.ts` 145 行）都已就位。**所以这一刀搬的是状态所有权与装配，不是组件拆分。**

**抽取边界**：

| 关注点 | 当前位置 | 去向 |
| --- | --- | --- |
| `hostRailWidth`、`requestRailWidth`、`navigationWidth`、`resizingHostRail`、`resizingRequestRail` | `App.svelte` 312-316 | `WorkbenchShell.svelte` |
| `navigationWidths`、`hostRailMaxWidth`、`requestRailMaxWidth` | `App.svelte` 565-573 | `WorkbenchShell.svelte`（复用 `railResize.ts`） |
| 两栏布局模板（host rail、request list pane、两个 resize handle） | `App.svelte` 2204-2274 | `WorkbenchShell.svelte` |
| titlebar 与 workspace 内容 | 同文件 | 通过 snippet 从 `App.svelte` 传入 |

**约束**：

- 保持 `{#key $locale}` 与 `bind:clientWidth` 的语义——`navigationWidth` 是布局测量的根。
- `--workbench-sidebar-width` 仍必须作用到 `main`，`AppTitlebar` 依赖它计算宽度。
- 不得改变 `uiPreferences` 的持久化键，避免升级后丢失用户已有的侧栏宽度与折叠状态。
- 组件导出接口保持 `App.svelte` 现有传参形状，先纯搬迁，不做行为改动。

**验收**：`pnpm check`、`pnpm test`、`pnpm build:web` 通过。
注意 `apps/desktop/src/dev/NavigationResizePreview.svelte` 覆盖的是共享的 `NavigationResizeHandle`
（拖动、折叠、持久化），**它不渲染 `WorkbenchShell`**，所以 shell 的装配改动仍需在真实工作台里
人工点一遍侧栏拖动、折叠和刷新后的持久化。

**结果（已完成）**：新增 `lib/workbench/WorkbenchShell.svelte`（106 行，响应式阶段后 272 行），持有 rail
宽度、容器测量、拖动状态与 `fitNavigationWidths` 装配；`App.svelte` 通过 `hostRail`、`requestPane`、
`workspacePane`、`startupRecovery` 四个 snippet 传内容，通过 `bind:hostDisplayWidth` 与 `bind:resizing`
取回 `--workbench-sidebar-width` 和拖动类名。`App.svelte` 2498 → 2460 行。

**注意：这一刀只降了 38 行，响应式阶段又把 48 行状态接线加回 `App.svelte`（现 2508 行）。**
原因是两个 rail 的约 40 个业务 prop 必须留在 App（它们读取 `$navigation`、`workspace` 和业务 handler），
而折叠偏好与手机抽屉状态同时被 App 渲染的标题栏和两个 rail 读取。
**第一刀的真实收益不是行数，而是布局逻辑的归属**：断点、抽屉、触摸与 `--workbench-sidebar-width`
现在只有一个落点，且有 `workbenchShellRender.test.ts` 与 `shellMode.test.ts` 覆盖。
用行数衡量这次拆分是错的指标。

## 九、执行顺序与进度

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| 1 | 抽 `WorkbenchShell.svelte`（复用 `railResize.ts`） | 已完成（第八节） |
| 2 | 响应式适配（断点体系、窄屏抽屉、触摸目标）落在新 shell 上 | 已完成（[RESPONSIVE_SHELL.md](RESPONSIVE_SHELL.md)） |
| 3 | `navigationController.ts` 按资源拆分 | 部分完成：抽出 `navigation/navigationTypes.ts`、`navigationInputs.ts`、`hostSessionFacts.ts`，702 → 552 行；`navigationController.test.ts`（896 行）尚未拆分 |
| 4 | `SettingsPanel.svelte` 按域拆分 | 已完成：1941 → 623 行，抽出 `settings/` 下 Web Access、Adapters、Voice、Notifications 四个 section |
| 5 | `httpApplicationTransport.ts` 三刀 + 测试拆分 | 已完成：1116 → `httpApplicationOperations.ts` 402 + `httpApplicationSession.ts` 623 + `httpApplicationTransport.ts` 131；测试拆成投影 / 水位 / 流三个文件 + 共享 harness |
| 6 | C 类 facade 腾挪 | 已完成：`feedback.rs` 799 → 564（错误码移到 `feedback/error.rs`）、`sqlite.rs` 701 → 581（row mapping 移入已有 `sqlite/row_mapping.rs`）、`diagnostics.rs` 703 → 599（打包移到 `diagnostics/package.rs`） |
| 7 | 接入前端行数门禁 + 豁免清单 | 已完成：`scripts/check-frontend-module-size.mjs`（上限 700，9 个只减不增的豁免，`i18n.ts` 白名单），已进 CI 三个 job 与 release validate |
| 8 | D 类测试文件拆分、观察名单清理 | 未完成：`navigationController.test.ts` 896、`attachmentController.test.ts` 831、`draftManagedSessionController.test.ts` 717 仍在豁免清单里 |

剩余工作按优先级：三个大测试文件按场景拆（阶段 8）→ P2 的 `ScreenshotOverlay.svelte`（1046）、
`RambleSessionController.svelte`（706）、`ArchivedSessionsWorkspaceView.svelte`（721）。
前端门禁会阻止这些文件在拆分期间继续变大。

### `App.svelte` 的状态边界（进行中）

`App.svelte` 从 2525 行降到 2039 行，共享状态已经全部有主：

| 模块 | 拥有 | 测试 |
| --- | --- | --- |
| `lib/workbench/draftSession.ts` | 当前草稿 / 已保存草稿 / save phase / revision / editor 文档 | 7 |
| `lib/workbench/workspaceSession.ts` | 打开的请求、终态结果、已发布包、提交/批准/取消标志 | 6 |
| `lib/workbench/workspaceShellSession.ts` | 打开的视图、每个会话视图记住的请求、待激活目标、快照持久化 | 5 |
| `lib/workbench/attachmentSession.ts` | 采集/导入的忙碌标志、状态行、预览 URL、拖拽态 | 控制器测试 |
| `lib/workbench/rambleSession.ts` | Ramble 与语音实时状态 | 4 |
| `lib/workbench/submissionController.ts` | 批准 / 取消 / 打开反馈包 | 5 |
| `lib/workbench/startupController.ts` | 启动阶段、挂载标志、失败面、会话视图恢复解析 | 6 |

判据：**服务器事实不复制、跨组件共享才进 store、按领域切不按字段切**。App 只保留装配、
模板 snippet、组件句柄（`sessionWorkbench`、`rambleController`）和纯 UI 局部状态
（`resumePrompt`、`onboardingOpen`、`taskBriefOpen`、`projectSearch`、settings/archive 选择态、
`pageError` 及其去重游标）。

剩下的逻辑块（按耦合度排序）：

1. **`lib/workbench/workspaceNavigationController.ts`** —— `activateWorkspaceTab`、`closeWorkspaceTab`、
   `loadWorkspaceTarget`、`commitWorkspaceTarget`、`activateRequest`、`openRequest`、作用域保存/恢复，
   约 370 行。注意它与 `createWorkspaceTransition` 互相依赖：先声明
   `let workspaceNavigation: WorkspaceNavigationController`，transition 的 `loadTarget`/`commitTarget`
   用闭包调用它，控制器创建后再赋值（`startupController` 已经用过同样的 late-bound 组合）。
2. **`lib/workbench/managedSessionActions.ts`** —— `openAgentSession`、`openNewManagedSession`、
   `managedDraftPromoted`、`archiveSessionFromUi`、`deleteManagedSessionFromUi`，并把
   `deletingSessionCommands`、`deletingManagedSessionIds`、draft controller 缓存一并收进去。
3. **`lib/workbench/draftOperationsController.ts`** —— `activeActionFor`、`enqueueDocumentTask`、
   `routeDraftOperation`、`selectAction` 与 `activeActionByRequest`。
4. **cooking** —— `cookingRequestIds`、`cookedPreview` 收进 `cookingSession`。
5. **shell 布局偏好** —— `hostRailPreference`、`requestRailPreference`、`phoneHostRailOpen`、
   `phoneRequestRailOpen`、`shellMode` 收进 `shellLayoutSession`，让 `WorkbenchShell` 直接订阅。

全部完成后 `App.svelte` 应只剩 props、store/控制器装配、模板与 snippet。

### 为什么不能只靠搬函数

`App.svelte`（2526 行）是唯一没有走「搬函数出去」路线的 A 类文件，原因是它的局部状态耦合：
`restoreInitialWorkspaceSnapshot` 一个函数就依赖 `clearWorkspace`、`workbenchMounted`、`loadingWorkspace`、
`workspaceTransition`、`sessionViewResolutions`、`startupWorkspaceFailure`、`pageError`、`navigation`、
`applicationTransport` 等约 12 个 App 局部，`approveFeedback` / `cancelFeedback` 同样依赖
`workspace`、`completedResult`、`savePhase`、`approving`、`cancelling` 等读写点。
机械提取需要传入十几个 getter/setter，净减少接近零。

正确的拆法是先建立共享状态边界，再搬逻辑：

1. 把 `workspace`、`completedResult`、`savePhase`、`approving`、`cancelling` 收进一个
   `workbench/workspaceSession.svelte.ts`（或 store），App 只订阅；
2. 把启动 / 恢复序列（`startWorkbench`、`restoreInitialWorkspaceSnapshot`、
   `refreshSessionViewRecovery`、`applySessionViewResolutions`）抽成
   `workbench/startupController.ts`，依赖上面的状态对象；
3. 提交动作（`approveFeedback`、`cancelFeedback`、`openFeedbackPackage`）抽成
   `workbench/submissionController.ts`；
4. 托管会话动作（`openAgentSession`、`openNewManagedSession`、`archiveSessionFromUi`、
   `deleteManagedSessionFromUi`）抽成 `agents/managedSessionActions.ts`。

目标是把 `App.svelte` 收敛到 800 行以内的「装配 + snippet 传参」，但这是一次有回归风险的重构，
需要独立一轮来做，且要先补齐这三块的状态测试。
