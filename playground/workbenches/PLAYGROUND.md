# 完整工作台测试执行规程

面向在本目录工作的 Agent。用户发出“读取 PLAYGROUND.md，开始完整工作台测试”即授权执行下面的人工体验流程；用户操作工作台，你负责材料、续接、结果检查与报告。

## 1. 能力与现有进度

1. 读取本目录 README.md。测试期间只读固定材料，只在 .runs/ 写运行产物。不要修改父仓库代码、修复应用、安装依赖或替用户操作工作台。
2. 使用本会话注入的 RAMBLEDESK_COMMAND 命令。不要读取、打印、保存或改写私有 RAMBLEDESK_FEEDBACK_* 变量，不拼接 HTTP endpoint，不使用其他会话的 MCP/Pi/dsh 通道。
3. 调用 feedback list-workbenches --limit 20，按 next_offset 读完目录；确认 ramble、questions、single_choice、document_review 的 v1 均可用。分别调用 feedback describe-workbench --type <type> --version 1，核对 fixtures 中的合同。不要把整个目录或凭据写入报告。
4. 缺少命令、会话能力或任何类型时，直接说明具体缺项和需要重开/更新的应用版本；不要用自由正文替代类型、伪造成功或悄悄跳过。若目录多于这四种类型，报告本套材料覆盖的四种及额外未覆盖项，不能声称全覆盖。
5. 检查 .runs/latest.json 及对应 run.json。若已有本轮记录，先按记录中的 request_id 用 feedback get / recover 取真实状态；不能仅凭本地阶段判断已提交。用户说“继续”但没有记录时，说明没有可恢复的本轮记录，不擅自启动新轮。
6. 非终态请求只恢复原请求并结束当前回合，不能新建、轮询、sleep 或阻塞等待。取消即中止本轮。用户明确要求新一轮时，必须先确认原请求已终态；不能自动取消原请求。跨会话无法恢复时回原会话继续，不为同一轮另开 ID。

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

脚本只生成本地文件，不发请求。它生成独立 UUID、将附件路径变成绝对路径，写出 .runs/<run-id>/run.json 和四份请求文件，并更新 latest.json。已有未结束轮会被拒绝；先取得真实终态并更新 run.json。

没有 Node 时，执行等价操作：创建 .runs/<时间戳-随机标识>/，从四份 fixture 复制输入，每份先分配独立 UUID 并写入 request_id，把 attachment.path 解析为本目录内现存文件的绝对路径，再按 prepare.mjs 的 run.json 结构记录进度，更新 latest.json。不需要安装 Node。

所有请求 ID 必须在发送前持久化。保存 CLI 原始响应为对应阶段的 receipt.json / result.json（可在同一命令中写出并显示 stdout）；不要把“请求成功建立”误记成“人类测试通过”。一旦交接成功，结束当前回合，不连续发起后面的请求。超时或投递不确定时保留 ID，先 recover，绝不能另造 ID重试。

## 3. 四个场景

每次只发送 run.json 中尚未完成的一个阶段。请求材料中已经包含人类操作说明，不额外要求用户在聊天里回复“继续”。

| 阶段 | 输入文件 | 提交后检查 |
| --- | --- | --- |
| 1 | 01-ramble.json | manifest.workbench.type 为 ramble；result.kind 为 free_feedback；读取实际 markdown、附件引用。 |
| 2 | 02-questions.json | 按问题 id 检查全部答案，value/label/wasCustom 与请求选项或自定义内容一致；自定义仅允许在 allowOther=true 的题；不依靠正文猜答案。 |
| 3 | 03-single-choice.json | result.status=answered，selected_option_id 属于原始选项；正文为空也应能得到有效选择。 |
| 4 | 04-document-review.json | 来源版本、结论、批注、段落标记均来自真实结构化结果，按下节逐项核对。 |

收到应用自动续接后，用**原 request_id**调用 feedback get，读取反馈包及其引用的 Markdown/附件。状态仍未终态时恢复后交还用户。status=cancelled 或 resolution=cancelled 时记录中止（run.status=cancelled），不发下一张；resolution=approved 表示最终确认完成（run.status=completed），不是普通反馈，也不再发确认请求。其它状态按实际合同处理，不能只看 completed。

对于普通提交：把响应保存为 <阶段>.result.json，在 run.json 中记下 outcome、观察结果与未验证项，再发下一阶段。终态输出重复投递时先查看 run.json，确保下一阶段仍使用已分配的同一 ID，不创建重复请求。

文稿审阅重点：
- result.source_version 必须等于请求的 source_version，verdict 必须为 ready 或 changes_requested。
- annotation 的 paragraph_id 存在，id 唯一；普通批注无 replacement，建议改写有 replacement，空字符串是删除建议。
- 局部锚点按 Unicode 标量值计算：[...paragraph.text].slice(start,end).join('') 必须等于 quote。整段锚点 start/end/quote 同时为 null。跨段单条批注不应生成。
- 段落标记无重复且只引用原段落，decision 只允许 keep/revise/remove。
- resolved 仍是人类的意见状态，不等于 Agent 已经改稿；不要修改 fixture、原稿或自动应用建议。
- 用户没有体验某个操作就记“未验证”；不能从最终结果推断撤销、重开应用、语音或工具条定位已通过。

## 4. 汇总与结束

四份真实结果读完后，在当前运行目录创建 REPORT.md，至少包含：
- 本轮标识、四个 request_id、运行的类型/版本。
- 每种工作台的实际人类输入摘要和结构化结果检查结论。
- 区分“结果验证通过 / 用户报告问题 / 未验证 / 中止”，并列出问题复现材料。
- 特别记录是否实际测试了自定义回答、空正文提交、Unicode 选区、删除建议、解决/重新打开、草稿恢复。没证据就写未验证。
- 人类主动说没问题可以作为其体验反馈记录，但不能代替结构化结果核对。

使用 run.json 预留的 report_request_id 写出 05-report.json：workbench.type=ramble、version=1、data.actions 只放一个“查看本轮报告”的动作；title="工作台测试 · 本轮汇总"；what_happened 不超过 200 个 Unicode 标量值；REPORT.md 作为绝对路径 Markdown 附件；allow_finish=true；final_summary 用实际发现写简短摘要（最多 12000 字符）。不要预先写“全部通过”。

将本轮状态写为 awaiting_report，把第五张汇总卡发给人类并结束当前回合。用户可批准结束，也可提交补充问题。读到批准或取消后更新本轮状态并结束，不再递归发确认卡。若提交补充意见，将其加入报告；只有用户明确表示结束时才按托管会话的 task_finished 机制结束，否则回应实际问题并按该会话规则交接，不能把补充意见当作批准。五张卡是正常完成路径，追加问题可能产生后续交接。这个测试任务本身不修复应用；用户另行明确要求的工作另算。

若补充意见需要第六张及后续交接，发送前把新 request_id、输入文件路径、阶段与状态写入 run.json.followups；恢复时优先检查未终态 followup，不能退回第五张卡重复发送。用户明确结束后将 run.status 更新为 completed。

人工取消不会自动续接。用户再次发“继续/新一轮”时先 get 原请求，记录中止；只有明确重跑才开始新轮。汇总页批准正常会续接，若续接失败或会话中断，本地 awaiting_report 只是待对账状态，下次开始前通过 get 确认终态即可。
