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

## v0.5.0

What's new in RambleDesk 0.5.0

Human feedback workbenches
RambleDesk now offers ten workbench types: questions, free feedback, document review, web review, terminal trials, drag sorting, visual feedback, diff review, table review, and media review. New single-choice requests use a single question; existing single_choice requests retain their original contracts and results.

Review documents with paragraph comments and suggested wording; annotate images or a blank canvas; attach comments to diff lines, ranges, and hunks; suggest table values; or leave timestamp and range comments on saved audio and video. Source documents, diffs, and table values remain unchanged by review suggestions. Media retains the 20 MiB attachment limit and requires explicit playback.

Web review opens real pages and records element comments; cross-origin pages must load the review bridge and permit embedding. Terminal trials run a real PTY on the backend machine, support output quotes, and stop and drain before submission. These trial actions have their normal effects. Drag sorting records final order, labels, and deleted items.

Feedback text, answers, and comments share voice, paste, screenshot, attachment, and speech-tidying input where supported. Input stays with its original request and field, and drafts save text and structured state together. Expanded views retain the same request and draft; first-use guides explain controls without modifying feedback or starting trials.

Reliability and Agent continuation
Fix cancellation immediately after sending, including prompts not yet dispatched to the Agent, and prevent late cancellation from affecting the next turn. Permission cards retain exact tool identities and available command or file details; permissions still require the user's response.

Submitted feedback initializes or resumes its original ACP session in the background. Slow initialization does not block other sessions; startup failures remain visible for explicit retry. Stopping a managed session prevents its existing feedback queue from restarting the Agent within the current runtime; a new submission or explicit connection retry can resume delivery.

Fix table edits losing unsaved text on double-click, unsuccessful tools appearing as file changes, Windows UNC terminal directory handling, missing playground attachment names and path-alias validation, and the website demo getting stuck during submission. Strengthen regression checks for concurrent sends, continuation recovery, and workbench loading.

Workspace, development, and documentation
The native window fits the available monitor work area. Request context and materials stay above scrolling workbench content, navigation columns share aligned headers, and terminal views share four saved color presets. Table review uses a straight-edged workspace layout.

The workbench scaffold adds type-owned contracts, views, and playground materials; the playground covers ten types in thirteen scenarios. The bilingual README now explains human participation, the ten workbenches, and setup with localized WebP screenshots. Separate installation guides and interaction-design references document setup, related implementations, and research; the positioning map is a design illustration, not a token benchmark.

Upgrade and compatibility
Before upgrading from 0.4.0, close all instances and back up the database directory, attachment library, and application configuration together. Database extensions advance from 20 to 22 while retaining existing data. Version 0.4.0 refuses the upgraded database; reinstalling it is not a rollback. Restore the complete pre-upgrade backup to return to 0.4.0, and separately preserve any feedback created after upgrading. Do not delete migration records. Guidance: https://github.com/l1veIn/rambledesk/blob/v0.5.0/docs/DATA_COMPATIBILITY.md

Request summaries in what_happened are limited to 200 Unicode scalar values. Longer values return INVALID_ARGUMENT without truncation; put detailed materials in workbench data or attachments. Supported workbench contracts depend on the adapter. Generic MCP still requires returning to the Agent manually to continue.

Windows x64 packages include NSIS and MSI; NSIS provides the Tauri updater artifact and signature. Windows installers are not Authenticode-signed. Apple Silicon macOS provides an ad-hoc signed DMG without notarization or automatic updates. Installation: https://github.com/l1veIn/rambledesk/blob/v0.5.0/docs/INSTALLATION.md

Preview fixtures and automated checks do not establish native microphone or screen-capture permissions, physical-phone support, codec support on every platform, or native PTY compatibility on every OS. Platform and device acceptance must be assessed separately from the release build.

中文摘要
0.5.0 将反馈扩展为十种工作台：问答、自由反馈、文稿审阅、网页评审、终端试用、拖动排序、视觉反馈、差异评审、表格评审和媒体评审。新单选使用单题问答，旧 single_choice 请求保留原合同与结果。

文稿、图片、代码差异、表格和音视频都有对应的批注方式；原稿、固定 diff 和表格原值保持不变，修改意见作为建议交回 Agent。网页与终端提供真实体验操作；跨源网页需接入 bridge 并允许嵌入，终端运行在后端主机，提交前停止并收尾输出。音视频材料上限 20 MiB，不自动播放。

正文、答案和批注共用受支持的语音、粘贴、截图、附件与整理能力，输入固定到原请求和字段，正文与结构化状态共同保存。全屏页签共用草稿，首次使用引导不自动修改内容或启动试用。

修复发送后立即取消的竞态，迟到的取消不再影响下一轮；权限卡片补齐工具身份和可用详情，不自动授权。反馈在后台初始化或恢复原 ACP 会话，慢连接不阻塞其他会话，失败等待显式重试；当前运行实例中主动停止会话后，旧队列不会自行重启 Agent。

修复表格双击丢失未保存文本、失败工具被计入文件改动、Windows UNC 终端目录、脚手架附件名称与路径别名校验、官网演示提交状态卡住等问题，并加强并发发送、续接恢复及工作台加载回归。窗口适配显示器工作区，情况说明和材料固定在业务区上方，终端提供四种配色，表格工作区改用直角布局。

Playground 用十三个场景覆盖十种工作台。中英文 README 重新说明人参与任务的方式，配合对应语言的 WebP 界面截图、十种工作台清单、独立安装指南和交互研究资料；定位图表达设计意图，不是 Token 实测排名。

从 0.4.0 升级前退出所有实例，完整备份数据库目录、附件资料库和应用配置。数据库扩展从 20 升到 22；0.4.0 无法打开升级后的资料库，回退须恢复升级前完整备份，并另存升级后新增反馈，不删除迁移记录。详见上述数据兼容说明。

what_happened 摘要最多 200 个 Unicode 标量值，超限返回 INVALID_ARGUMENT，不自动截断；详细材料放入工作台 data 或附件。各适配器支持的工作台合同不同，通用 MCP 仍需手动回到 Agent 继续。

Windows x64 提供 NSIS 和 MSI，NSIS 提供 Tauri 更新产物与签名，尚无 Authenticode；Apple Silicon macOS 提供 ad-hoc 签名 DMG，尚未公证、不支持应用内更新。安装说明：https://github.com/l1veIn/rambledesk/blob/v0.5.0/docs/INSTALLATION.zh-CN.md

自动化与预览不能替代真实设备验收，尤其是麦克风与截图权限、实体手机、各平台媒体编解码及原生 PTY；构建通过不代表这些体验全部通过。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.4.0...v0.5.0

## v0.5.0-rc.1

RambleDesk 0.5.0-rc.1 — release candidate

Human feedback workbenches
- Ten production workbench types cover free feedback, questions, document review, web review, terminal trials, drag sorting, visual feedback, diff review, table review, and media review. Use a single question for new single-choice requests; existing single_choice requests keep their original contracts and results.
- Feedback text, custom answers, and review comments share voice, paste, screenshot, attachment, and speech-tidying input. Each input stays with the request and field where it started; drafts save text and structured state together.
- Document review keeps the source unchanged, supports continuing paragraph comments and suggested wording, and requires an explicit review decision. Paragraph removal is a reversible suggestion.
- Web review opens the actual page, captures element comments, and keeps the same page and draft when moving between ordinary and expanded tabs. Cross-origin development pages must load the review bridge and permit embedding.
- Terminal trials use a real PTY on the backend machine. Start or restart explicitly, retain up to 16 trials per request, quote selected output into feedback, and stop and drain the terminal before submission.
- Terminal appearance offers four color presets. Changes apply immediately, preserve the running trial, and share a saved preference across ordinary, expanded, and history views.
- Drag sorting supports keyboard and pointer reordering, label edits, deletion and restoration. Submitted results keep final labels, order, and deleted IDs; sorting stays in the ordinary view.
- Visual feedback supports freehand strokes, arrows, rectangles, and text on a saved image or blank canvas. Drafts retain editable annotations; submission saves a composed PNG and structured feedback.
- Diff review offers GitHub-style file navigation, unified and split views, inline comments, and expanded tabs including read-only history. Comments attach to old or new lines, line ranges, or hunks in an immutable file diff; file-viewed progress is local presentation state. Overall feedback shares the ordinary submission flow without imposing an approval workflow.
- Table review uses a spreadsheet grid with stable row and column identities, keyboard navigation, suggested cell values, and cell comments. The frozen source remains unchanged; submitted results preserve suggestions and comments separately.
- Media review uses an audio/video player, timeline points and ranges, and a comment sidebar. Commenting pauses playback; selecting a comment seeks to its timestamp. Saved media uses the existing 20 MiB attachment limit, with explicit load and codec errors and no automatic playback.
- Free feedback, document review, web review, terminal trials, visual feedback, diff review, table review, and media review offer five-step first-use guides and a Show guide entry. Guides explain controls without changing drafts, creating comments, starting a terminal, or playing media. Diff review's updated guide has its own new guide version.

Workspace and extension development
- The native window fits the available monitor work area at startup and supports smaller, high-DPI displays through the existing responsive layout.
- Request context and materials stay above the workbench's scrolling content. The feedback column shares one draft and submission flow with every type.
- Headers for request navigation, request details, and feedback share one height. Collapsed request navigation hides filters and counts while keeping request shortcuts.
- Expanded tabs are optional per type and disabled in the ordinary template. Types that enable them retain session navigation and the same request state.
- A workbench scaffold adds type-owned contracts, rules, fields, views, and playground materials through two shared registration files. The rating_review acceptance example is restricted to development builds.
- Optional guide metadata lives in each workbench definition. A shared guide engine reads it directly, preserving existing first-use records and avoiding a separate type list or guide file.

Agent feedback continuation
- Permission cards retain the exact tool identity and show its command or file details from the current session and turn when the Agent omits repeated context. Missing details remain explicit; no permission is granted automatically.
- Cancelling immediately after sending also cancels a prompt that has not reached the Agent yet; a late cancellation cannot target the next turn.
- Submitted feedback initializes or resumes its original ACP session without opening the Agent page. Slow initialization in one session does not hold delivery in another; startup failures remain visible and await explicit retry.
- Explicitly stopping a managed session prevents already-queued feedback from automatically restarting its Agent within the current runtime. A new submission or explicit connection retry can resume delivery. Pending permissions and other interactions still require the user's response.

Acceptance boundaries
- Request summaries in what_happened are limited to 200 Unicode scalar values; longer requests return INVALID_ARGUMENT without truncation. Put detailed materials in workbench data or attachments. See docs/PROTOCOL.md.
- Playground preparation validates required attachment names before creating a run. Visual, diff, sorting, and development rating fixtures now include file_name; image references retain the exact saved attachment name.
- The playground covers ten production types in thirteen scenarios, including blank-canvas drawing, image annotation, table review, and separate audio/video reviews, followed by a summary request. Existing runs retain their saved stages and request IDs. Preview fixtures and automated checks do not establish microphone, screenshot, physical-phone, codec support on every platform, or cross-platform PTY compatibility.
- This is a test release after 0.4.0, not the stable updater target. Windows ships an NSIS installer with a Tauri updater signature; Authenticode is not enabled. Apple Silicon macOS ships an ad-hoc signed DMG without notarization or automatic updates. Follow the README first-launch instructions.
- Back up the complete data set before upgrading. Database extensions advance from 20 to 22; 0.4.0 refuses the upgraded database. To roll back, close the app and restore the pre-upgrade backup; do not delete migration records. See docs/DATA_COMPATIBILITY.md.
- Fixed table edits losing unsaved text on double-click, failed or unfinished agent tools appearing as file changes, Windows UNC terminal working directories, generated playground attachment names, and the website demo becoming stuck while publishing.

中文摘要
- 请求的 what_happened 情况摘要限制为 200 个 Unicode 标量值；超限返回 INVALID_ARGUMENT，不会自动截断。详细材料放入工作台 data 或附件，详见 docs/PROTOCOL.md。
- 原生窗口启动时适配显示器可用区域，小屏和高 DPI 环境使用已有响应式布局。权限卡片保留精确工具 ID，并补齐当前会话、当前轮次对应的命令或文件详情；没有详情时明确提示，不自动批准。
- 修复消息刚发送就取消时取消指令丢失的竞态；尚未发给 Agent 的消息也会取消，迟到的取消不会影响下一轮。
- 新增十种正式工作台：自由反馈、逐项问答、文稿审阅、网页评审、终端试用、拖动排序、视觉反馈、差异评审、表格评审和媒体评审。新单选使用单题问答，旧 single_choice 请求保留原合同与结果。
- 正文、答案和批注共享语音、粘贴、截图、附件与语音整理；已开始的输入固定原请求和字段，正文与结构化状态共同保存。
- 文稿审阅保留原稿，支持批注续写、建议改写、可恢复删除线和显式整稿判断；网页评审保留真实网页、元素意见及普通/全屏页签往返状态，跨源开发页需接入 bridge 并允许嵌入。
- 终端运行在后端主机，主动开始或重启，每请求最多保存 16 轮，选中输出可引用到正文，提交前停止并收尾输出。排序支持拖动、键盘、名称编辑、删除与恢复，结果保留最终名称、顺序及删除 ID，普通视图无全屏入口。
- 终端新增四种配色预设，切换即时生效并保留正在运行的试用，普通、全屏和历史视图共用已保存的样式。请求页面三列表头高度统一，收起请求栏时隐藏筛选按钮和数量，保留请求快捷入口。
- 视觉反馈支持已有图片和空白画布上的简笔、箭头、矩形与文字，草稿保留可编辑标注，提交保存合成 PNG；差异评审将批注绑定到固定 diff 的修改前后行、行范围或改动块，具体流程由 Agent 编排。
- 网页评审、终端试用、视觉反馈和差异评审增加五步首次使用引导及手动重看入口，等待业务视图加载完成后出现，不自动修改草稿、创建标注或批注、启动终端或执行命令。
- 公共情况说明和材料固定在业务区上方；普通类型通过两份共享注册文件扩展，脚手架默认关闭全屏，rating_review 仅供开发验收。
- 可选引导在各工作台现有 definition 中声明，共享引导组件直接读取，保留首次使用记录，无需独立类型名单或引导文件。
- 提交反馈后由后台初始化或恢复原 ACP 会话，无需先打开 Agent 页面；慢连接不阻塞其他会话，启动失败保留原因并等待显式重试。当前运行实例中显式停止托管会话后旧队列不自行重启，新提交或显式连接可恢复投递，权限等交互仍由用户作答。
- Playground 准备流程在创建轮次前验证附件必填名称；补齐视觉反馈、差异评审、排序及开发评分样例的 file_name，图片引用与附件保存名精确匹配。目录别名按真实路径校验，避免 macOS 临时目录中的合法附件被误判越界。
- 表格评审保留原始值，分别记录单元格建议与批注；媒体评审支持音视频的时间点和区间意见，主动播放，评论时暂停，材料上限 20 MiB。
- Playground 用十三个场景覆盖十种正式类型，分别体验空白画布、图片批注、表格及音频/视频，再以汇总请求收尾；自动化与预览不能替代真实设备验收。
- 本版本为候选测试版，不替换稳定更新目标。Windows 提供 NSIS 与 Tauri updater 签名，尚无 Authenticode；Apple Silicon DMG 使用 ad-hoc 签名，尚未公证、不支持应用内更新，首次打开按 README 操作。
- 升级前备份完整资料。数据库扩展由 20 升到 22，0.4.0 会拒绝打开新数据库；回退必须退出应用并恢复升级前备份，不删除迁移记录。详见 docs/DATA_COMPATIBILITY.md。
- 修复表格双击丢失未保存文本、失败或未完成工具被统计为文件改动、Windows UNC 终端目录、脚手架附件名称缺失及官网演示发布状态卡住。

## v0.4.0

What's new in RambleDesk 0.4.0

ACP Agent sessions
- Start and resume Agent conversations inside RambleDesk through a shared ACP layer: connection, models, modes, tool activity, permissions, and supported user questions.
- Settings and onboarding share one Agents page. One-click connection installs the needed bridge and uses the detected ACP entry while keeping account settings.
- New sessions continue from onboarding into the selected Agent. Model options load in the actual project; failures give targeted guidance instead of a generic retry.
- Managed Agents use the built-in feedback command. Submitted feedback is stored durably and queued for continuation in the original session.
- New setups use one DeepSeek (DSH) entry with the managed deepseek-acp bridge. Existing DeepSeek Harness configurations remain available as custom configurations.

Ramble handoff
- Watching an Agent session or an already-open Ramble page opens the latest request for that conversation as soon as it arrives, and the request list shows it immediately.
- Existing sessions keep the composer collapsed until you need it; new-session drafts keep a full composer. Unsaved Ramble drafts still block automatic switching.
- Submitting feedback always sends a continuation for that request. Delivery succeeds when that continuation is sent to the agent, not when the turn later ends. An earlier unknown delivery stays on that request until you retry or mark it delivered; it does not hold later submissions.
- A turn that ends without a feedback handoff gets one bounded reminder. The built-in feedback command uses a private local IPC channel.

Workspace
- Sessions, archives, settings, and task briefs open as workspace tabs. Tabs share the available width, shrink to a minimum, then scroll; middle-click closes a tab without starting browser autoscroll.
- The task brief renders Markdown in "What happened". On a wide layout, "What happened" and the actions sit in equal columns.
- New Appearance settings add theme colors, interface scaling, fonts, and workspace backgrounds. The selected palette also applies to the speech overlay and ramble console.

Phone and Web Access
- On a phone, both rails become overlay drawers, the titlebar keeps a navigation button, and the request list opens from a floating button.
- Web Access has a settings section for the browser server: port, autostart, and token rotation. Browser sessions resume from a stored cookie.

Agent setup
- Settings can clear unused Agent configurations and detect them again, as on a fresh install. Configurations still used by sessions stay and cannot be deleted.

Release notes
- This stable release includes Windows x64 NSIS and MSI installers and an Apple Silicon macOS DMG. Windows updater artifacts are signed, but Windows Authenticode signing and Apple notarization are not yet enabled.
- For SmartScreen and Gatekeeper first-launch instructions, see https://github.com/l1veIn/rambledesk/blob/v0.4.0/README.md
- Automated checks do not replace real-model feedback-loop and clean-install acceptance testing.

中文摘要
- 新增应用内 ACP 智能体会话：统一连接、模型与模式、工具活动、授权和受支持的用户问答；引导和设置共用 Agents 页，一键连接安装组件并采用检测到的 ACP 入口。
- 托管智能体通过内置反馈命令完成 Ramble 流程；新接入 DeepSeek 统一为 DeepSeek (DSH)，旧 Harness 配置仍可作为自定义配置保留。
- 正在查看该 Agent 会话或已打开的 Ramble 页时，新请求会立刻打开最新一条，并马上出现在请求列表中；已有会话的输入框默认收起，未保存的 Ramble 草稿仍会挡住自动切换。
- 提交反馈就会为该请求发送续接；投递成功以续接消息已发给 Agent 为准，不要求这一轮正常结束。更早一条投递状态未知时，只留在那一条上处理，不会卡住后面新提交的反馈。
- 工作区标签先均分宽度再滚动，中键关闭标签时不再触发浏览器自动滚动；任务简报的「发生了什么」按 Markdown 渲染，宽屏下与「需要体验」等宽；外观配色同时作用于语音悬浮窗和 Ramble 控制台。
- 手机端侧栏改为抽屉；Web Access 可设置端口、自动启动和令牌，浏览器会话用 Cookie 恢复。
- 设置里可清除未使用的 Agent 配置并重新检测，相当于清空后重新安装；仍被会话使用的配置不会删除。
- 本版为 0.4.0 正式版，提供 Windows x64 NSIS/MSI 与 Apple Silicon macOS 安装包；尚无 Windows Authenticode 签名及 Apple 公证，首次启动步骤见上述 README。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.4...v0.4.0

## v0.4.0-rc.3

What's new in RambleDesk 0.4.0-rc.3

Phone and browser access
- The workbench now adapts to phone screens: both rails become overlay drawers, the titlebar keeps a navigation button, and the request list opens from a floating button.
- Web Access gained a dedicated settings section for the browser server: port, autostart, and token rotation. Browser sessions resume from a stored cookie instead of asking for the token again on every visit.
- Agent session options on a phone collapse into a single entry that shows the model name; tapping it opens the full list of model, reasoning effort, and access options instead of crowding the composer.

Workspace tabs
- The tab strip now behaves the same at every width: tabs share the available width evenly, shrink down to a minimum, and only then scroll horizontally.
- On desktop the mouse wheel scrolls the strip, both ends fade to show there is more content, and a newly opened tab is brought into view at the end of the queue.

Task brief
- The "What happened" section renders Markdown, so lists, code blocks, links and emphasis written by the Agent display as intended. Single line breaks are preserved.

Adapter setup
- The adapter starter prompt is now "/ramble Lets work on something together", matching the /ramble command that starts a feedback request.

Under the hood
- Workbench state ownership was reorganised: the open request, draft, attachments, Ramble and voice state, startup, workspace navigation, cooking, shell layout and preview fixtures each have a single owning module, and App.svelte is now a composition root.
- Preview fixtures are served through an Application Transport implementation, so preview mode no longer branches inside workbench logic.
- A frontend dependency-direction check and module-size gate keep the new boundaries from regressing; frontend coverage grew past 1200 tests, including component tests for the capture overlay, the Ramble controller and the agent composer options.

Release candidate notes
- This test release includes Windows x64 NSIS and Apple Silicon macOS installers. Windows updater artifacts are signed, but Windows Authenticode signing and Apple notarization are not yet enabled.
- For SmartScreen and Gatekeeper first-launch instructions, see https://github.com/l1veIn/rambledesk/blob/v0.4.0-rc.3/README.md
- Automated checks do not replace real-model feedback-loop and clean-install acceptance testing.

中文摘要
- 工作台适配手机屏幕：两条侧栏改为抽屉，标题栏保留导航按钮，请求列表用左下角浮动按钮打开。
- Web Access 新增浏览器服务器设置（端口、自动启动、刷新令牌），浏览器会话通过 Cookie 恢复，不再每次访问都要求输入令牌。
- 手机上 Agent 会话选项收进一个入口，只显示模型名；点开后是完整的模型、思考强度和访问权限列表，不再挤占输入框。
- 工作区标签在任何宽度下行为一致：先均分宽度收缩到最小值，再横向滚动；桌面端可用滚轮滚动，两端有渐隐提示，新打开的标签会自动滚动到队尾。
- 任务简报的「发生了什么」按 Markdown 渲染，列表、代码块、链接和强调都能正常显示，单换行也会保留。
- 适配器示例提示词改为 "/ramble Lets work on something together"。
- 内部：工作台状态改为每个领域单一所有者，App.svelte 收敛为组合根；预览数据改由 Application Transport 提供；新增前端依赖方向与模块大小门禁，前端测试超过 1200 个。
- 本版为候选测试版，提供 Windows x64 NSIS 和 Apple Silicon macOS 安装包；尚无 Windows Authenticode 签名及 Apple 公证，首次启动步骤见上述 README。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.4.0-rc.2...v0.4.0-rc.3

## v0.4.0-rc.2

What's new in RambleDesk 0.4.0-rc.2

ACP feedback reliability
- Managed ACP Agents receive shared Ramble workflow guidance on every turn. A turn that ends without a feedback handoff gets one bounded reminder; missing or failed handoffs are reported instead of silently ending the workflow.
- The built-in feedback command uses a private local IPC channel, keeping feedback credentials out of Agent environments and working with bridges that filter token variables. Turn receipts are isolated so an older request cannot satisfy a newer turn.
- Explicit feedback skip reasons and cancellation handling prevent automatic reminder loops and unwanted continuation after cancellation.

DeepSeek setup
- New setups use one DeepSeek (DSH) entry with the managed deepseek-acp 0.8.0 bridge. A separate global dsh installation is not required for this path.
- Existing DeepSeek Harness configurations and sessions remain available as custom configurations; their launch settings and history are preserved.
- Agent discovery labels distinguish supported Agents from programs actually found on the device, and connection checks no longer imply that model-driven feedback has been verified.

Agent-to-Ramble navigation
- New feedback requests automatically open from an Agent conversation when its composer is empty. This applies across Agent providers and on later turns, not just the first message.
- Unsent text, active IME composition, and other workspace pages are not interrupted. Refreshing old requests or returning to an Agent tab does not replay an automatic jump; user navigation takes precedence.

Release candidate notes
- This test release includes Windows x64 NSIS and Apple Silicon macOS installers. Windows updater artifacts are signed, but Windows Authenticode signing and Apple notarization are not yet enabled.
- For SmartScreen and Gatekeeper first-launch instructions, see https://github.com/l1veIn/rambledesk/blob/v0.4.0-rc.2/README.md
- Automated checks do not replace real-model feedback-loop and clean-install acceptance testing.

中文摘要
- 修复 ACP 首轮及后续轮次未进入 Ramble 的问题：统一逐轮注入反馈流程说明，缺少交接时最多提醒一次，仍失败则明确报错；取消后不继续补发。
- 内置反馈命令改用私有本地 IPC 通道，不再依赖 Agent 环境中的反馈令牌，兼容会过滤 TOKEN 变量的桥接程序；反馈回执按轮次隔离。
- DeepSeek 新接入统一为 DeepSeek (DSH)，托管安装 deepseek-acp 0.8.0，无需额外全局安装 dsh；保留旧 Harness 自定义配置和会话历史。
- 新 Ramble 请求到来时，仅在 Agent 会话页且输入框为空时自动打开；保护未发送文字和中文输入法组合输入，其他页面不动，旧请求刷新不重复跳转。
- 本版为候选测试版，提供 Windows x64 NSIS 和 Apple Silicon macOS 安装包；尚无 Windows Authenticode 签名及 Apple 公证，首次启动步骤见上述 README。真实模型闭环和干净安装仍需实机验收。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.4.0-rc.1...v0.4.0-rc.2

## v0.4.0-rc.1

What's new in RambleDesk 0.4.0-rc.1

ACP Agent sessions
- Start and resume Agent conversations inside RambleDesk, with a shared ACP layer for connection capabilities, model and mode selection, tool activity, permissions, and supported user questions.
- Onboarding and Settings share a simpler Agents page with one ACP connection card and advanced settings. One-click connection installs the bridge and uses the detected ACP entry while keeping account settings.
- Continue from onboarding into the selected Agent's first session. Model options load in the actual project; session, configuration and message failures provide targeted guidance. Only confirmed authentication failures prompt sign-in or API-key setup.
- Message acknowledgement recovery preserves drafts and avoids resending accepted messages; explicitly failed messages can be returned to the composer for review and retry.
- Managed Agents use the built-in feedback command. Submitted feedback is stored durably and queued for continuation in the original Agent session; interrupted deliveries expose recovery actions.
- External adapters remain available for users who run their Agent independently of RambleDesk's managed ACP client.

Conversation and workspace
- Separate Agent and Ramble views, compact tool-call rows with expandable scrollable details, paged history, supported attachments, and live session configuration and context usage.
- Resizable session and request columns remember their widths and collapse at their minimum size. Larger desktop minimum dimensions preserve workspace room, and sidebar scrolling keeps Settings accessible.
- Sessions retain creation-time order within projects, with pinned sessions first. New sessions highlight the required project selection.
- New Appearance settings add theme colors, interface scaling, fonts, and workspace backgrounds alongside light and dark modes.

Speech and diagnostics
- Review speech before writing with confirm, cancel, Tidy, and inline edit actions. Optional automatic Tidy includes guidance about the existing automatic Tidy threshold.
- Automatic preview of waiting requests is off by default. Diagnostics can be cleared and recording can be disabled.

Release candidate notes
- Fix the macOS release build and prevent normally exited ACP processes from being reported as connection failures during cleanup.
- Interrupted continuation turns can remain in an unknown delivery state and block later feedback until reviewed. Confirm an already-read delivery or retry it from its status details.
- Scanning and component-only repairs preserve custom launch profiles. One-click connection adopts the detected ACP entry; advanced settings remain available for custom paths and environments.
- This prerelease includes Windows x64 NSIS and Apple Silicon macOS installers. Windows MSI is reserved for stable releases.

中文摘要
- 新增应用内 ACP 智能体会话，统一连接、模型与模式选择、工具活动、授权及受支持的用户问答；引导和设置共用单张 ACP 连接卡片与高级设置，一键连接安装组件并采用正确入口，同时保留账号配置。
- 引导直接进入所选智能体的新会话，选好项目后加载真实模型选项；会话准备、配置与消息错误分别提供就地处理，只有明确认证失败才提示登录或配置 API Key。发送确认丢失时先核实是否已接收，避免重复消息；明确失败的消息可放回输入框后重试。
- 托管智能体通过内置反馈命令完成 Ramble 流程，反馈持久保存后排队续接原会话；外部适配器继续支持独立运行的智能体，登录及 API Key 配置由智能体自身处理。
- 会话界面更简洁：工具调用按行收起，展开详情限制高度并可滚动，支持历史分页、附件和实时配置；侧边栏及请求列可拖动、收起并记住宽度，会话按创建时间排序，新会话突出工作目录必选提示。
- 新增外观页，提供主题配色、窗口缩放、字体与工作区背景；语音确认悬浮窗增加整理、编辑及可选自动整理，自动预览默认关闭，诊断记录可清除及停用。
- 已知限制：中断后送达状态未知的旧反馈需要先确认或重试，才会继续投递后续反馈；重新检测与单独修复组件保留自定义配置，一键连接采用检测到的 ACP 入口，特殊路径和环境可在高级设置中调整。
- 本候选版提供 Windows x64 NSIS 与 Apple Silicon macOS 安装包。
- 修复 macOS 发布构建，以及 ACP 程序正常退出后被清理过程误报为连接失败的问题。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.4...v0.4.0-rc.1

## v0.3.4

What's new in RambleDesk 0.3.4

Web Access
- New authenticated local Web Client: the shared Workbench client is served over HTTP from the Web Access Server with first-visit token authentication and a browser bootstrap, so a local browser can open workspaces and submit text feedback.
- The Workbench application API — workspace read and list, draft mutation, submit and cancel, attachment streaming — runs over authenticated HTTP through a single ApplicationTransport contract, with conformance tests shared by the Tauri and HTTP transports.
- Browser sessions stay current via realtime invalidation: authenticated WebSocket events, health probing, and snapshot refetch after reconnect.
- Capability boundaries are explicit per runtime: native operations sit behind Tauri capability implementations, browser implementations report unavailable states instead of degrading silently, and capability manifests are scanned consistently across platforms.
- Browser workflows add image paste/upload attachments (client uploads separated from server workspace selection), local edge speech recognition with sherpa Wasm, and the completed edge capture plugin boundary with attachment candidates.
- Web Access lifecycle and support boundaries are productized: live runtime health with actionable failures is reported, security and lifecycle acceptance is complete, and the support matrix is published.

Workspace tabs and navigation
- Sessions, archives, settings, task briefs, and profiles now open as browser-style draggable workspace tabs with keyboard navigation and overflow handling; superseded full-screen and modal flows are removed, and tab snapshots restore safely after relaunch.
- The request list is collapsible and filterable with preserved task titles, and automatic task brief previews can be disabled in General settings (on by default).
- The titlebar is compacted with a clearer color hierarchy: the status capsule moved to the far right, drag and double-click maximize gestures cover the whole bar, and the inactive Reload button was removed.

Feedback, speech, and the task brief preview
- The task brief preview tab gains one primary action on the far left of its footer — Submit feedback, or Cook and submit when Cooking is enabled — at the opposite end from the Ramble control and reusing the session submission pipeline.
- Empty and whitespace-only replies are now blocked at the publish gate with a clear notice, in the session and preview views alike; completed or cancelled requests are rejected before any side effect.
- The speech overlay is draggable, shows live voice levels, can confirm before writing, has configurable visibility and opacity, and confirms or discards pending speech via global shortcuts. The opacity slider now fades only the frosted glass layer while the text stays crisp, with a new default of 95.

中文摘要
- 新增带鉴权的本地 Web Client：Workbench 客户端由 Web Access Server 经 HTTP 提供，配合首次访问令牌鉴权与浏览器引导，本地浏览器即可打开工作区并提交文本反馈；完整应用 API 通过统一 ApplicationTransport 契约暴露到 HTTP，Tauri 与 HTTP 传输共享一致性测试。
- 浏览器会话经鉴权 WebSocket 实时失效、健康探测与重连后快照重取保持同步；能力边界按运行时显式声明，浏览器对不可用能力明确报告而非静默降级。
- 浏览器支持图片粘贴/上传附件（客户端上传与服务端选路分离）、边缘本地语音识别（sherpa Wasm）与完整的边缘捕获插件边界；Web Access 生命周期、安全验收与支持矩阵完成，并实时上报运行时健康。
- 会话、归档、设置、任务简报与资料页统一为浏览器式可拖拽工作区标签页，支持键盘切换与溢出管理，废弃的全屏/模态流程移除，标签页快照重启后可安全恢复。
- 请求列表可折叠并支持筛选；自动任务简报预览可在 General 设置中关闭（默认开启）；标题栏更紧凑、配色层级更清晰，状态胶囊移到最右侧，拖拽与双击最大化覆盖整条标题栏。
- 任务简报预览页底部最左侧新增单一主操作——提交反馈（启用 Cooking 时为 Cook 并提交），与右侧 Ramble 控制相隔最远、避免误触，复用会话视图的提交管线；空回复与纯空白回复在提交入口即被拦截并提示，已完成/已取消请求在任何副作用前被拒绝。
- 语音悬浮窗可拖动、实时显示电平、写入前可确认，可见性与不透明度可配置，并支持全局确认/丢弃快捷键；不透明度滑块现在只淡化磨砂玻璃层、文字保持清晰，默认不透明度为 95。
- 任务简报中的 Action 组仅在请求可操作时可点击；设置导航移除重复的适配器配置脚注。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.3...v0.3.4

## v0.3.3

What's new in RambleDesk 0.3.3

Draft architecture
- Restores the proven single-editor ownership model from v0.3.2 while keeping versioned TipTap JSON as the draft source of truth and Markdown as its readable projection.
- Active background Rambles now use semantic JSON operations, a serialized queue, and compare-and-swap saves; old Markdown-only and rc.1/rc.2 v1 drafts upgrade lazily.
- Actions are standard Blockquote containers. Reopening an Action creates a distinct group, repeated active clicks toggle it off, and stable ASR segment IDs prevent duplicate transcript/header insertion.

Post-processing and controls
- Tidy is manual-only and applies strict one-to-one labeled results in a single undoable editor transaction.
- Tidy and Cooking share one Post-processing page but keep separate providers, credentials, models, reasoning settings, and prompts.
- Pending speech can be shown in italics or left visually neutral; only segments in the active Tidy run receive a transient inline spinner. Action Groups use padded blue-tinted text blocks without a left accent bar.
- Record-style Ramble controls and configurable global Ramble/screenshot shortcuts are retained. Repeated commands are collapsed, shortcut reset no longer deadlocks, and failed rebinding restores the previous registration.

Capture and storage reliability
- Screen capture keeps RambleDesk windows out of the captured image and preserves macOS window exclusion while restoring the windows that were visible before capture.
- Windows storage migration normalizes ordinary and verbatim path forms and rejects overlapping destinations before and during traversal, preventing recursive self-copying. Thanks to BingForest for reporting the issue.

中文摘要
- 草稿架构恢复 v0.3.2 已验证的单 Editor 所有权，同时保留以版本化 TipTap JSON 为真源、Markdown 为可读投影的结构化能力；后台 Ramble 使用语义操作、串行队列与 CAS 保存，旧 Markdown 草稿和 rc.1/rc.2 v1 文档惰性升级。
- Action 改为标准 Blockquote 容器；重新打开会创建独立区块，再次点击活动 Action 会关闭选择；稳定 ASR 段 ID 防止重复写入语音和 Action 标题。
- Tidy 仅能手动触发，与 Cooking 在同一后处理页面使用两套独立配置；保留录音按钮与可配置全局快捷键，并修复重复命令、快捷键重置死锁和重绑回滚。
- 待整理语音可选择以斜体区分或完全不区分；只有本轮正在整理的段落显示临时行首转圈。Action Group 改为无左侧强调线的淡蓝色独立文本区块。
- 截图流程会排除 RambleDesk 窗口，并在结束后恢复截图前可见的窗口；macOS 窗口排除保持有效。
- Windows 数据位置迁移会统一普通路径与逐字路径格式，并在扫描和复制期间持续拒绝重叠目标，避免递归复制自身。感谢 BingForest 报告此问题。

Full changelog: https://github.com/l1veIn/rambledesk/compare/v0.3.2...v0.3.3

## v0.3.3-rc.3

What's new in RambleDesk 0.3.3-rc.3

Draft architecture
- Restores the proven single-editor ownership model from v0.3.2 while keeping versioned TipTap JSON as the draft source of truth and Markdown as its readable projection.
- Active background Rambles now use semantic JSON operations, a serialized queue, and compare-and-swap saves; old Markdown-only and rc.1/rc.2 v1 drafts upgrade lazily.
- Actions are standard Blockquote containers. Reopening an Action creates a distinct group, repeated active clicks toggle it off, and stable ASR segment IDs prevent duplicate transcript/header insertion.

Post-processing and controls
- Tidy is manual-only and applies strict one-to-one labeled results in a single undoable editor transaction.
- Tidy and Cooking share one Post-processing page but keep separate providers, credentials, models, reasoning settings, and prompts.
- Pending speech can be shown in italics or left visually neutral; only segments in the active Tidy run receive a transient inline spinner. Action Groups use padded blue-tinted text blocks without a left accent bar.
- Record-style Ramble controls and configurable global Ramble/screenshot shortcuts are retained. Repeated commands are collapsed, shortcut reset no longer deadlocks, and failed rebinding restores the previous registration.

Capture and storage reliability
- Screen capture keeps RambleDesk windows out of the captured image and preserves macOS window exclusion while restoring the windows that were visible before capture.
- Windows storage migration normalizes ordinary and verbatim path forms and rejects overlapping destinations before and during traversal, preventing recursive self-copying. Thanks to BingForest for reporting the issue.

中文摘要
- 草稿架构恢复 v0.3.2 已验证的单 Editor 所有权，同时保留以版本化 TipTap JSON 为真源、Markdown 为可读投影的结构化能力；后台 Ramble 使用语义操作、串行队列与 CAS 保存，旧 Markdown 草稿和 rc.1/rc.2 v1 文档惰性升级。
- Action 改为标准 Blockquote 容器；重新打开会创建独立区块，再次点击活动 Action 会关闭选择；稳定 ASR 段 ID 防止重复写入语音和 Action 标题。
- Tidy 仅能手动触发，与 Cooking 在同一后处理页面使用两套独立配置；保留录音按钮与可配置全局快捷键，并修复重复命令、快捷键重置死锁和重绑回滚。
- 待整理语音可选择以斜体区分或完全不区分；只有本轮正在整理的段落显示临时行首转圈。Action Group 改为无左侧强调线的淡蓝色独立文本区块。
- 截图流程会排除 RambleDesk 窗口，并在结束后恢复截图前可见的窗口；macOS 窗口排除保持有效。
- Windows 数据位置迁移会统一普通路径与逐字路径格式，并在扫描和复制期间持续拒绝重叠目标，避免递归复制自身。感谢 BingForest 报告此问题。

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
