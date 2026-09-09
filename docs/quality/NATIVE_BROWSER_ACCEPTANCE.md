# 原生与浏览器实机记录

2026-09-09 至 09-10，macOS 本机；验收发生时 HEAD 为 rc3 `b7ab9d0`，各批次构建包含当时的工作区改动，源码与产物哈希分别记录。最终代码已提交为 `27b50f1` 与 `5d57526`，不将后续修复计入较早构建。所有操作使用独立数据库、loopback 服务和测试反馈，未接触日常工作数据。下面区分真实 UI 操作、产物核对与未验范围。

## macOS 原生

构建：`tauri build --debug --bundles app --config /tmp/rambledesk-quality-native.json --ci`；Rust debug、Vite production frontend、ad-hoc 签名，独立 identifier `io.rambledesk.quality`。下表先记录 00:18:57 完成的 build3（`/tmp/quality-native-build3.log`）；后文分别列出 build4 及后续 Web Access 构建的产物与实测。它们都不是官方安装／升级产物。

| 实际操作 | 观察结果 |
| --- | --- |
| 打开普通结构化草稿，键入标记，等待 Saved r2，Cmd-Q，再启动应用 | 原有 H2 结构及 `Native acceptance: retain this paragraph across restart.` 均恢复 |
| 原生文件选择器导入 51 字节 TXT | 编辑器出现附件引用、Saved r4；TXT 弹窗明确说明不支持预览，不把此项算作正文预览通过 |
| 提交普通反馈，打开反馈包 | 终态显示，编辑和重复提交禁用；Finder 实际打开含 attachments、feedback.md、manifest、uncooked 的反馈包目录 |
| 标题栏双击，再次双击 | 窗口放大并恢复；没有据此声称拖动手势已验 |
| 最终构建另一草稿键入 `Native final toolbar verification.`，点击 Undo，再点击 Redo | 输入后 Undo 启用；Undo 移除文字并启用 Redo；Redo 恢复完整文字 |
| 提交该草稿 | 英文 UI 显示英文 `Feedback submitted · return to host` 和 fallback 提示；通用默认文案的 locale 修复有效 |
| Cmd-Q 后只读检查 SQLite 和包文件 | 两次提交均 completed/r4，包 revision、manifest、正文和附件 hash 一致；[原生包证据](evidence/native-package.json) |

证据工具曾用 SQLite `immutable=1` 忽略尚在 WAL 的已提交记录，误读成 seed。该读取结果已在 [作废证据](evidence/native-seed-read-invalidated.json) 保留原因；工具改为只读、包含 WAL 的连接，并新增真实 WAL 回归。实际 UI 与重验结果相符，不能用旧 seed 结果证明提交。

00:51 的 **build4** 还包含保存 invalidation 原地刷新和私有 Markdown parser 修复。再次启动该隔离应用，
在 240 段草稿键入 `Undo must survive an accepted autosave.`，明确等到 **Saved r2** 后点击 Undo，整段标记消失、
Redo 启用；点击 Redo 恢复完整标记，并保存到 r4。这是保存后的真实原生操作，不只是在 autosave 前点击。
随后点击 Cancel feedback，确认取消对话框，UI 显示 Cancelled、草稿只读。正常 Cmd-Q 后重新核对
[最终原生包证据](evidence/native-package-final.json)：长草稿 cancelled/r4，已发布的两个邻居及各自包保持不变，
所有外部测试请求的 managed delivery 数为 0。该次构建仍为 debug/ad-hoc，不升级为正式安装验收。
[最终原生产物清单](evidence/native-final-provenance.json) 保存 bundle 的 18 个文件及 SHA-256；
`codesign --verify --deep --strict` 通过，签名身份为 ad-hoc、无 TeamIdentifier，未公证。

## Chrome 与 Safari

- Chrome **152.0.7977.83**，独立访客窗口；Safari **26.3.1（21623.2.7.11.7）**，独立私人窗口。均为真实浏览器应用，通过原生 UI 操作。
- Chrome：键入正文、设为 H2、保存、刷新，结构保留；真实文件选择器导入附件；提交后通过浏览器保存对话框下载 JSON。实际下载文件与 HTTP / SQLite / 磁盘包核对一致，见 [Chrome 下载证据](evidence/chrome-download.json)。
- Safari：打开预置 Markdown 请求附件，实际预览出 H2；键入完整 `Safari acceptance: preserve both attachment references.`，Saved r3 后刷新恢复，附件引用保留；提交并实际下载，见 [Safari 下载证据](evidence/safari-download.json)。
- 最终构建 Chrome：未保存格式变更后立即刷新，浏览器显示“系统可能不会保存您所做的更改”确认框；选择取消后草稿保留。Saved r2 后再次刷新不再弹框，已保存结构恢复。保存过程中 Undo 回到旧基线仍应保护的场景由真实 App 测试覆盖，不冒充手工通过。
- 最终 parser/navigation 构建 Chrome：另一未提交测试请求键入 `Chrome final saved history survives.`，明确等到 Saved r2 后点击 Undo，标记消失且 Redo 可用；Redo 恢复完整标记。与原生复验共同证明自动保存后的刷新不会再清掉编辑历史。

原生工具的 HTML paste 与 Safari AX setValue 未能可靠产生页面输入事件；此类自动化尝试没有登记为产品通过。文件选择、Markdown 附件预览和真实下载分别有证据，不能合并成“所有粘贴和预览已通过”。

## 完成边界

本记录分批覆盖 macOS 核心编辑、重启恢复、文件导入、终态展示、包打开，以及 Chrome / Safari 的部分核心 Web 合同。
下述 build6 已实际完成两端各自草稿的 Stop → 离线编辑 → Start → 重新认证 → 保存与刷新恢复，
以及同一草稿交换先后顺序的两轮 CAS、冲突原文保留和 token 轮换后的撤销与重新认证。
这些具体路线通过，不代表所有断线组合或 Web 功能均已验证。

正式安装／升级、所有系统权限与录音设备中断、托盘、真实图片剪贴板、全部 native 手势仍未覆盖。
Windows 与手机真机环境暂不可用，用户明确要求保留待验。Agent 实际失败另见 [Agent 验收](AGENT_ACCEPTANCE.md)，
不能以这些夹具替代真实 Agent 闭环。

## build4 历史：Web Access 在 Keychain 读取中阻塞

9 月 10 日约 01:13，在 build4 的 Native Quality（PID 16633、独立 `tqXZR0/validation.sqlite3`）
尝试现有 Web Access 设置，状态最初为 Stopped，将端口从 37643 改为 55903 后点击 Start。
UI 持续 Starting 超过 45 秒，没有应用内错误；未观察到可见系统授权弹窗，CUA 无法取得 SecurityAgent
交互表面。此时尚未进行 Chrome dirty 草稿的断线或重新认证操作。

[独立诊断证据](evidence/native-web-recovery-blocked.json) 保存关键栈片段、完整 sample 的路径和 SHA-256，
以及端口、日志与退出核对。01:14:37 的一秒只读采样中，该线程 260 个样本均处于
`start_web_access → load_or_create → keyring::get_password → SecKeychainFindGenericPassword →
SecurityServer::decrypt → mach_msg2_trap`。55903 没有监听；该 PID 仅有 Local Integration 的
127.0.0.1:55590 listener。应用日志只有一条 CapsLock 系统消息，主线程仍在正常 AppKit event loop。
源码先读取 OS credential，再创建 Web listener；因此可以定位为 Keychain 同步 IPC 尚未返回，
不能进一步声称已证明某个可见授权弹窗或其具体系统原因。

没有执行 Rotate，也没有提取、打印 token 或变更安全设置；诊断没有杀进程。随后通过 Native UI
正常 Cmd-Q 退出，另用 `ps` 确认 PID 16633 已不存在。正在运行的独立反馈夹具没有因此被停止或修改。
本次结果是这台机器上 debug/ad-hoc Quality 构建的未完成尝试，不能代表正式签名／公证版本的故障或成功；
该次运行未走到 Web listener、认证和 dirty 草稿恢复 UI，不把启动阻塞算作重新认证测试通过。
这一失败保留为 build4 历史；私有令牌文件后的启动及重新认证结果分别记录如下。

## build5：私有令牌文件启动与重新认证后的保存停滞

[build5 产物清单](evidence/native-web-build5-provenance.json) 固定本次 debug/ad-hoc bundle。
原生 Quality PID **33504** 使用隔离的 `s9sGEX/validation.sqlite3`。首次 Start 后，**1.3 秒**的 UI
快照仍为 Starting，**8.523 秒**的快照已为 Running；这是两个观察时点，不将 8.523 秒当作精确启动耗时。
本次没有出现系统凭据授权提示。实际 app-data `auth` 目录权限为 **0700**，`web-access.token` 为
**0600**；该 PID 同时监听 Web Access **55903** 与 Local Integration **57876**。

Chrome Guest 和 Safari Private 分别编辑各自请求到 **Saved r2**，实际刷新后均通过 cookie 恢复，
不需再次输入 token。随后执行以下真实动作：

1. Native UI Stop Web Access，55903 停止监听；两个已有浏览器页面仍接纳离线文字，但一直显示 Saving。
2. Web 停止期间，Native 将邻居 **03** 请求保存到 **r3**，证明停止 Web 没有停止 Backend Runtime。
3. Native Start 同一端口；两端出现认证 gate，明确 Connect 后 gate 消失，页面内离线文字仍在。
4. 两端重新认证后仍停留 Saving **超过 60 秒**，SQLite 中两份草稿仍为 r2，离线标记未持久化。

[停滞证据](evidence/native-web-reauth-save-stalled.json) 保存操作顺序、SQLite 核对及当时页面文本。
通过可访问性读取保留原文后，才重载新 bundle；没有把保留页面文本或重新认证 gate 消失写成保存成功。

问题定位到断线重连时重复创建 ready 等待，使旧保存等待未被后续 ready、认证撤销或连接失败结束。
[定向回归](../../apps/desktop/src/lib/application/httpApplicationSessionReconnect.test.ts) 的四条用例先红后绿，
覆盖正常 ready、撤销、health probe 失败，以及真实 Draft 保存过程经重新认证后的恢复。实际浏览器复验见 build6。

## build6：Chrome 与 Safari 的离线草稿保存恢复

[build6 产物清单](evidence/native-web-build6-provenance.json) 固定修复后的 bundle；Native Quality
PID **37895** 使用同一隔离数据库。Chrome Guest 和 Safari Private 在两条独立请求上重复相同路线，
重新认证恢复过程中均未重载页面：

| 实际操作 | 两端观察结果 |
| --- | --- |
| 从各自 Saved r2 开始，Native Stop Web，再在现有浏览器页输入唯一离线标记 | 保存结束等待并显示 Save failed，正文保留，不再永久 Saving |
| Native Start 同一端口，浏览器出现认证 gate，明确 Connect | gate 消失后保留原 Editor 内容，自动保存到 Saved r3 |
| 只读核对 SQLite | 两份结构化文档各自保留 H2 和所属浏览器唯一标记；Chrome 标记未串到 Safari 请求，反向亦然 |
| 随后分别在 Chrome、Safari 实际 Cmd+R | 没有未保存离页提示、没有再次出现认证 gate；r3 和各自标记恢复 |

[恢复证据](evidence/native-web-reauth-save-recovered.json) 保存两端操作方法、请求 revision、文档 SHA-256
与 H2/唯一标记检查。邻居请求保留独立状态；此前 Native 在 Web 停止期间成功保存的事实继续成立。
这些结果证明本次真实 Native Web Access 的启停、两端重新认证、离线保存失败释放和随后保存恢复，
同一草稿竞争与 token 轮换由下一节独立验证；实体手机和正式安装包仍待验。

## build6：双浏览器 CAS 与 token 轮换

两轮均从 Chrome Guest 与 Safari Private 加载同一请求的 **Saved r3** 开始。Native Stop Web 后，
两端各自输入唯一标记；Native Start 后，按以下顺序完成认证：

| 回合与目标 | 先认证的一端 | 后认证的一端 | SQLite 核对 |
| --- | --- | --- | --- |
| A：01 普通请求 | Chrome 保存到 r4，正文含 `CHROME WINNER` | Safari 显示 Save failed；`CAS SAFARI LOSER` 原文完整保留在编辑器 | 只有 Chrome 胜者标记；其余请求未改变 |
| B：04 请求 | Safari 保存到 r4，正文含本轮胜者标记 | Chrome 显示 Save failed；本轮输家原文完整保留在编辑器 | 只有 Safari 胜者标记；两轮均无输家标记入库 |

A 回合先将 Safari 输家原文通过可访问性读取保存到隔离 fixture，再明确确认重载；重载后看到
Chrome 胜者内容与 r4。B 回合的 Chrome 冲突稿继续留在页面中。两轮后，长草稿仍为 **r1**，
附件请求仍为 **r5**，两份竞争草稿各为 **r4**。没有自动合并、丢弃冲突稿或覆盖已经保存的胜者。

随后在 Native Settings 点击 Refresh token 并确认，实际轮换的仅为 `io.rambledesk.quality` 自有令牌文件。
Chrome 和 Safari 已有 session 同时出现认证 gate。Safari 手动用旧 token Connect，明确显示
“That Web Access token was not accepted. Check it and try again.”；新 token 在两端均被接受。
重新认证后，Chrome 未保存的冲突原文仍在，Safari 的干净草稿仍为 Saved r4。

[CAS 与轮换证据](evidence/native-web-cas-rotation.json) 固定两轮操作、各请求 revision 与文档 SHA-256、
胜者／输家标记检查，以及两份原始页面文本的路径和文件 hash；不包含 token 或凭据 hash。
这补足本机两个浏览器的上述并发与撤销路线，不扩展为其他设备、媒体手势或正式安装验收。

本轮验收完成后正常退出 Quality（PID 37895），确认 Web 55903 关闭；Chrome 访客、Safari 私人页及 IAB
测试页已关闭，临时视口恢复。删除本轮自有的 Quality Web / Local Integration 凭据，保留独立 SQLite、
附件与两端冲突原文。见[清理记录](evidence/native-web-cleanup.json)和[离线草稿核对](evidence/native-web-final-drafts.json)。
后者的 URL 为最初 seed launcher 的历史地址；本轮实际 Native Web 地址始终为 127.0.0.1:55903。
