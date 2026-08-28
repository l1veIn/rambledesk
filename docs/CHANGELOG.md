# 更新日志 / Changelog

每次发布前，请在本文件**顶部**为该版本新增一个条目，标题格式为 `## vX.Y.Z`。
发布管线（`.github/workflows/release.yml`）会在构建完成后自动把对应条目写入：

- GitHub Release 正文；
- 更新清单 `latest.json` 的 `notes` 字段（应用内"软件更新"弹窗显示的就是它）。

所以发布者只需要维护本文件，不需要手工复制文案。

编辑约定：

- 条目使用**纯文本**：更新弹窗以 `<pre>` 渲染，不解析 Markdown（避免 `**`、`-` 列表等符号原样显示）。
- **英文在前，中文摘要在后**，方便中英文用户。
- 未写条目的版本会自动回退到通用说明（管线不中断，但会打警告）。

---

## v0.3.3-rc.3

What's new in RambleDesk 0.3.3-rc.3

Draft architecture
- Restores the proven single-editor ownership model from v0.3.2 while keeping versioned TipTap JSON as the draft source of truth and Markdown as its readable projection.
- Active background Rambles now use semantic JSON operations, a serialized queue, and compare-and-swap saves; old Markdown-only and rc.1/rc.2 v1 drafts upgrade lazily.
- Actions are standard Blockquote containers. Reopening an Action creates a distinct group, repeated active clicks toggle it off, and stable ASR segment IDs prevent duplicate transcript/header insertion.

Post-processing and controls
- Tidy is manual-only and applies strict one-to-one labeled results in a single undoable editor transaction.
- Tidy and Cooking share one Post-processing page but keep separate providers, credentials, models, reasoning settings, and prompts.
- Record-style Ramble controls and configurable global Ramble/screenshot shortcuts are retained. Repeated commands are collapsed, shortcut reset no longer deadlocks, and failed rebinding restores the previous registration.

中文摘要
- 草稿架构恢复 v0.3.2 已验证的单 Editor 所有权，同时保留以版本化 TipTap JSON 为真源、Markdown 为可读投影的结构化能力；后台 Ramble 使用语义操作、串行队列与 CAS 保存，旧 Markdown 草稿和 rc.1/rc.2 v1 文档惰性升级。
- Action 改为标准 Blockquote 容器；重新打开会创建独立区块，再次点击活动 Action 会关闭选择；稳定 ASR 段 ID 防止重复写入语音和 Action 标题。
- Tidy 仅能手动触发，与 Cooking 在同一后处理页面使用两套独立配置；保留录音按钮与可配置全局快捷键，并修复重复命令、快捷键重置死锁和重绑回滚。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.2...v0.3.3-rc.3

## v0.3.2

What's new in RambleDesk 0.3.2

Speech recognition
- SenseVoice is now the recommended default model for reliable multilingual transcription. X-ASR remains available as the lower-priority streaming option.
- Existing rc.7 users who only inherited the old X-ASR default are migrated once to SenseVoice; an explicit later X-ASR selection is preserved.
- Settings and onboarding now show the same recommended model, ordering, descriptions, and download actions.

Ramble workflow
- /ramble [task] now consistently starts a task-scoped feedback loop across Pi, DeepSeek Harness, and Generic MCP hosts, while /ramble_on remains the explicit persistent-mode switch.
- A bare /ramble or a generic starter uses the active conversation when possible, otherwise gathering the goal, context, constraints, desired output, priorities, and completion criteria inside RambleDesk.
- Generic MCP and dsh now install one shared capability-aware skill, and onboarding uses natural English and Chinese starters so the agent begins in the user's language.

Feedback reliability
- Pi and DeepSeek Harness waits no longer disconnect because of Node/Undici's response-header timeout; interrupted flows keep the same durable request id for recovery.
- Generic MCP calls are now stateless, so a long human Ramble or stale transport session cannot hide an already-completed feedback request.
- Ramble guidance now explicitly distinguishes the durable request id from disposable MCP transport state and prevents duplicate replacement requests.

中文摘要
- 语音识别：SenseVoice 提升为推荐默认模型，X-ASR 保留为低优先级流式选项；rc.7 继承旧默认值的用户会一次性迁移到 SenseVoice，后续手动选择 X-ASR 不会被覆盖；设置与新手引导同步展示推荐状态和模型顺序。
- Ramble 工作流：Pi、DSH 与通用 MCP 的 /ramble [任务] 统一为任务级反馈循环，/ramble_on 专门开启持续模式；裸 /ramble 或通用开场语会优先利用当前任务，否则在 RambleDesk 内收集完整任务简报；通用 MCP 与 dsh 共用同一份能力自适应 skill，新手引导使用自然的中英文启动语以保持 Agent 回复语言。
- 反馈可靠性：Pi/DSH 的人工等待不再因 Node/Undici 响应头超时而断开；通用 MCP 改为无状态调用，长时间 Ramble 或陈旧 transport session 不再影响使用原 request_id 读取结果。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.1...v0.3.2

## v0.3.1

What's new in RambleDesk 0.3.1

UI
- Onboarding adapters step: Pi, DSH, and generic MCP hosts are now three equal rows with theme-aware logos and bottom-aligned install buttons.
- The onboarding dialog no longer closes on outside clicks or Escape; only "Set up later" and the finish button close it.
- The Ramble console start button now uses the play triangle icon, matching the task brief preview.
- The request list header shows a dynamic count pill next to "Requests".

Reliability
- Startup failures (a database created by a newer app version, an unwritable log directory, and similar cases) now show a clear dialog and exit cleanly instead of a silent crash.
- Opening a database created by a newer app version reports an actionable message instead of a generic migration error.

中文摘要
- 界面：新手引导适配器步骤改为三行同款卡片，图标随浅色/深色主题变色，安装按钮底部对齐；引导弹窗不再因点击外部或 Esc 意外关闭；"开始记录"按钮图标改为三角形；请求列表标题旁新增数量胶囊。
- 可靠性：启动失败不再闪退，改为弹窗说明原因；旧版本打开新版本创建的数据库时会提示安装最新版。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.0...v0.3.1

## v0.3.0

What's new in RambleDesk 0.3.0 (first 0.3.x stable release)

- Native adapters: Pi and DeepSeek Harness (DSH) adapters, plus generic MCP hosts (Antigravity support, SSE transport, subscriptions and multi-location skill injection).
- Workbench: archived session management, "last 24 hours" request filtering, local-path request attachments, and clickable links in task briefs and markdown previews.
- Updates: release notes are shown on launch after an update.
- Packaging: branded installers, the new web homepage, release-pipeline hardening, and stability fixes.

中文摘要
- 首个 0.3.x 稳定版：接入 Pi 与 DeepSeek Harness 原生适配器，以及通用 MCP 主机（支持 Antigravity、SSE 传输、订阅与多位置技能注入）。
- 工作台新增：会话归档管理、最近 24 小时过滤、本地路径附件、任务简报与预览中的可点击链接。
- 其他：启动时显示更新说明；安装包品牌化；发布管线加固与稳定性修复。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.0.2...v0.3.0
