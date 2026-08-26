# ADR 004：一份 Feedback Draft、全局唯一 Active Ramble、可撤销 Light cleanup

- 状态：Accepted
- 日期：2026-08-26
- 取代：ADR 003 中「切换 Request、重新载入或提交前必须先停止当前 Ramble」
- 补充：ADR 002 的增量入正文；ADR 003 的统一采集状态与文档流映射

## 上下文

语音 Ramble 必须在录音过程中把 Stable 文本写入正文（ADR 002），并且 Ramble 高于单个
输入工具（ADR 003）。实践中出现过两类错误形状：

1. 把每次开始/停止麦克风做成 clip/note 对象，再同步回 Markdown。这会引入隐藏 marker、
   第二编辑面和捕获事务锁。
2. 工作台用同一个 TipTap `setContent` 切换 Request，导致所属草稿的 selection、Undo
   和整理中区间在导航时丢失；随后的 Stable/cleanup 绕过 editor 直接 merge SQLite。

同时，可选的轻度转写整理若被当成第二份原始证据，会把 Draft、Uncooked 和模型输出拆成
多套状态。产品需要的是一份可编辑正文，以及跨 Request 导航时仍然活着的那一次 Ramble。

## 决策

### 1. 正文就是 Feedback Draft

工作台里可编辑的正文只有 TipTap 中的 Feedback Draft。Task Brief 只读。点击 Action
序号等于把该 Action 原文粘贴进正文，不是第二反馈通道。

### 2. Light cleanup 是一次可撤销覆盖

用户显式启用后，系统对待整理语音做轻度整理，语义等于选中这段再粘贴覆盖。成功后不
单独保存覆盖前文本；失败或超时则原文不动。整理中锁段并暂时禁用 Undo/Redo；完成后
Undo 撤回这次覆盖。默认关闭。它不是 Cooking。

`Uncooked` 表示未经 Cooking、由人类治理并在提交时确认的源正文，不表示逐字转写。

### 3. 全局最多一个 Active Ramble

原生层仍然只有一个 `SpeechSession`。Active Ramble 有唯一所属反馈请求；该请求可以
不是当前可见 Request。语音、Ramble 全局截图和自动剪贴板只进入所属 Feedback Draft。
在另一个 Request 上开始 Ramble 必须显式 handoff，不得静默抢占。

切换可见 Request 不停止 Ramble。所属 Feedback Draft Session（TipTap、selection、
Undo、待整理/整理中、save queue）保活。工作台最多同时挂载所属会话与当前可见会话。
标题栏胶囊和 Ramble Console 是同一所属会话的投影和控制面，不另挂编辑器。

所属 Request 的提交、批准、取消、删除必须先回到该会话再有界 settle。其他 Request
的终态不影响这次 Active Ramble。停止麦克风只改变输入子状态，不释放 Draft 会话。

### 4. 重新载入不是保活导航

用户主动重新载入 Workspace 时：停止原生语音、有界 settle/save、使 session
generation 失效并 dispose，再从 SQLite hydrate。意外 webview reload 后与原生层
reconcile，停止孤儿 `SpeechSession`，丢弃旧异步结果。崩溃重启只恢复已保存 Markdown，
不恢复 Undo、cleanup 或 Active Ramble。

### 5. 持久化合同不变

Draft 仍是整篇 `body_markdown` + CAS revision，存 SQLite。不引入 clip、journal、
raw sidecar 或隐藏 Markdown 身份。语音追加和 cleanup 必须走 editor transaction，
不得在所属 Session 存活时直接 merge SQLite。

音频 warm-up 队列允许大于 ADR 002 所写的 512，以覆盖 recognizer 后台加载窗口，但仍
必须有界，并在 20 分钟录音中保持内存稳定。

## 后果

正向：

- 增量入正文和统一 Ramble 得以保持，同时允许对照其他 Request 时继续采集；
- Light cleanup 不再分裂证据模型；
- 复杂度留在 request-scoped editor session，而不是对象图。

代价：

- 必须同时挂载最多两个 editor，并维护 session generation；
- 整理中暂时禁用整篇 Undo；
- 导航保活不承诺崩溃恢复。
