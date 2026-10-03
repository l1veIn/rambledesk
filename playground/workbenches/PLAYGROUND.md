# 完整工作台测试执行规程

面向在本目录工作的 Agent。用户发出“读取 PLAYGROUND.md，开始完整工作台测试”即授权执行下面的人工体验流程；用户操作工作台，你负责材料、续接、结果检查与报告。

## 1. 能力与现有进度

1. 读取本目录 README.md。测试期间只读固定材料，只在 .runs/ 写运行产物。不要修改父仓库代码、修复应用、安装依赖或替用户操作工作台。
2. 使用本会话注入的 RAMBLEDESK_COMMAND 命令。不要读取、打印、保存或改写私有 RAMBLEDESK_FEEDBACK_* 变量，不拼接 HTTP endpoint，不使用其他会话的 MCP/Pi/dsh 通道。
3. 调用 feedback list-workbenches --limit 20，按 next_offset 读完目录；完整新轮确认 ramble、questions、document_review、web_review、terminal、sort、visual_feedback、diff_review 的 v1 均可用；只测网页时确认 web_review 与汇总所用的 ramble，只测终端时确认 terminal 与 ramble。分别调用 feedback describe-workbench --type <type> --version 1，核对 fixtures 中的合同。恢复旧轮只要求其持久化阶段与汇总需要的类型，不因新增阶段阻止旧轮恢复。不要把整个目录或凭据写入报告。
4. 缺少命令、会话能力或所需类型时，直接说明具体缺项和需要重开/更新的应用版本；不要用自由正文替代类型、伪造成功或悄悄跳过。若目录多于这八种类型，报告本套材料实际覆盖的类型及额外未覆盖项，不能声称全覆盖。
5. 检查 .runs/latest.json 及对应 run.json。若已有本轮记录，先按记录中的 request_id 用 feedback get / recover 取真实状态；不能仅凭本地阶段判断已提交。用户说“继续”但没有记录时，说明没有可恢复的本轮记录，不擅自启动新轮。
6. 非终态请求只恢复原请求并结束当前回合，不能新建、轮询、sleep 或阻塞等待。若待恢复的是网页阶段，先按下文恢复原 URL 的样例服务，再交还原请求；先根据真实请求状态记下它已发送，不能把本地 prepared 当作改写已发 URL 的授权。取消即中止本轮。用户明确要求新一轮时，必须先确认原请求已终态；不能自动取消原请求。跨会话无法恢复时回原会话继续，不为同一轮另开 ID。

PowerShell 示例（尖括号路径/ID须替换为实际值）：

~~~powershell
& $env:RAMBLEDESK_COMMAND feedback list-workbenches --limit 20
& $env:RAMBLEDESK_COMMAND feedback describe-workbench --type document_review --version 1
& $env:RAMBLEDESK_COMMAND feedback request --input '<绝对路径>/01-ramble.json'
& $env:RAMBLEDESK_COMMAND feedback get --request-id '<原请求 UUID>'
& $env:RAMBLEDESK_COMMAND feedback recover --request-id '<原请求 UUID>'
~~~

Bash/sh 使用 "$RAMBLEDESK_COMMAND" feedback ...，保持参数与 JSON 文件独立，不把 JSON 内联拼成命令。以应用实际 --help 为准。

## 2. 准备一轮

在没有待处理旧轮、或旧轮已完成/已取消后执行：

~~~sh
node prepare.mjs new
~~~

脚本只生成本地文件，不发请求。它生成独立 UUID、将附件路径变成绝对路径，按实际 stages 为完整新轮的请求标题生成 `x/N` 编号，写出 .runs/<run-id>/run.json、各阶段请求文件，并更新 latest.json。当前默认有十个场景；新增类型由 fixture 自动发现，不改其他材料里的固定总数。已有未结束轮会被拒绝；先取得真实终态并更新 run.json。恢复时不重新编号已发送请求。

用户明确说“只测试网页评审工作台”时，使用 `node prepare.mjs new web_review`，本轮 stages 只有 `05-web-review.json`。直接进入下方网页服务准备，发送唯一网页请求后做汇总；不把其余工作台记为已测。该模式同样不能绕过未结束旧轮。

用户要求体验新增普通类型时，先调用目录和 `describe-workbench` 核对该类型及汇总所用 `ramble`，执行 `node prepare.mjs check <type>`，再用 `new <type>` 准备。按本类型的材料和结果 schema 核对，不追加到旧轮或替代固定十场景。开发评分演练显式加 `--development` 并确认后端提供 `rating_review`；缺少开发构建时说明缺项。所有身份、恢复、单次交接和汇总规则保持一致。

没有 Node 时，执行等价操作：先核对每个附件有必填的 `file_name` 和本目录内现存的 `path`，再创建 .runs/<时间戳-随机标识>/，复制本轮需要的 fixture，每份先分配独立 UUID 并写入 request_id，把 attachment.path 解析为绝对路径并保留 file_name，再按 prepare.mjs 的 run.json 结构记录进度，更新 latest.json。网页阶段还需要已有的可用开发服务器；没有服务时明确说明缺项，不安装依赖、跳过网页或把纯文本反馈记为网页体验。

所有请求 ID 必须在发送前持久化。保存 CLI 原始响应为对应阶段的 receipt.json / result.json（可在同一命令中写出并显示 stdout）；不要把“请求成功建立”误记成“人类测试通过”。一旦交接成功，结束当前回合，不连续发起后面的请求。超时或投递不确定时保留 ID，先 recover，绝不能另造 ID重试。

## 3. 十个场景

只测试终端时，先确认 `terminal` 和汇总所用 `ramble` 可用，再执行 `node prepare.mjs new terminal`。本轮只有 `06-terminal.json`；prepare 将其中 cwd 占位值解析为 playground 绝对目录，不要直接发送原始模板。Node CLI 只读取固定样例，无服务准备步骤。发送后结束当前回合；结果核对与汇总沿用下述规则，独立终端轮也只有两张卡。恢复旧轮严格保留其原始 stages，不自动追加终端阶段。

每次只发送 run.json 中尚未完成的一个阶段。请求材料中已经包含人类操作说明，不额外要求用户在聊天里回复“继续”。

完整新轮覆盖八种类型，第 2、3 步均为 questions，第 7、8 步均为 visual_feedback；独立网页轮只覆盖网页阶段与汇总。恢复旧轮时严格按该轮保存的 stages、输入和 type 核对，不自动追加阶段、重新编号或替换汇总路径。已有 `03-single-choice.json` / `single_choice` 请求仍检查 `selected_option_id` 与旧 `status`，不要更换请求 ID、改写输入或要求已退出发现目录的类型重新出现。

| 阶段 | 输入文件 | 提交后检查 |
| --- | --- | --- |
| 1 | 01-ramble.json | manifest.workbench.type 为 ramble；result.kind 为 free_feedback；读取实际 markdown、附件引用。 |
| 2 | 02-questions.json | 按问题 id 检查全部答案，value/label/wasCustom 与请求选项或自定义内容一致；自定义仅允许在 allowOther=true 的题；不依靠正文猜答案。 |
| 3 | 03-single-question.json | 类型仍是 questions，answers 恰有一项 id=opening、wasCustom=false，value 属于原始选项；正文为空也应能完成单题选择。 |
| 4 | 04-document-review.json | 来源版本、结论、批注、段落标记均来自真实结构化结果，按下节逐项核对。 |
| 5 | 05-web-review.json | 实际页面/视口/元素上下文、意见、附件和整体正文均来自真实结构化结果；按网页重点核对。发送前先准备样例服务。 |
| 6 | 06-terminal.json | 从体验 Markdown 复制命令；检查 terminal 各轮会话的目录/shell/尺寸/output/screen/终态及正文引用，体验交互/Ctrl+C/页签保留及停止、退出后重新启动。 |
| 7 | 07-visual-feedback.json | 检查空白画布底色、绘制及意见恢复，result.source_version/width/height/annotations 与原请求一致，composite_attachment_id 指向本轮合成 PNG。 |
| 8 | 08-visual-image.json | 背景来自唯一同名持久图片，尺寸正确，标注与意见定位到原画布；检查 PNG、只读历史及原图保持不变。 |
| 9 | 09-diff-review.json | 检查来源版本、文件 ID、零基 hunk_index、old/new 侧和实际行闭区间；整块批注两个行号均为 null，意见与整体正文来自真实提交。 |
| 10 | sort.json | 检查 sort 的 result.order、items 与 removed_ids；保留与删除的 ID 恰好覆盖全部输入，编辑后的文字与顺序符合人类体验，正文可为空，提交后只读。 |

### 网页服务准备与恢复

即将发送网页阶段前执行：

~~~sh
node web-review-server.mjs start '<本轮绝对目录>'
~~~

需要 Node 18+，仅使用内置模块。脚本启动隐藏的独立进程，只绑定 `127.0.0.1`，从仓库固定 public 样例读取 HTML/CSS/JS 和 bridge；不提供反馈 API、不读取反馈凭据、不启动 Vite。首次自动选空闲端口，将实际 URL 写入本轮尚未发送的 `05-web-review.json`，请求 ID 保持不变；fixture 中的 4173 是准备前的占位地址，不能直接发送。服务在交接后继续运行。

`web-review-server.json` 和日志保留在本轮目录；不要将服务控制口令放入报告或反馈附件。恢复时先用 `node web-review-server.mjs status '<本轮绝对目录>'` 检查服务，再按需 `start`。已有服务复用，停止后重启也绑定原端口；已经发送的页面 URL 不可改写，端口冲突时明确报告，不为同一轮另造请求 ID。

没有 Node 18+ 但已有能服务这四份固定样例资产的开发服务器时，手工在发送前填写其实际 URL，并记录如何保持服务和关闭服务；不要直接用不可选择元素的第三方页面替代本例。启动准备只检查本地服务是否就绪，不轮询反馈或等待人类操作。

收到应用自动续接后，用**原 request_id**调用 feedback get，读取反馈包及其引用的 Markdown/附件。状态仍未终态时恢复后交还用户。status=cancelled 或 resolution=cancelled 时记录中止（run.status=cancelled），不发下一张；resolution=approved 表示最终确认完成（run.status=completed），不是普通反馈，也不再发确认请求。其它状态按实际合同处理，不能只看 completed。

本轮已取消或最终确认完成后，若有网页服务记录，执行 `node web-review-server.mjs stop '<本轮绝对目录>'`。网页普通提交后可在完成只读观察时停止服务；未提交请求恢复时保持原服务，不清除其记录。停止只针对该轮服务，不按 PID 杀其他进程。

对于普通提交：把响应保存为 <阶段>.result.json，在 run.json 中记下 outcome、观察结果与未验证项，再发下一阶段。终态输出重复投递时先查看 run.json，确保下一阶段仍使用已分配的同一 ID，不创建重复请求。

文稿审阅重点：
- result.source_version 必须等于请求的 source_version，verdict 必须为 ready 或 changes_requested。
- annotation 的 paragraph_id 存在，id 唯一；普通批注无 replacement，建议改写有 replacement，空字符串是删除建议。
- 局部锚点按 Unicode 标量值计算：[...paragraph.text].slice(start,end).join('') 必须等于 quote。整段锚点 start/end/quote 同时为 null。跨段单条批注不应生成。
- 段落标记无重复且只引用原段落；新界面删除段落写入 remove，恢复时移除此项。v1 合同仍接受旧草稿的 keep/revise/remove，不把历史结果误判为无效。
- 新界面同一段落重复评论或语音评论应打开已有批注续写，建议改写为原有批注增加建议措辞；收起后应显示一行意见。历史草稿同段已有多条批注时仍须保留，不能根据新交互规则删除历史内容。
- 收起按钮应提示“收起批注”并使用收起图标。删除有内容、附件引用或建议改写的批注应二次确认，取消或 Escape 后完整保留；普通空白批注直接删除，空措辞的删除建议仍须确认。
- 批注不再包含解决状态，界面不应提供标为已解决、重新打开或状态筛选；读取旧草稿时忽略历史 status，但保留意见、建议措辞和锚点。不要修改 fixture、原稿或自动应用建议。
- 用户没有体验某个操作就记“未验证”；不能从最终结果推断撤销、重开应用、语音或批注展开续写已通过。

网页评审重点：
- `manifest.workbench.type` 为 `web_review`，`result.source_version` 与请求一致；批注 `id` 唯一，`body` 非空。整体意见单独从反馈 Markdown 读取，不把元素意见注入正文来猜测。
- 每条批注保留当时的 HTTP(S) `page_url`、`viewport`、`element.selector/tag_name/text/rect`。首屏与 `#team-plan` 路由、1280/1440 与 390 视口可以不同；矩形是当时文档的整数 CSS px 坐标，滚动后不能当作视口坐标核对。
- selector 和摘要是捕获线索，不是跨版本身份；返回同页同视口续写保留原意见与上下文，页面改版或元素丢失应明确提示，不能把意见自动挂到别的元素。
- 批注正文的附件引用与可选 `screenshot_attachment_id` 都应指向本请求的真实反馈附件。没有截图时不要补造截图 ID。
- 按附件记录默认普通工作台、网页工具栏“选择元素”旁的全屏评审按钮打开独立页签、评审时会话列表仍可切换、普通/评审页签往返与关闭后保留草稿及网页状态。底部固定批注不随选中元素或网页滚动移动，只显示“批注 N”和意见框，保留删除、收起及输入工具，长正文在框内滚动，小屏限制宽高。浮动卡不展示捕获信息，结果与终态历史仍须保留。记录设备按钮只有图标与 tooltip、没有尺寸文字，以及固定手机画布水平居中；视口数值从捕获数据或页面尺寸核对。还需记录浏览按钮/弹窗、选择模式拦截原点击、评审记录回访、跨 SPA 返回、整体反馈浮层不改变视口、空白批注阻止提交、删除取消、共享输入归属、提交后只读及长记录滚动。必须凭用户实际体验或明确报告，不能仅从结果 JSON 推断这些 UI 操作已通过。
- 网站嵌入或 bridge 未连接时明确记为问题或未验证；仅预览/纯正文提交不证明元素选择通过。用户可按自己的判断只留整体意见，但不伪造元素结果。

终端重点：核对 `manifest.workbench.type=terminal`、result.sessions 和反馈正文引用；各轮会话 id 唯一、目录/shell/尺寸/原始 ANSI 输出/最新画面与用户体验一致。退出码仅代表 shell 会话结束，不把它当作某条 CLI 的退出码或用户满意度。较早日志截断必须保留 truncated 标记；原始按键不应出现在独立 input 字段。需要实际体验从 Markdown 复制命令、自由输入、方向键/Ctrl+C、输出引用、页签与请求切换后会话保留、停止及 shell 退出后主动重新启动（旧轮记录保留、新轮 ID 不同）、提交停止收尾并冻结所有会话、终态只读、应用重启后保存记录。未操作的项记为未验证，不能从 JSON 推断 UI 全部通过。

排序重点：核对 `manifest.workbench.type=sort`，`result.order` 按优先级从高到低保存保留条目的稳定 ID，`result.items` 同序保存编辑后的名称，`removed_ids` 记录删除项目。保留与删除必须构成原输入的完整、无重复划分，不新增 ID；显式删除全部条目可返回空列表。空名称能保存草稿，但保留的条目名称为空时不能提交。保留初始顺序也是有效结果，排序理由从反馈正文读取。旧版仅 order 的历史结果按原输入名称解释，不改写旧结果。编辑、删除与恢复、拖动、手柄键盘排序、切换请求后的恢复和提交后只读都需要实际操作或用户明确报告；普通工作台没有全屏入口或上下按钮。触屏未体验时记为未验证。

共享输入重点：

- 新请求默认向右侧正文输入；正文、自定义回答、批注意见和建议措辞顶部都应有麦克风、截图、粘贴和附件工具栏，并共享一个录音会话。
- 同框再次点击麦克风会暂停，点击另一框麦克风则保持录音并切换后续语音。右下角只保留左侧 Rambelle 大头像、右侧状态对话气泡和条件式整理按钮，不应再出现字幕、输入位置、定位、默认输入或暂停控件。
- 已开始的语音保留原目标，切换题目或批注不会重定向；清除或改选固定选项后，迟到转写不能覆盖新选择。
- Rambelle 区域在有待整理语音时显示“整理 N 段语音”，忙碌时显示处理状态，失败时可重试，成功后收起按钮；没有待整理内容时不显示空按钮。
- 整理只统计当前请求中尚未整理的语音来源段，覆盖补充说明、答案和批注；键入与普通粘贴文字不进入批次。
- 字段工具栏的截图、图片粘贴和附件进入该字段的附件标签，能预览且不变成正文中的无关引用。文件选择或截图期间换框不改变原归属；附件依然属于当前请求。
- 整理后各段原位更新，切换目标不改变本批次位置，后来录入的语音留到下一批次，异步期间被修改或删除的内容不被覆盖。
- 提交区域的“整理补充说明”及“整理补充说明并提交”仍是 Cooking，只处理补充说明；不能把它记录为跨字段语音整理通过。
- 工具栏与 Rambelle 位置、语音来源标识、条件式整理按钮、字段附件标签和异步保护都需要实际操作或用户明确报告，不能仅从最终 Markdown 或答案字符串推断。未配置识别、整理或采集能力时记为未验证。

## 4. 汇总与结束

本轮 stages 中的真实结果读完后，在当前运行目录创建 REPORT.md，至少包含：
- 本轮标识、实际运行的全部 request_id、类型/版本；完整新轮为十个场景，独立类型轮按筛选结果，旧轮按原记录。
- 每种工作台的实际人类输入摘要和结构化结果检查结论。
- 区分“结果验证通过 / 用户报告问题 / 未验证 / 中止”，并列出问题复现材料。
- 特别记录是否实际测试了默认输入区、每框顶部工具栏、同框暂停/跨框切换、右下 Rambelle 条件式整理、自定义回答、字段附件标签与换框后归属、清除或改选后的迟到转写、跨字段“整理 N 段语音”、键入与普通粘贴不入整理批次、异步整理期间修改或删除的保护、空正文提交、Unicode 选区、批注原地续写与收起摘要、段落删除线与恢复、删除建议、原稿内批注阅读与续写、草稿恢复。没证据就写未验证。
- 人类主动说没问题可以作为其体验反馈记录，但不能代替结构化结果核对。

使用 run.json 预留的 report_request_id 写出汇总输入：文件编号取 `stages.length + 1`，两位数字（完整新轮 `11-report.json`、独立网页/终端轮 `02-report.json`、原七阶段旧轮仍为 `08-report.json`、原四阶段旧轮仍为 `05-report.json`）；恢复已发送汇总时使用原文件。workbench.type=ramble、version=1、data.actions 只放一个“查看本轮报告”的动作；title="工作台测试 · 本轮汇总"；what_happened 不超过 200 个 Unicode 标量值；REPORT.md 作为绝对路径 Markdown 附件；allow_finish=true；final_summary 用实际发现写简短摘要（最多 12000 字符）。不要预先写“全部通过”。

将本轮状态写为 awaiting_report，把汇总卡发给人类并结束当前回合。用户可批准结束，也可提交补充问题。读到批准或取消后更新本轮状态并结束，不再递归发确认卡。若提交补充意见，将其加入报告；只有用户明确表示结束时才按托管会话的 task_finished 机制结束，否则回应实际问题并按该会话规则交接，不能把补充意见当作批准。完整新轮十一张卡、独立网页/终端轮两张卡是正常完成路径，追加问题可能产生后续交接。这个测试任务本身不修复应用；用户另行明确要求的工作另算。

若补充意见需要后续交接，发送前把新 request_id、输入文件路径、阶段与状态写入 run.json.followups；恢复时优先检查未终态 followup，不能退回汇总卡重复发送。用户明确结束后将 run.status 更新为 completed。

人工取消不会自动续接。用户再次发“继续/新一轮”时先 get 原请求，记录中止；只有明确重跑才开始新轮。汇总页批准正常会续接，若续接失败或会话中断，本地 awaiting_report 只是待对账状态，下次开始前通过 get 确认终态即可。
