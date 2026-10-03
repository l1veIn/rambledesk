# 0.5.0-rc.1 源码与 Windows 原生验收

本记录固定保存 2026-10-03 对提交 `456dfeb16fdb02d2ce48c1399c1a417461652243` 的源码检查、三平台 CI 和 Windows 源码构建原生验收结果。下文所列场景已经执行并核验；正式候选安装包的交付结论按文末独立判据记录，不由源码验收推定。

## 对象与范围

| 项目 | 绑定对象 |
| --- | --- |
| 上一正式版本 | `v0.4.0` |
| 验收版本 / 提交 | `0.5.0-rc.1` / `456dfeb16fdb02d2ce48c1399c1a417461652243` |
| 变更入口 | [PR #38](https://github.com/l1veIn/rambledesk/pull/38) |
| 同一提交 CI | [run 37112798525](https://github.com/l1veIn/rambledesk/actions/runs/37112798525)，三个平台全部成功 |
| 实现口径 | 10 种生产工作台、13 个 playground 场景；开发专用场景另计，见[工作台索引](../workbench/README.md) |
| 原生环境 | Windows；本地 Tauri debug 可执行文件嵌入生产前端；数据库、材料库、令牌、WebView2 及项目目录独立隔离 |

`v0.4.0` 到该提交共有 51 个提交。初审逐提交检查原有 49 个提交（含合并提交）的变更，并单独检查当时未提交的表格、媒体及差异评审材料；随后复核两个发布准备/修复提交。关键协议、持久化、生命周期、工作台、脚手架、网站和发布流程均在审查范围内，不据此声称逐行审阅全部新增代码。

原生反馈使用合成材料和测试输入，验证产品交互与持久化行为，不作为独立真人产品判断。测试、浏览器夹具、原生程序和安装器的证据分别记录。

## 发现与修复

下列修复均已进入本记录绑定的提交。R1–R9 来自初审，R10–R13 来自后续原生体验、真实 Agent 探针和跨平台 CI。

| 编号 | 问题及影响 | 修复与回归 |
| --- | --- | --- |
| R1 | 脚手架附件遗漏 `file_name`，新工作台不能通过真实 prepare 校验 | 补齐生成字段；隔离测试实际组合[生成器及测试](../../scripts/workbench-new.test.mjs)与 playground prepare，覆盖普通和开发工作台 |
| R2 | 媒体工作台测试有 3 处类型错误 | 使用带检查的类型窄化；桌面类型门禁通过 |
| R3 | 合同测试仍只预期 8 类，遗漏表格与媒体 | 覆盖 10 类目录、describe、HTTP/command/MCP 一致性、创建和跨适配器重放；真实 WAV 写入/读回逐字节核对 |
| R4 | 表格输入框双击冒泡，重新编辑时覆盖未保存内容 | 阻止双击冒泡；新增实际事件序列先复现失败再通过，另以原生 UI 验证 |
| R5 | 未完成或失败的 Agent 调用被误算为文件改动 | 仅聚合成功完成的调用；覆盖 pending、in-progress、failed 和失败删除 |
| R6 | Windows 扩展 UNC 路径转换后可能失去绝对路径语义 | 正确处理 `\\?\UNC\`；Windows 路径回归通过，真实 SMB 会话不在本次范围内 |
| R7 | 网站自动保存取消提交定时器，演示停在发布中 | 按请求和操作管理定时器；11 个真实组件回归及生产预览浏览器回验通过，见[网站测试配置](../../web/vitest.config.mjs) |
| R8 | 升级/回退说明未区分历史兼容合同与新增扩展 | 明确 0.4.0 仅支持扩展 20，新版到 22；回退须恢复完整备份，见[数据兼容说明](../DATA_COMPATIBILITY.md) |
| R9 | 文档仍为 8 类/10 场景，遗漏新增材料合同 | 同步为 10 类/13 场景，补齐字段、格式传播和开发说明；历史验收数量保留原时点含义 |
| R10 | 原生最低窗口宽度在高 DPI/小工作区超出可用区域 | 调整最低尺寸并限制在工作区；实测最小窗口可用 |
| R11 | ACP 权限请求遗漏工具标识/详情，审批目标不明确 | 保留工具 ID，仅以同会话、同轮次的对应调用补足信息；缺失时明确显示，不推测或自动授权；定向回归和原生权限卡验证通过 |
| R12 | 首次协议发送前快速取消，可被迟到发送清除 | 公布 Running 前同步准备，发送/取消共用分发锁，取消分发期间阻止下一轮进入；真实 ACP 门闩红/绿测试及跨平台 CI 通过 |
| R13 | macOS 临时目录别名使合法材料被误判越界 | 根目录与附件均取真实路径；真实目录别名回归通过，同时继续拒绝目录外逃逸 |

另修正 Windows HTTP 验收器对扩展路径的误判，保留归一化后的组件级包含检查；Linux continuation 测试改为等待实际子进程退出，不再以反馈投递状态代替进程结束。后者属于测试时序修正。

首轮 CI [37111550962](https://github.com/l1veIn/rambledesk/actions/runs/37111550962) 的快速取消、路径别名和 continuation 失败，以及初次真实 Agent 权限中止记录均保留。后续成功不改写这些发现过程。

## 自动化与协议结果

最终 CI 于 `2026-10-03T09:49:57Z` 观测为 completed / success。以下计数来自三个 job 的实际日志；门禁之间有重叠，不相加为独立测试总数。

| 门禁 | Linux | Windows | macOS |
| --- | --- | --- | --- |
| Rust workspace（不含 desktop） | 535 通过、2 ignored | 546 通过、2 ignored | 549 通过、2 ignored |
| Desktop Rust | 134 通过 | 130 通过 | 138 通过 |
| Core/storage workbench fixtures | 294 通过 | 295 通过 | 294 通过 |
| 桌面前端 | 2,078 通过、1 跳过 | 2,078 通过、1 跳过 | 2,078 通过、1 跳过 |
| 工作台工具 | 17 通过、1 跳过 | 17 通过、1 跳过 | 17 通过、1 跳过 |
| Python feedback acceptance | 9 通过、2 平台跳过 | 10 通过、1 平台跳过 | 9 通过、2 平台跳过 |

三个平台均通过格式、大小约束、术语/核心边界检查、workspace all-targets Clippy（警告视为错误）、Svelte/TypeScript、前端生产构建和生成合同一致性。Linux 另通过 Pi 22、DSH 28、完整工作台扩展演练、官方 MCP Inspector、网站类型检查/11 个测试/构建。网站测试以 `pnpm test:web` 长期运行，复用桌面 Vitest 依赖，不进入生产 bundle。工作流见 [ci.yml](../../.github/workflows/ci.yml)。

Linux 的 Rust 测试未使用 `--all-targets`，Windows/macOS 使用该参数，平台专用用例也不同，因此数量不同。条件跳过包括：由父测试调用的 Rust 子进程 helper、显式启用的真实 npm catalog 安装、需要已有模型配置的 Cooking、需 `RAMBLEDESK_PLAYGROUND_TEST_COMMAND` 的真实 CLI loopback，以及平台或 symlink 权限相关的 Python 用例。没有据此认证真实模型 Cooking 或全部 CLI/设备环境。

同一提交的本地生成合同、完整扩展演练和 MCP Inspector 也通过，临时源码修改已恢复。真实隔离 HTTP 服务完成保存、旧 revision 拒绝、3 次发布及重放、1 次取消；在线和停服后的 SQLite/包哈希一致，包含反馈附件与请求材料。存储回归固定旧版 SQL 校验和，重建 0.4.0 账本及请求/草稿/材料，验证升级、重复打开和旧材料字节保留。

本地中间 Rust 全量结果早于快速取消修复，未作为最终提交结论；未运行完的重复本地检查没有记为通过。最终完整门禁以以上同一 SHA 的 CI 为准。

## 官方旧版数据的原生升级与回退

官方 `RambleDesk_0.4.0_x64-setup.exe` 已核对 SHA-256 和 updater 签名，篡改样本被拒绝。安装器 SHA-256 为 `a2b64880fe696f5680b8d588351ea8ea10f6f13e5ab8b5659cd4429e63d2482d`。从中解包运行官方旧版，在独立数据目录用原生 UI 保存 revision 1 草稿，退出后完整备份。这证明官方旧版程序运行与保存，不作为安装器安装证明。

本地源码验收可执行文件的 SHA-256 为 `38a535ebd3f40d3232a5b88f12f2dbd038bc197212212a9dcc17546020414d04`；构建清单同时记录 14 个资源文件。源码程序打开备份的独立副本后，只读 SQLite 检查确认：

- 扩展由 20 升到 22，原扩展账本字段未变；18 个旧表按主键比较，没有丢失旧行/列。
- 请求、草稿、宿主会话和 request action 的旧列值一致；草稿正文、文档、revision、时间戳保留。草稿 SHA-256 为 `0740106b4b47b4b9ca7857482143c77b966cdb42897cb2048a32ac4fe6222396`。
- SQLx 1–10 保留，1–3 的 checksum 差异经直接 SHA-384 核对确认为已有 LF/CRLF 兼容归一化，其余账本字段一致。
- SQLite `quick_check=ok`、外键违规为 0。旧版原生夹具没有附件或 managed session，因此这些已有数据的保留不由该夹具证明；填充材料字节保留另有存储升级回归。

随后在完整副本上运行官方 0.4.0，UI 实报 `database_version_unsupported`，第一个不支持的扩展为 **21 > 20**；升级账本实际仍到 22。所有用户数据、扩展账本和 schema 与拒绝前一致。

此次拒绝**并非零写入**：官方 LF 构建在拒绝前，将 SQLx 1–3 的 checksum 从本地 CRLF 变体归一化为 LF 变体；SHA-384 证明两者属于已知等价换行变体，其他字段未变。初次“所有逻辑字段不变”的严格断言因此失败并保留原记录；最终核验分别验证数据/扩展/schema 不变及精确 checksum 归一化，没有放宽产品迁移规则。

官方旧版成功打开原完整备份的另一副本，原生 UI 显示原草稿和 revision 1；恢复后的 18 表全部旧列/行与原备份一致，SQLx 1–10、扩展 11–20，完整性及外键检查通过。最终回退 verifier 退出 0、7 项检查全通过。

原备份数据库、WAL 等持久文件哈希与最初记录一致。早期只读 SQLite 访问曾改变 SHM 临时读锁元数据，因此不声称全部 sidecar 字节全程不变；最终 verifier 使用另建 DB+WAL 副本读取，没有再修改原备份文件。完整备份恢复有效，不表示旧版能直接使用升级后的数据库。

## 真实 Agent 两轮与原生工作台

通过原生设置检测已安装的 `deepseek-acp@0.8.0`，选择隔离项目和 DeepSeek（DSH），从原生输入框发起任务。模型实际产生两次反馈请求，均在原生编辑器提交；两次投递后 Agent 继续，最后回复包含两个精确合成标记，并成功执行 `feedback skip --reason task_finished`，返回 `skipped / task_finished`。此处 skip 是已验证的结束记录操作。

本次 Windows 适配器的 `workspace-write` 沙箱访问反馈 IPC 需要逐次 `allow_once`。5 次权限请求分别对应首次 request/get、第二次 request/get 和最终 skip；权限卡显示命令、cwd 和 tool-call ID，每次只批准精确 QA 指令，没有全局放权或永久允许。第一轮 request/get 各有一次 `ipc_access_denied` 后授权重试，合计 7 条工具调用。该行为限定于本次适配器/平台/沙箱组合，不推断所有 Agent 都存在同样问题，也不证明无需权限交互的后台执行。

独立只读 verifier 退出 0、7 项断言全通过：

- 两轮保持相同 remote/run；初始轮次与两次 continuation 恰为 3 个持久轮次，各有 1 条用户消息、1 次成功终结、0 turn error。
- 每个请求仅一次 continuation 和 delivered attempt，最终回复含两个标记；工具重试不等于反馈重复投递。这是本次两个请求的观测结果，不扩大为普遍 exactly-once 保证。
- 7 条工具命令逐字符匹配验收 allowlist，cwd 均为隔离项目；最终 skip 仅一次。
- 两包 manifest、请求/宿主身份及 `feedback.md` / `uncooked.md` 哈希匹配；SQLite 完整性检查通过。

原生表格、音频及恢复操作和只读核验均已完成：

| 场景 | 实际结果 |
| --- | --- |
| 表格编辑 | 输入建议后双击选词，未保存的完整值保持；保存 r1，原值仍独立保留 |
| 音频播放与批注 | 无 autoplay；主动播放推进至 4.490 秒并到达结尾；0 ms / 8,000 ms 批注保存 r4 |
| 受控终止/重启 | 核对可执行文件路径后仅终止隔离进程树；同一程序/数据重启，精确表格建议和两条音频批注恢复 |
| 提交封存 | 表格最终 request r2 / manifest source r1；音频 request r5 / source r4；两者均 completed / feedback_submitted，编辑器封存 |
| 最小窗口 | 外窗约 962×542（约 960×540 客户区），位于工作区内，响应式堆叠布局的 Submit 可操作 |
| Agent 恢复状态 | 原 remote 绑定、原 run、31 条 activity ID 和两个 delivered attempt 保持；仅 3 条用户消息，无提示或 continuation 重放；recovery 为 interrupted、无 active turn |

工作台最终 verifier 退出 0、5 项全通过。两包原 spec、版本、manifest source/result、revision 关系及 manifest/Markdown 哈希一致。音频输入 base64、原 WAV、SQLite digest、manifest digest 与发布 WAV 的 SHA-256 全部相同，为 `fdbfe6c0d1eb055b12f837d74ab9af2d05c500fa4f07528e8e3a0dbac3b5a272`；发布文件为 `audio/wav`，256,044 字节，WAV 头时长 8,000 ms。

文件播放证明媒体传输、播放位置及 UI 状态，不证明麦克风、声卡输出质量或录音权限。Agent 重启验证涵盖绑定/历史保留和无重放，没有测试重启后再次向提供商发送新轮次。

## 证据索引与边界

以下为本地保留的报告文件名，未作为仓库附件发布；不将其写成外部可下载链接，也不在本文放入绝对用户路径、会话 ID、原始提示词、凭据或正文。公开可复查的工程证据为上述固定提交及 CI 链接。

| 范围 | 本地报告 |
| --- | --- |
| 初审与修复 | `RELEASE_REVIEW.md`、`commits.tsv`、`backend-fixes.md`、`frontend-fixes.md`、`scaffold-website-fixes.md` |
| CI、取消与权限 | `ci-456dfeb-evidence.md`、`ci-456dfeb-run.json`、`windows-quick-cancel-race.md`、`acp-permission-review.md` |
| 构建与原生步骤 | `source-native-package.json`、`native-ui-acceptance.md` |
| 升级与回退 | `native-source-data-upgrade-verification.json`、`native-upgrade-readonly.json`、`native-rollback-verification.json`、`native-rollback-verification.md`、`native-rollback-verification-initial.json` |
| Agent 与数据核验 | `native-readonly-verification.md`、`native-agent-round1-readonly.json`、`native-agent-final-readonly.json`、`native-agent-loop-verification.json` |
| 工作台保存/恢复/发布 | `native-workbench-save-readonly.json`、`native-workbench-final-verification.json`、`native-workbench-final-verification.md` |

本次没有物理 Apple Silicon Mac 的安装/升级和完整原生 UI、真实手机软键盘/触控/旋转/安全区、屏幕录制/麦克风权限或真实 SMB 会话验收。长时资源观察、同 GC 条件 retained heap 和全部媒体设备生命周期也不由这些结果关闭。历史质量记录保持原时点内容，不能据本次局部验收宣布 M5/M6 全部完成。

## 正式候选交付的独立判据

正式产物结论以 tag `v0.5.0-rc.1` 对应提交 `456dfeb16fdb02d2ce48c1399c1a417461652243` 的 [Release run 37114362576](https://github.com/l1veIn/rambledesk/actions/runs/37114362576) 完整成功、实际资产/签名核对和安装器报告为准，按[发布检查](../RELEASE_CHECKLIST.md)记录。源码原生可执行文件及旧版解包运行不能代替安装器安装证明。

Windows NSIS、对应 `.sig`、Apple Silicon DMG、`latest.json`、`SHA256SUMS.txt` 必须属于同一候选构建；RC 不要求 MSI，macOS 不提供 updater artifact。[Windows 安装器 smoke](../../scripts/windows-installer-smoke.ps1)的 hosted runner 报告须绑定候选提交及安装器哈希，证明稳定版真实安装 → 候选覆盖升级 → 卸载和合成文件保留；原生 UI 与数据库升级按其独立证据判断。

本记录是固定源码验收快照，不跟踪正式产物或公开发布状态；对应结果由该 Release 的构建/资产、安装器报告及交付记录承载。Windows updater 签名不等于 Authenticode，macOS ad-hoc 不等于 Developer ID 公证；未验实机及设备范围应在交付说明中继续明确。
