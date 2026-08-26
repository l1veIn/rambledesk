# PR #12 现状审计与 Replacement PR 规划

> 状态：产品合同已确认，可按本文实施。
> 日期：2026-08-26。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文若与术语表冲突，以术语表为准。
> 审计对象：[PR #12 `feat(desktop): record-first ramble notes and light cleanup`](https://github.com/l1veIn/rambledesk/pull/12)。

## 结论摘要

建议以当前 `main` 新建 replacement PR，不在 PR #12 的分支上继续叠加修复，也不重写整个
RambleDesk。新的 PR 应当：

1. 保留 PR #12 中即时开始录音、后台加载 ASR、Light cleanup 能力、通用模型调用和 MCP 修复；
2. 删除 Ramble clip、Brief/Block Note、隐藏 Markdown marker、tooltip 编辑器，以及散落在 UI
   中的 capture save/terminal 锁；
3. 恢复 [ADR 002](adr/002-incremental-voice-ramble.md) 的“Stable 转写持续进入正文”和
   [ADR 003](adr/003-unified-ramble-state.md) 的“Ramble 是统一采集状态”，同时修订其中
   “切换 Request 前必须停止 Ramble”的旧条款；
4. 正文 = TipTap 里正在编辑的那一份 Feedback Draft。Task Brief 只读。Light cleanup 就是对
   待整理语音做一次可撤销的覆盖编辑，与用户自己改字等价；全程只有这一份正文；
5. 截图、附件在完成后再插入当前光标，不做发起时预留。点 Action 序号切换当前频道，新内容带归属
   粘贴进正文，且永不进入 cleanup；
6. 工作台全局最多一个 Active Ramble。它由一个未终态 Feedback Request 持有；切换当前可见
   Request 不停止采集，而是把所属 Request 的 Feedback Draft 会话保活，并投影到标题栏胶囊。

不引入 RambleJournal、raw sidecar、隐藏对象身份，也不按异步完成时间重排文档。

PR #12 不会因为创建 replacement PR 自动关闭。建议在 replacement 达到可测试状态前保持 #12
开放；新 PR 标明 `Supersedes #12`，验收后再手动关闭 #12。

`f589290`（MCP `tools/list` 缓存头）与内容流无关，应单独合进 `main`，不要等 replacement。

## 已确认的产品合同

### 唯一编辑面

- 所有“正文”都指 TipTap 中可编辑的 Feedback Draft。
- Task Brief（What happened、Actions to experience、context hint）不能编辑。
- 开始/停止一次麦克风会话不构成新的反馈对象或文档边界。
- 事实来源是 SQLite 中按 CAS revision 原子保存的版本化 TipTap 文档 JSON；由同一文档生成的
  Markdown 是 Cooking/提交投影。撤回只活在当前编辑会话，不持久化，也不改用浏览器缓存。

### 全局唯一 Active Ramble 与保活会话

- 工作台全局最多存在一个 Active Ramble；原生语音也只允许一个活动 `SpeechSession`。
- Active Ramble 始终有且只有一个所属反馈请求。它可以不是当前可见 Request，但所有语音
  Stable、Ramble 全局截图和剪贴板采集仍只进入所属 Request。
- 切换可见 Request 不停止 Ramble，也不销毁、清空或改装所属 Request 的 Feedback Draft
  会话。TipTap 文档、selection、Undo 历史、待整理/整理中区间、inflight cleanup、保存
  revision 和 save queue 必须一起保活。
- 工作台最多同时挂载两个 Feedback Draft 会话：Active Ramble 所属 Request 与当前可见
  Request；两者相同时只有一个。不得为所有访问过的 Request 建无限 editor cache。
- 标题栏胶囊和 Ramble Console 都是 Active Ramble 的状态投影和控制面，不是第二、第三编辑面，
  也不另挂 TipTap。它们只向 Active Ramble coordinator 发送意图，再由 coordinator 路由到所属
  Feedback Draft Session。点击胶囊回到所属 Request；主标签使用“Ramble 进行中”，并显示正在
  录音、整理中、暂停或错误等子状态。
- 保活从开始 Ramble 持续到明确结束、提交、批准、取消或应用退出；停止一次麦克风只改变输入
  子状态，不自动释放 Feedback Draft 会话。应用崩溃或重新载入不承诺恢复 Undo 和 cleanup
  会话态。
- 在另一个 Request 上开始 Ramble 必须显式 handoff：先停止、settle 并保存原所属 Request，
  再把唯一 Active Ramble 交给新 Request；不得静默抢占或同时录音。
- 所属 Request 自己的提交、批准、取消和删除不得在后台绕过会话直接执行；必须先回到该
  Request，完成有界 drain/save，再进入终态。其他 Request 的终态动作不影响这个进行中的
  Ramble。

### Light cleanup = 用户编辑

- 用户启用后，系统把待整理语音原地换成整理结果。语义等于选中这段文本再粘贴覆盖。
- 因此它可被 Undo 撤回，在产品语义上等同于用户自己整理。不是第二套证据，不单独存 raw。
- 这里的监督不是要求用户实时注视每次自动替换，而是：用户显式启用、结果始终留在可编辑
  Feedback Draft 中、当前会话可撤销，并且最终由人类明确提交。Uncooked 是在这种人类治理下
  形成并确认的源反馈证据，不是逐字转写证据。
- 未启用 Cooking 时，`feedback.md` 与 `uncooked.md` 来自同一份 Feedback Draft 的 Markdown
  投影（可以已经过 cleanup）。Cooking 的原稿也是这份投影。
- 失败或超时：原文不动。
- 默认关闭；启用才自动找时机。涉及模型服务、隐私和费用。

### 插入规则

- 语音 Stable 追加到正文末尾，立即可见并随草稿自动保存。
- Partial 只用于临时 UI。
- 截图、文件、剪贴板图片：完成后再插入**当时光标**；取消则什么都不插。不预留、不重排。
- Active Ramble 的全局快捷键、Ramble Console 和自动剪贴板采集始终指向所属 Request，使用
  其保留的最后合法光标，并写进同一个所属 Session，不得再开一个 editor 或改走 SQLite merge。
  当前页面里的普通编辑、Action 和附件按钮则指向当前可见 Request。
  只有发生在所属 Request 或来自 Ramble 全局采集的非语音输入，才按“插入非语音”时机触发
  其 cleanup；在另一个可见 Request 内进行的普通编辑、Action 粘贴或附件操作不得额外改动
  所属 Request。
- 点击 Action 序号：切换当前 Action 频道。之后追加到正文段尾的语音、截图和粘贴带上该
  Action 的节点归属，Markdown 用 `------------------------ Action 2 ------------------------` 可读分隔线导出。再点一次同一序号回到默认频道，Markdown 用无标签长横线分隔。不把 Action 指令原文贴进正文，也不从分隔线反向重建节点属性。

### 待整理 / 整理中 / 正文

只认一种会进模型的东西：**待整理语音**（麦克风追加、还没进过 cleanup 的 Stable 文本）。

| 状态 | 含义 |
| --- | --- |
| 待整理 | 刚说的、还没送去整理 |
| 整理中 | 已送出；原文留着，轻提示，锁住 |
| 正文 | 其余一切：整理结果、手打、Action、截图、附件 |

规则：

1. 接着说就接着攒。新的 Stable 接到当前待整理后面。整理中的那一段锁着，新语音从它后面
   另起一截待整理。
2. 满一截、停一停、或开始干别的，就把当前待整理送去整理。一次只跑一段。
3. 模型只吃这一截语音。返回后原地覆盖、解锁，变成普通正文。

触发（任一即送）：

- 待整理达到 3 个 Stable；
- 待整理超过约 500 字；
- 上一 Stable 之后约 3 秒没有新语音，且待整理非空；
- 在所属 Request 上、或经由 Ramble 全局采集插入非语音：截图落地、粘贴、拖文件；
- 停止 Ramble、Cooking、提交。

不触发：光标移动、打开 Task Brief、切窗口但还在说、切换当前可见 Request，以及在另一个
可见 Request 内的普通编辑 / Action / 附件操作。

用户在待整理里改 ASR 错字：仍算待整理，送出的是改后的字。在已经是正文的段落里打字：
普通编辑，不进模型。

### 整理中保护

- 该段可见、带轻提示，不可选中编辑；不换成占位句。
- 只要存在整理中文本：禁用 Undo/Redo 按钮和 Ctrl/⌘Z、Ctrl/⌘⇧Z。
- 整理完成后，这次覆盖出现在 TipTap 历史里；用户按撤销即撤回整理，回到覆盖前的语音。
- 截图、Action、粘贴的落点若在待整理或整理中段内：先把待整理送去整理并锁住，再把插入
  改到锁段之后。落点若在已经是正文的别处：仍插在光标处（真剪贴板），并仅当这次插入属于
  所属 Request 或 Ramble 全局采集时，才按“插入非语音”把当前待整理送去整理。
- 新语音追加在锁段之后，不堵麦克风。
- 提交/Cooking 前有界等待（约 30s）。超时则保持编辑器现有原文并允许提交，不锁死 UI。

代价（接受）：整理中期间整篇不能撤销，包括这段时间内插入的 Action 或新语音。整理结束后
先撤销的是那次覆盖。

### 重新载入与重启

- 存盘只有一份 Markdown。重启后未完成的待整理/整理中都变成普通正文，不再自动补整理。
- 用户主动重新载入 Workspace 时，先停止原生语音、对所属 Session 做有界 settle/save、使当前
  session generation 失效并 dispose，再从 SQLite 重新 hydrate；重新载入不是保活导航。
- webview 意外重新载入时不能依赖前端异步 teardown。新前端启动后必须与原生层 reconcile：
  发现没有对应 owner/session generation 的孤儿 `SpeechSession` 就停止它，再把已保存 Markdown
  作为普通正文恢复。
- Stable、cleanup、附件和保存回调都必须带 session generation 或等价身份校验；Session 已失效
  后返回的旧结果一律丢弃，不能覆盖重新 hydrate 的 editor。

## 1. 审计基线

### 1.1 Git 与 GitHub 状态

截至 2026-08-26：

| 项目 | 状态 |
| --- | --- |
| PR | #12，Open，非 Draft；GitHub `mergeable_state=behind` |
| 作者 | `oldcai` |
| PR head | `6979b52afdf8f85592a645f8f5c540e86bb66045` |
| 本地测试分支 | `codex/pr-12-local-test`，与 PR head 完全一致 |
| merge base | `793bf9c581ec3f8396efc4e5170cf843e31fc1e6` |
| 当前 `origin/main` | `35d4d3e275e0a308388bf7955113a17c8d700440` |
| 规模 | 29 个提交，38 个文件，约 `+3226/-240` |
| 与 main 的关系 | PR 落后 3 个提交，均为 README/产品叙事文档改动 |
| CI | `verify`、`verify-windows`、`verify-macos` 均通过 |

已完成的本地机械验证包括：

- `pnpm check`：0 errors / 0 warnings；
- `pnpm test`：27 个测试文件、151 个测试通过；
- `cargo test -p rambledesk-mcp -p rambledesk-speech`：MCP 15、speech 11 通过；
- Rust fmt、clippy/模块大小与术语相关静态门槛通过。

这些结果说明分支的机械完整性较好，但不能证明当前领域模型和交互模型适合作为后续架构基础。
目前只完成了局部人工交互确认：正文段落右侧的录笔记入口可用，但发现性偏弱；尚未把整条
native 流程作为 replacement 目标验收。

PR 作者已记录已知缺陷：文档编辑和 capture 落地竞态会让 `rambledesk-capture://` marker
成对丢失。这是 clip 状态与 Markdown 双写的直接证据。

### 1.2 PR 之前的行为

`main` 上的语音 Ramble 遵循 ADR 002：

- `Stable` 转写事件在录音过程中直接作为普通 Markdown 段落追加到正文；
- `Partial` 只用于临时 UI，不制造 Draft revision；
- 停止录音只结束当前语音会话，不把整段录音重新建模成一个可编辑对象；
- 正文编辑、截图、剪贴板和附件是可并行的输入链路；
- Draft 和附件走已有 CAS/持久化链路，不存在第二套 clip/note 状态。

它的主要体验问题是启动录音前等待 ASR 模型初始化，以及缺少自动的轻度转写整理；它的基本
文档流方向没有问题。主动截图插入当前光标，见 ADR 003，replacement 保持这一点。

### 1.3 PR #12 之后的行为

PR #12 把主路径改成了 record-first、stop-to-materialize：

- 麦克风立即开始，ASR 在 worker 中后台加载，启动体感明显改善；
- `Stable` 片段先累积在 `sessionChunks`，停止录音时合并成一个 `RambleClip`；
- 每次开始—停止生成一个 magazine clip，并通过隐藏 marker 写入 Markdown；
- Task Brief 的 `what_happened`、Action、context ref 都可以单独启动 Brief Note 录音；
- clip 和 note 有各自的 tooltip/inline 编辑入口，修改后再反向替换正文；
- Light cleanup 在一次录音停止后运行，完成后才把整理结果写入正文；
- 截图仍按编辑器光标插入，语音要等停止后才写入；
- 为处理异步 cleanup、跨 Request 写入、Cooking、提交/批准/取消之间的竞态，加入了多组
  capture queue、save queue、retry latch、entry lock 和 terminal lock。

当前 PR 在代码层已经不能同时启动两个麦克风 Ramble：原生 `WorkbenchState` 只有一个
`Mutex<Option<SpeechSession>>`，再次启动会被拒绝；前端也只有一组全局 `ramblePhase +
rambleRequestId`。这说明“全局唯一”不是新增的底层限制，而是把已有偶然约束提升成明确产品
合同。现有缺口是：Request 可以切换，标题栏也已有只读胶囊，但所属 Request 的 TipTap 会被
当前可见 Request 的 Markdown `setContent` 改装；后台 Stable/cleanup 随后绕过 editor 直接保存，
因而丢失 selection、Undo 和区间会话态。

用户可见能力丰富了，但产品模型从“连续产生一份反馈正文”变成了“先产生多个录音对象，再把
它们同步到正文”。这正是实现复杂度急剧上升的根源。

## 2. 与既有产品和架构合同的关系

### 2.1 符合既有合同的部分

下列方向应保留：

- Ramble 仍附着于一个未终态的反馈请求；
- 最终仍发布不可变反馈包；
- Light cleanup 与 Cooking 分离，失败时退回原文；
- 本地语音不可用时，文字、截图和附件仍可完成反馈；
- `request_id` 用于把异步结果路由回正确请求；
- 原始音频默认不落盘，录音队列保持有界；
- UI 的瞬时状态不应成为反馈请求、Draft 或反馈包的唯一事实来源。

### 2.2 对 ADR 002 的偏移与恢复

ADR 002 要求录音过程中持续产出 Stable 并追加到正文，停止录音不重新整体替换此前稳定片段。
PR #12 改为停止时生成 clip。replacement 恢复“Stable 立即进入正文”。

自动整理改成对已在正文中的待整理语音做一次可撤销覆盖，而不是延迟正文首次出现。
音频 warm-up 队列从 512 增至 4096 随 speech 基础设施一并移植，并在 20 分钟录音中确认
内存有界；若保留 4096，在 ADR 002 的补充里写明，不另建内容流模型。

### 2.3 对 ADR 003 的保留与修订

ADR 003 把 Ramble 定义为高于单个输入工具的统一采集状态，且主动截图插入用户最后的编辑
光标。PR #12 的 `voiceSink = ramble | brief-note` 把采集状态重新分裂。replacement 删除
Brief Note 录音：语音始终进入 Active Ramble 所属 Request 的 Feedback Draft。截图完成后再
插入所属 Request 保留的当前光标，不改为按发起时间预留。

ADR 003 当前的“切换 Request 前必须停止 Ramble”与新合同冲突。replacement 保留“Ramble 是
统一采集状态”和文档流映射，但由新的 ADR 明确取代该导航条款：切换只改变当前可见 Request；
全局唯一 Active Ramble、所属 Request 和保活的 Feedback Draft 会话不变。ADR 003 继续作为
原始决策保留，并链接到取代该条款的新 ADR，不静默改写历史。

### 2.4 Uncooked 与 Light cleanup

术语表把 Uncooked Feedback 当作原始人类证据。这里需要正式改定义，而不是只补一条例外：
Uncooked 是 Cooking 前由人类直接形成、持续可编辑并在提交时确认的源反馈正文；`Uncooked`
表示“未经 Cooking”，不表示逐字转写或未经任何机器辅助。

Light cleanup 由人类显式启用，结果原位可见、当前会话可撤销，并且不能绕过 Feedback Draft
直接发布，因此属于人类治理下的草稿编辑。成功后不保留 cleanup 前转写（除当前会话的 Undo）；
失败/超时时编辑器里仍是原文。自动 cleanup 可能在所属 Request 不可见时发生，所以合同使用
“人类治理并最终确认”，不声称每次替换都处于实时注视下。

这不是“第二份 raw 证据合同”，而是明确 cleanup 属于草稿编辑。Phase 0 修订术语与宪章中
“不可追溯的 LLM 重写”的表述：禁止的是绕过可编辑 Feedback Draft 和人类提交确认的模型
改写，不是默认关闭、结果可见且当前会话可撤销的轻度整理。

当前 `submit_feedback` 仅在同时提供 Cooking 结果时才接受独立的 `uncooked_markdown`。
本 replacement **不必改这条 API**：无 Cooking 时两份正文继续相同。

## 3. PR #12 修改了哪些基础设施

### 3.1 应保留或移植

| 能力 | 当前实现 | 评价 | replacement 处理 |
| --- | --- | --- | --- |
| 即时开始录音 | `rambledesk-speech` 先启动采集，worker 后台创建 recognizer | 有价值，直接改善首句体验 | 移植最终实现及测试 |
| 音频 warm-up 缓冲 | 队列从 512 增至 4096，覆盖模型加载窗口 | 有价值，需 20 分钟验收 | 移植 |
| recognizer worker 拆分 | `native/worker.rs` | 符合模块大小和线程所有权 | 移植最终文件内容 |
| 通用模型调用 | `cooking.ts` 提取 `generateModelText` | Light cleanup 与 Cooking 可复用 | 移植，不要携带 marker prompt |
| Light cleanup | 独立设置、prompt、测试、失败回退 | 领域方向正确 | 保留能力，重写触发与写回 |
| Transcript timeout | 30 秒后保留原文 | 防止模型调用阻塞后续输入 | 保留超时，删除 clip queue |
| MCP `tools/list` | 明确 `ttlMs=0`、`cacheScope=private` | 与内容流无关 | **单独合进 main**：`f589290` |
| 录音提示音 | 录音启动反馈 | 可选的交互增强 | 保留 arm sound，删除 clip rack sound |

### 3.2 应重写或删除

| 区域 | 当前问题 | replacement 方向 |
| --- | --- | --- |
| `App.svelte` capture orchestration | 保存链、失败列表、重试、Cooking mirror、terminal counter 集中在 UI shell | 只保留导航和状态投影；Draft Session 拥有 editor/save/cleanup，terminal action 调用其 settle |
| `RambleSessionController.svelte` | 同时拥有 Ramble、Brief Note、语音 session、cleanup、clipboard、drain/lock | 收窄为语音输入；产出 Partial/Stable，由 Active Ramble 路由到所属 Draft Session |
| `briefNotes.ts` | 混合 Task Brief parsing、clip UI、marker serialization、Markdown 替换 | 删除 clip/note/marker；Action 序号切换当前归属频道 |
| `publisherController.ts` | 发布接口必须理解 capture drain 和 terminal lock | 只等待 inflight cleanup / 草稿保存 |
| Task Brief preview | 展示、录音、编辑、clip magazine 混在一个 Dialog | 只读展示；序号点击切换当前 Action 频道 |
| Markdown marker | `rambledesk-capture://...` 零宽链接承担对象身份 | 删除 |
| 整理写回 | `setContent` 整篇替换，无法按一次覆盖来撤销 | 对目标区间做 TipTap transaction，进入 Undo 历史 |
| Request 切换 | 复用同一 TipTap 并以 `setContent` 换文档，所属 Request 的 selection、Undo 和区间会话态失效 | 保活 Active Ramble 所属 Feedback Draft 会话；当前可见 Request 使用独立会话 |
| 标题栏胶囊 / Ramble Console | 只显示状态，或另开编辑面；所属 TipTap 并不存活 | 都投影同一所属 Session；点击胶囊返回；Console 不另挂 editor |

### 3.3 PR 没有修改、replacement 也不扩张的基础设施

replacement 不新增 journal 表、reservation 或 raw evidence 列。Draft 使用版本化 TipTap JSON、
Markdown 导出投影和一个 `expected_revision` 原子保存；节点类型、属性和 marks 属于可恢复文档，
inflight cleanup task、selection 与 Undo 属于编辑器会话态。

跨 Request 导航时，不再让 Stable 或异步 cleanup 绕过 TipTap、直接 merge SQLite Markdown。
Active Ramble 所属 Feedback Draft 会话始终存活；语音追加、cleanup replace、Undo 和 autosave
继续通过同一个 editor transaction/save queue 完成。`request_id` 负责确认所有权和隔离保存，
不是绕开 editor 的后台写入口。ADR 005 把该边界升级为结构化 Draft 存储模型。

## 4. 术语审计

### 4.1 保持不变的核心术语

人类、智能体、宿主、工作台、本地服务、反馈请求、反馈包、适配器、continuation、
context hint、Cooking、Cooked Feedback、身份字段、适配器分类：replacement 不改协议。

### 4.2 已发生或正在发生的术语偏移

| PR 中的词 | 偏移 | 判断 |
| --- | --- | --- |
| `Ramble clip` | 把 Ramble 从持续采集状态改造成一次 start-stop 产生的对象 | 不加入术语表；删除 |
| `Brief Note` / `Block Note` | 在 Feedback Draft 之外建立第二反馈通道 | 不加入术语表；改为当前 Action 频道归属后在正文继续说/写 |
| `capture` | 扩大为 speech/note/Markdown wrapper 的通用对象身份 | 收窄；不得把 speech/note 称为 capture |
| `rambledesk-capture://` | 内部对象身份泄漏进用户 Markdown | 删除 |
| `Record-first` | 把录音按钮提升为内容边界 | 只可作为实现描述 |
| `Operator` | 与术语表的“人类”并存 | 生产代码、UI、prompt 统一 Human/人类 |
| `annotation/批注` | 易被理解为 Task Brief 上的语音备注 | “批注”保留给截图编辑 |
| `Recording Request` / “录音中的 Request” | 把麦克风子状态误当成 Feedback Request 自身状态 | 使用“Active Ramble 所属反馈请求”；Recording 只描述麦克风输入子状态 |

### 4.3 需要修订的既有术语

#### Ramble

> **Ramble**：由一个未终态反馈请求持有的统一、可长时间持续的反馈采集状态。人类可以在该
> 状态中说话、编辑正文、截图、添加附件并把新内容归到某个 Action；开始/停止一次麦克风会话不构成
> 新的反馈对象或文档边界。所属反馈请求可以不是当前可见 Request；工作台全局最多一个 Active
> Ramble。Ramble 属于人类工作流，不属于适配器协议，也不是系统级听写。

#### Light cleanup

> **Light cleanup**：用户启用后，系统对 Feedback Draft 中尚未整理的语音做自动轻度整理，
> 去掉语气词、修正断句且不改变原意。它等于对这段文本的一次覆盖编辑，可撤销，不生成正式
> 反馈结构，不单独保存覆盖前文本。失败、超时则原文不动。整理过程中该段不可编辑，且暂时
> 禁用撤销。它不是 Cooking。默认关闭。

#### Uncooked Feedback

需要正式修订定义：

> **Uncooked Feedback**：Cooking 前，由人类直接形成、持续可编辑并在提交时确认的源反馈
> 正文。它可以包含人工编辑，以及用户显式启用、结果原位可见、当前会话可撤销的 Light
> cleanup。`Uncooked` 表示“未经 Cooking”，不表示逐字转写或未经任何机器辅助。提交后保存
> 为反馈包中的 `uncooked.md`，Cooking 不得覆盖它与 Cooked Feedback 的来源关系。

#### Feedback Draft

建议加入核心术语表（宪章已有 Draft）：

> **Feedback Draft**：一个反馈请求处理期间持续持久化、可由人类在 TipTap 中直接编辑的结构化
> 文档。语音、截图、附件和带 Action 归属的内容都进入其中。它是可变的 request-scoped 状态，
> 不是反馈包；工作台展示和编辑它，SQLite 完整保存其文档内容。

`Feedback Draft Session` 只作为实现词：表示一个 Request 在当前应用进程中的 TipTap、selection、
Undo、cleanup 区间和保存队列。它不进入核心术语表，不是新的持久对象；应用重启后只从 SQLite
恢复 Feedback Draft，不恢复 Session。

#### Action

`actions[]` 已是协议字段。建议在核心术语表给出产品对象：

> **Action**：反馈请求中带稳定 id 的一项真实使用或检查指令。它属于反馈请求的不可变输入。
> 点序号把后续正文归到该 Action；再点一次回到默认频道。这不等于修改原请求，也不等于
> `context_refs`。

不新增 **Task Reference** 作为领域对象。Action 是频道，不是粘贴。

### 4.4 不建议加入术语表的词

Content Flow、RambleJournal、Ramble owner、Feedback Draft Session、settled/unsettled span、
冻结、placeholder/reservation、clip/note/magazine/rack、generic Capture。产品文档使用
“Active Ramble 所属反馈请求”；会话实现里可用“待整理 / 整理中 / 正文”描述状态，不必把这些
实现关系和状态升格为持久产品对象。

## 5. Replacement 的目标体验

用户只开始一次 Ramble：

1. 第一段话的 Stable 立即出现在 TipTap 并进入草稿保存；
2. 待整理达到 3 个 Stable、约 500 字、停口约 3 秒，或用户去截图/粘贴时，这段
   语音进入整理中：原文仍在，轻提示，锁住，Undo 禁用；麦克风继续；
3. 用户截图并批注；完成后插在当前合法光标（若在锁段内则在锁段后）；
4. 用户点 Action 2，序号高亮；之后新内容归到 Action 2，Markdown 出现带 `Action 2` 的可读分隔线；
5. 用户继续说，新语音出现在锁段之后，成为新的待整理；
6. 整理返回，锁段被覆盖成整理结果，Undo 恢复可用；再按撤销即回到覆盖前；
7. 用户切换到 Request B；Request A 的 editor 会话被保活，语音、cleanup 和 autosave 继续只在
   A 上运行；B 使用独立 editor，可以正常查看和编辑；
8. 标题栏持续显示“Ramble 进行中 · Request A”及录音/整理子状态；用户点击胶囊回到 A，看到
   同一个 TipTap、selection、Undo 历史和 cleanup 区间；
9. 用户只在 TipTap 里改字；Task Brief 只读，序号只负责粘贴；
10. 停止 Ramble 时把剩余待整理送去整理；提交前有界等待；超时用编辑器里现有文字。

最终文档顺序按实际插入发生，不追求“发起时间线重建”。交错语音与截图不必完美。

若用户在 B 上尝试开始 Ramble，工作台不得产生第二个录音。UI 明确说明 A 正在进行，并提供
“返回 A”或“停止 A 并在 B 开始”的 handoff。handoff 必须等待 A 的有界 cleanup/save；失败
时保留 A 为所属 Request，不产生半切换状态。

## 6. 模块与 seam

不建立 RambleJournal。复杂度应留在编辑器会话里，而不是平行对象图。

建立一个深的 **Feedback Draft Session module**，把 request-scoped 的 TipTap、selection、
Undo、待整理/整理中区间、cleanup 写回、CAS revision、autosave queue 和 terminal settle
收在同一个 interface 后面。删除这个 module 时，上述复杂度会重新散落到 `App.svelte`、
`RambleSessionController`、editor 和 publisher，说明它有足够深度，不是透传 wrapper。

它对调用方只暴露少量意图级操作，例如：

- 追加 Stable、插入附件/Action、应用普通编辑；
- 查询当前 Markdown 和可提交状态；
- 在 handoff、终态或主动 reload 前有界 settle；
- 挂载为可见 editor 或保留为隐藏的 Active Ramble 所属会话；
- dispose 非活动会话。

待整理 mark、位置映射、cleanup generation、History transaction、save revision 重试等都是
implementation，不得泄漏成 `App.svelte` 需要组合的接口参数。

工作台级建立一个 interface 很小的 **Active Ramble coordinator**：对外只表达 start、pause/
resume、handoff、end 和 return-to-owner；implementation 维护 `ownerRequestId + phase`、执行
全局唯一约束和 handoff 顺序、把语音与全局采集路由给所属 Feedback Draft Session，并把状态
投影给标题栏胶囊和 Ramble Console。它不拥有正文、cleanup 区间或保存链。语音模块只负责
开始/停止全局唯一麦克风并产出 Partial/Stable，不理解 Request 导航和 Markdown。

Session host 最多保留 Active Ramble 所属会话和当前可见会话；二者相同时复用同一实例。
Request 切换不得对所属 editor 调用 `setContent`，也不得把后续 Stable/cleanup 改成直接写
SQLite。生产路径是 TipTap transaction → 文档 JSON + Markdown 投影 → `save_feedback_draft`。

测试以 Feedback Draft Session 的 interface 为主要 seam，使用真实 TipTap state 和可控的本地
保存 adapter 验证可见结果、Undo、保存隔离和 settle；纯文本 cleanup 仍可有纯函数测试。新的
interface 测试建立后，删除只验证旧 clip/note/marker 内部状态的浅层测试，不叠加两套模型。

当前 `applyExternalMarkdown` / `setContent` 会整篇替换，不能当这次覆盖的 Undo 单位。
语音追加和 cleanup 必须走编辑器 transaction。

## 7. 分支和代码移植策略

### 7.1 分支

从当时最新 `origin/main` 创建：

```text
codex/continuous-content-flow
```

不要从 `6979b52` 或 PR #12 的中间提交建分支。`8c2c173` 已经混合了语音基础设施和 clip/note
模型。

MCP 修复用独立 PR 合进 main，不必等这条分支。

### 7.2 直接 cherry-pick

| commit | 处理 |
| --- | --- |
| `f589290` `fix(mcp): answer tools/list...` | 单独 PR 进 main |

### 7.3 按最终文件或 hunk 移植

| 来源 | 移植内容 | 不携带 |
| --- | --- | --- |
| `native.rs` + `native/worker.rs` | 麦克风先启动、后台 recognizer、abort、4096 队列、worker 拆分 | clip/note |
| `cooking.ts` | `generateModelText` 和 LLM readiness | `rambledesk-capture://` 保留规则 |
| `lightCleanup.ts` + tests | prompt、模型调用、空结果/原文回退 | 停止录音才触发、先 cleanup 再首次写正文 |
| preferences/settings/i18n | 独立开关、prompt、provider、隐私提示 | clip/note 文案和 sound |

`5bb0931` 不能单独 cherry-pick；把最终 native speech 状态整理成一个可审阅提交。

### 7.4 不移植

- `BriefNoteBlock.svelte`、`RambleClipIcon.svelte`、`briefNotes.ts` 及 marker/tooltip/clip
  tests；
- `rambleClipsByRequest`、`briefNotesByRequest`、capture save/retry/terminal locks；
- `voiceSink = brief-note` 和 Brief Note lifecycle；
- Task Brief 的录音和编辑能力；
- clip rack sound 和 fly-in；
- Cooking prompt 中保存 `rambledesk-capture://` 的规则。

PR #12 后半段的故障知识仍应写成测试，但目标换成“一份草稿 + 区间 replace”，例如：

- Active Ramble 属于 A 时切换到 B，A 的异步 cleanup 仍通过 A 的保活 editor transaction
  覆盖目标区间，B 的正文和 revision 不变；
- 点击标题栏胶囊回到 A，仍是同一个 editor session，cleanup 覆盖可以 Undo；
- A 活动时在 B 请求开始 Ramble 不会产生第二个语音 session；handoff 失败仍由 A 持有；
- 目标区间已被用户修改或已不存在时，丢弃 cleanup 结果；
- 保存失败不能被 `allSettled` 误判成功；
- Cooking snapshot 不能丢掉其后到达的正文；
- 同一段待整理只能交付一次整理结果。

## 8. 建议实施阶段

### Phase 0：术语和 ADR

- 修订 `TERMINOLOGY.md`：Ramble、Light cleanup、Uncooked 与 Draft 的关系；新增 Feedback
  Draft、Action；不同步增加 Task Reference。
- 同步 `PRODUCT.md`、`ARCHITECTURE.md`、`CONSTITUTION.md`；`PROTOCOL.md` 若提到 Uncooked
  为“未经任何整理的原文”，改为与本文一致。
- 新增一份短 ADR：恢复增量入正文；cleanup 是可撤销覆盖；一份草稿；全局唯一 Active
  Ramble；所属 Feedback Draft Session 跨 Request 导航保活；不引入 clip/journal。
- ADR 002 保持 Accepted。ADR 003 保留并链接新 ADR；其中“切换 Request 前必须停止”由新
  ADR 明确取代，其余统一采集和文档流决策继续有效。4096 队列若保留，在 002 的补充中说明。
- 运行术语 residual scan。

### Phase 1：独立基础设施

- MCP 修复单独进 main；
- 移植 native speech background-load；
- 提取通用模型调用和 Light cleanup 设置；
- 保持 main 的 Stable 立即入正文；不引入 clip/note/marker。

### Phase 2：Feedback Draft Session 与 Active Ramble ownership

- 建立深的 request-scoped Feedback Draft Session，收拢 TipTap、selection、History、revision、
  autosave queue 和 terminal settle；
- 工作台全局只允许一个 Active Ramble，明确 `ownerRequestId`；原生单 `SpeechSession` 约束保留；
- Session host 最多挂载所属会话与当前可见会话；返回所属 Request 时复用同一个 editor；
- 标题栏胶囊可点击返回所属 Request，并展示 Ramble 主状态与录音/整理子状态；Ramble Console
  与胶囊只消费 coordinator snapshot、向 coordinator 发送意图，不另挂 editor；
- 删除跨 Request 的直接 Markdown/SQLite merge 路径；语音和 cleanup 始终进入所属 editor；
- 实现显式 handoff，以及所属 Request 提交、批准、取消、删除前的返回与有界 settle；
- 终态锁必须 request-scoped：其他 Request 的提交、批准、取消、删除只 settle 自己的 Session，
  不停止、锁住或更换当前 Active Ramble；
- 实现主动重新载入的 teardown，以及意外 webview reload 后的原生语音 reconcile、generation
  失效和旧异步结果丢弃。

### Phase 3：待整理跟踪与可撤销 cleanup

- Stable 追加为带待整理标记的编辑器 transaction；
- 按已确认触发启动 cleanup；一次一段；整理中锁段并禁用 Undo/Redo；
- 成功则区间 replace 进入 History；失败/超时解锁且原文不动；
- 非语音节点永不进模型。

### Phase 4：Action 粘贴与 UI 删除

- Action 序号切换当前频道并为新节点盖章；
- Task Brief 只读；
- 删除 clip magazine、note button、tooltip 第二编辑面；
- Markdown 只保留普通文本、引用、列表和标准 `attachment://`。

### Phase 5：残留清理与验收

- #12 若未作为正式版本分发，不为隐藏 marker 建兼容层；测试者若留下重要 Draft，提供一次
  性把 wrapper 转成普通 Markdown 的清理即可；
- 删除旧 UI、state、i18n、测试残留；
- 自动化、native、长录音验收；
- 打开 replacement PR，门槛达标后关闭 #12。

## 9. 验收矩阵

### 9.1 自动化

- `pnpm check`、desktop 单元测试、Rust fmt/clippy/模块大小；
- core/storage/local-server/speech/MCP 测试（Draft 合同无 schema 扩张则不必新集成表）；
- Feedback Draft Session interface 测试覆盖 owner/visible 隔离、Undo、cleanup、CAS save 和
  terminal settle；
- Active Ramble 测试覆盖全局唯一、显式 handoff、胶囊返回和最多两个已挂载 editor；
- request-scoped terminal 测试覆盖 A Active 时 B 的提交/批准/取消/删除，以及这些动作不停止或
  锁住 A；
- cleanup trigger 使用 fake timer 验证：切换到 B 及 B 内编辑/Action/附件不触发 A，但 A 自己的
  Stable 数、字数和停口计时仍能触发；
- reload 测试覆盖主动 teardown、孤儿原生语音 reconcile、session generation 失效，以及旧
  cleanup/附件/save 回调不会覆盖新 editor；
- Markdown residual：正文与 Cooking 输出均无 `rambledesk-capture://`；
- 术语 residual：生产代码和 UI 不再出现 Ramble clip、Brief Note、Block Note、Operator、
  Recording Request。

### 9.2 关键场景

| 场景 | 预期 |
| --- | --- |
| 连续说话超过 3 个 Stable | 原始 Stable 转写已在正文并已保存；该段整理中锁住；新语音在其后另起待整理 |
| 待整理超过约 500 字 | 触发整理，不等第三个 Stable |
| 说完去点目标软件，约 3 秒无新语音 | 待整理被送去整理 |
| cleanup 超时/失败 | 原文保留并解锁；后续语音不阻塞 |
| 整理中打字、Undo、把 Action/截图插进该段 | 不能改锁段；Undo 禁用；插入改到锁段后 |
| 整理完成后 Undo | 撤回这次覆盖，语音原文回来 |
| 手改待整理里的错字后再触发整理 | 模型吃改后的字 |
| 说话 → 截图完成 → 继续说 | 截图在完成时光标处（或锁段后）；不重排 |
| 文件选择器取消 | 不插入 |
| 点击 Action 序号 | 切换当前频道；不插入指令原文；不进 cleanup |
| A 正在 Ramble 时切到 B | A 的 editor、selection、Undo、cleanup 和 save queue 保活；B 使用独立 editor |
| A 隐藏时继续说话/cleanup 返回 | Stable 和 replace 只通过 A 的 editor transaction；B 正文和 revision 不变 |
| 点击标题栏胶囊 | 回到 A 的同一个 editor session；不是重新从 Markdown `setContent` |
| A 活动时在 B 开始 Ramble | 不产生第二路录音；提供返回 A 或显式 handoff |
| handoff 成功/失败 | 成功先 settle/save A 再交给 B；失败仍由 A 持有，不出现半切换 |
| A 有待整理语音时切到 B 并编辑/点 Action/加附件 | 这些 B 内操作不触发或修改 A；A 自己的 3 秒停口计时仍可独立触发 cleanup |
| A Active 时 B 提交/批准/取消/删除 | 只 settle 并终结 B；A 继续录音，owner、editor 和 cleanup 不变 |
| A 隐藏时使用全局截图快捷键 | 插入 A 保留的最后合法光标；B 页面内普通附件按钮仍只修改 B |
| A 隐藏时打开 Ramble Console | Console 消费 A 的 Active Ramble snapshot，命令经 coordinator 路由；不另挂 TipTap，也不直接写 SQLite |
| 麦克风暂停或 cleanup 中 | 胶囊继续存在并显示 Ramble 主状态和准确子状态 |
| 立即提交 | 有界等待；超时用当前正文 |
| 录音或 cleanup 中主动重新载入 | 停止语音并有界 settle/save；旧 generation 失效；重新 hydrate 后不恢复 Undo 或补跑 cleanup |
| webview 意外 reload | 启动时停止孤儿语音；旧异步结果丢弃；已保存 Markdown 作为普通正文恢复 |
| 应用崩溃并重启 | 已保存 Markdown 恢复；Active Ramble、Undo 和未完成整理不恢复、不补跑 |

### 9.3 Native 人工验收

- macOS、Windows 各一次真实麦克风、截图批注、附件、Action 粘贴、继续说话、撤销整理、提交；
- macOS、Windows 各验证一次 A 录音 → 切 B 编辑 → 胶囊回 A，以及在 B 尝试开始第二个 Ramble；
- 验证 A 录音时 B 的批准/取消不会停止 A，并各执行一次录音中主动 reload；
- 5、10、20 分钟中文及中英混合 Ramble，观察内存、队列、首句和尾句；
- 断网、模型超时、麦克风断开、截图取消、存储写失败；
- 不打开 Task Brief 全屏 Dialog 也能完成主要 Ramble；
- Feedback Package 中 `feedback.md`、`uncooked.md`、manifest 和附件引用正确；无 Cooking
  时两份正文相同（可含 cleanup 结果）。

## 10. 范围

不再作为开工前悬空决策（已确认）：

- 一份正文，cleanup 等同用户整理，不单独持久化 raw；
- Uncooked 是未经 Cooking、由人类治理并最终确认的源反馈，不等于逐字转写；
- 草稿仍走 SQLite，不走浏览器缓存；
- 工作台全局最多一个 Active Ramble；它有唯一所属 Request，可以与当前可见 Request 不同；
- Active Ramble 所属 Feedback Draft Session 跨 Request 导航保活；标题栏胶囊可返回；
- Session host 最多保留所属会话与当前可见会话；不缓存全部历史 Request；
- 标题栏胶囊和 Ramble Console 只投影 Active Ramble、向 coordinator 发意图，不属于 Draft
  Session interface，也不挂 editor；
- 不做截图 reservation / 按发起时间排序；
- 整理触发：3 Stable / 500 字 / 停口约 3s / 所属 Request 或 Ramble 全局采集上的非语音插入 /
  停止 Ramble；切换可见 Request 不触发；
- 整理中锁段 + 禁用撤销；完成后 Undo 撤回覆盖；
- 主动重新载入会停止语音、settle/save 并失效旧 generation；意外 webview reload 会停止孤儿
  语音并丢弃旧异步结果，不恢复会话态；
- Action 序号切换当前归属频道，不插入指令原文，也不触发 cleanup。

不应扩大：

- 不重写反馈请求、反馈包、适配器或 continuation；
- 不把 RambleDesk 变成通用系统听写；
- 不引入 event-sourced 编辑器或 RambleJournal；
- 不持久化 TipTap History、selection、inflight cleanup task 或 Active Ramble；文档节点及属性完整恢复；
- 不支持多个 Request 同时 Active Ramble，也不做静默抢占；
- 不让模型决定事实顺序或自动提交；
- 不增加 Task Brief 录音通道；
- 不为未发布 PR 长期保留隐藏 marker；
- 不把 `what_happened` / context hint 做成第一版点击插入。

## 11. Replacement PR 与 #12 的生命周期

1. `f589290` 单独合进 main；
2. #12 暂时保持 Open，作为可运行对照和 donor；
3. 从最新 main 创建 `codex/continuous-content-flow`；
4. 新 PR 写明 `Supersedes #12`，列出保留、重写和删除；
5. 移植提交保留对 #12 及作者的来源说明；
6. north-star 与 package 验收后手动关闭 #12，关闭说明链接 replacement，写明“未合并但能力
   已选择性吸收”。

不要 force-push 原作者分支，也不要在 #12 上用大量 revert 改成另一套模型。

## 12. Definition of Done

- 语音 Stable 持续、立即进入 TipTap 草稿，不依赖停止录音；
- Light cleanup 在用户启用后按已确认时机自动触发，等于一次可撤销覆盖，不阻塞采集；
- 整理中锁段、禁用撤销；完成后可撤回；
- Uncooked 的术语合同明确为“未经 Cooking、由人类治理并确认”，不再与逐字 raw 混用；
- 截图完成后再插入光标；Action 粘贴引用块且不进 cleanup；
- 正文是唯一编辑面，Task Brief 只读；
- 全局最多一个 Active Ramble；所属 Request 与当前可见 Request 分离且 handoff 明确；
- 所属 Feedback Draft Session 在 Ramble 期间保持同一个 TipTap、selection、Undo、cleanup
  区间和 save queue；跨 Request 不走后台 Markdown merge；
- 标题栏胶囊与 Ramble Console 只消费 Active Ramble coordinator snapshot、发送意图并可返回
  所属 Request；它们不进入 Draft Session interface；最多挂载 owner + visible 两个 editor；
- 所属 Request 的提交、批准、取消、删除会返回并 settle；其他 Request 的终态动作不停止或
  锁住 Active Ramble；
- 主动 reload 和意外 webview reload 都有明确 teardown/reconcile；旧 generation 的 cleanup、
  附件和保存结果不能写入新 editor；
- UI 不再持有 clip/note/capture 事务系统；
- `rambledesk-capture://`、Ramble clip、Brief/Block Note 零生产残留；
- ADR、术语表、产品文档、代码、UI 文案和测试命名一致；
- 自动化门槛和 macOS/Windows native 场景通过；
- replacement PR 可独立审阅，不依赖阅读 #12 的 29 个提交。

最终目标不是“把 PR #12 做得更稳定”，而是把其中有价值的能力放回已经确立的产品模型：
**每个 Ramble 有且只有一个所属反馈请求、一份持续存活的 Feedback Draft 会话、一次可撤销的
轻度整理，最终发布一个不可变反馈包；工作台全局最多一个 Active Ramble。**
