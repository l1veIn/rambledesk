# 工程门禁验收

> CURRENT：2026-09-10。最新交付检查见末节“Unix 凭据与浏览器重连的最终整合”；此前各轮按时间保留，不混合计数。本机门禁不等于远端 CI 或真实设备验收。

## 版本与环境

- 基线 HEAD：`b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce`（`v0.4.0-rc.3`），包含本轮未提交修改。
- 本机：macOS 26.3.1（25D2128）、arm64。
- Rust：`1.91.1`；Node：`22.23.0`；pnpm：`10.12.4`。
- 下述早期后端检查使用当时的 workspace `Cargo.lock`；Unix 凭据替换后的锁文件精简及最新检查见末节。
- 启动检查时的历史快照为 `/tmp/quality-final-provenance.json`；最终[源码清单](evidence/final-source-provenance.json)
  逐文件保存 103 个 tracked 修改或新增源码、测试和配置文件的哈希，避免 `git diff` 漏掉 untracked 文件。
  [最终核对](evidence/final-validation.json) 关联各次原始日志与门禁结果。产物按各自 Web/native 清单识别，当前工作区尚未提交。
- 较早运行的五份日志已另存为 `/tmp/quality-final-*` 快照，其 SHA256 记录在
  `/tmp/quality-final-prior-log-snapshots.json`，避免后续运行覆盖原始结果。

## 后端与协议结果

| 检查 | 结果 | 本机日志 |
| --- | --- | --- |
| `cargo fmt --all --check` | 通过 | `/tmp/quality-final-rust-fmt.log` |
| `pnpm check:rust-size` | 通过：250 文件，阈值 800 行 | `/tmp/quality-final-rust-size.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过，退出码 0（1m39s） | `/tmp/quality-final-clippy.log` |
| `cargo test --workspace --exclude rambledesk-desktop` | 39 个结果组：401 passed、0 failed、2 ignored | `/tmp/quality-rust-workspace.log` |
| `cargo test -p rambledesk-desktop --target-dir target/desktop` | 7 个结果组：130 passed、0 failed、0 ignored | `/tmp/quality-final-desktop-rust.log` |
| `cargo test -p rambledesk-feedback-client` | IPC 诊断修复后 10 passed、0 failed | [Agent 验收](AGENT_ACCEPTANCE.md) |
| `pnpm test:pi` | 22 passed、0 failed、0 skipped | `/tmp/quality-pi.log` |
| `pnpm test:dsh` | 28 passed、0 failed、0 skipped | `/tmp/quality-dsh.log` |
| `pnpm contracts:check` | feedback / host 生成合同与源码一致 | `/tmp/quality-contracts.log` |
| `pnpm mcp:inspector-smoke` | `request_feedback`、`get_feedback`、`cancel_feedback` 真正 Inspector smoke 通过 | `/tmp/quality-mcp.log` |

桌面 Rust 测试按仓库 `test:rust` 和 CI 约定使用独立 `target/desktop`，避免 Tauri 与后端 CLI 的产物相互影响。
上面的 workspace 401 条结果来自本轮较早运行，早于最后的 IPC 错误分类修改；后者已单独跑完整
feedback-client crate 的 10 条测试。两个计数有重叠，不相加后宣称为一次新的 workspace 全量结果。

workspace 的两条 ignored 具有明确含义：

- `process::tests::fixture_process` 是由其他测试监管启动的辅助子进程，不作为普通顶层用例执行。
- `real_catalog_install_inspect_and_initialize` 需要下载真实 npm 包，默认不执行。此次没有为了计数安装新 Agent。

MCP smoke 日志包含 Inspector 依赖树的 ink/react peer 版本警告；命令仍通过完整 smoke。
没有据此更改第三方依赖或忽略 Rust `-D warnings` 门禁。

## 早期前端整合记录（build4）

9 月 10 日 00:51:16 启动、持续 48.03 秒的该次整合检查，仍基于上述 HEAD 加本轮工作区：

| 检查 | 该次结果 | 本机日志 |
| --- | --- | --- |
| `pnpm --filter rambledesk-desktop test` | 203 files passed、1 live file skipped；1326 tests passed、1 live test skipped | `/tmp/quality-final-frontend4.log` |
| `pnpm --filter rambledesk-desktop check` | 0 errors / 0 warnings；最后独立类型检查 | `/tmp/quality-final-check5.log` |

该次全套包含 Browser beforeunload、Editor Undo/Redo、默认 continuation 中文 fallback、
保存广播刷新当前请求时保留编辑器，以及 Markdown 私有实例隔离和真实 Editor 生命周期回归。
唯一 skipped 为 [真实 Cooking 用例](../../apps/desktop/src/dev/cookingAcceptance.test.ts)：全量门禁未提供
opt-in 配置/fixture/output 三项路径，不会自动调用付费模型。其独立真实运行结果见
[Cooking 验收](COOKING_ACCEPTANCE.md)，不在这次全量通过数里重复计算。

首次整合运行仅失败于架构方向门禁：组合验收 harness 位于 `lib/workbench`，引入了新的
`workbench → shell` 依赖。将该组合入口移至 `src/dev` 后重新全跑通过，未放宽允许依赖边。
首次失败日志保留为 `/tmp/quality-final-frontend.log`。

00:00:51 的较早整合为 199 files / 1306 tests passed，另 1 live skipped，保留在
`/tmp/quality-final-frontend2.log`；对应类型检查为 `/tmp/quality-final-check.log` 的 0/0。
该次早于上述 UI 修复；00:17:11 的第三次全套为 201 files / 1318 tests passed、1 live skipped，
持续 64.71 秒，保留 `/tmp/quality-final-frontend3.log`。当时范围由 00:51 的第四次更新，
不把不同轮次计数相加。之后仅增加编辑结果 Export 的 harness 接线，并用上述 check 和独立 production
build 验证；1326 不被描述成该按钮的 UI 测试。Markdown 延迟及资源行为见
[性能验收](PERFORMANCE_ACCEPTANCE.md)，全量测试通过不代替 30 分钟资源结论。

### 第四次本机原生构建

`tauri build --debug --bundles app --config /tmp/rambledesk-quality-native.json --ci` 已完成，
日志为 `/tmp/quality-native-build4.log`，包含 `beforeBuildCommand pnpm build:web`、Rust 编译、
ad-hoc 签名，以及 `Finished 1 bundle`：`target/debug/bundle/macos/RambleDesk Quality.app`。
本轮构建包含上述当前生产代码修复。它是本机 macOS debug acceptance App，日志明确跳过公证；
不是发布签名产物、远端 CI、Windows/Linux 验证或自动完成的原生操作验收。
Native 保存后 Undo/Redo 等实际交互由 [实机记录](NATIVE_BROWSER_ACCEPTANCE.md) 单独登记，
不从构建成功推断。Web 性能冻结产物有独立哈希，不以它们替代这个原生 bundle 的身份。

### Browser 离页保护增量

真实 Chrome UI 发现：已保存的普通段落改为 H2 后立即刷新，尚未发送的 autosave 不会持久化，
重新进入仍显示旧 revision / 普通段落。因此在现有 App Browser Client 生命周期中增加标准
`beforeunload` 保护。事件发生时同步查询 DraftSession dirty 和 DraftController 的只读
`hasPendingSave()`；即使保存 A 时 Undo 回旧 saved O，也不会因为界面暂时显示 Saved 而漏掉在途保存。

只有 `environment=browser` 注册监听器；App dispose 移除。成功保存且没有本地差异/在途保存后不再阻止
离页。确认交给浏览器；用户选择离开时遵循其选择，不在 unload 中启动异步保存或承诺后台保存完成。
Tauri 的原生关闭/托盘合同未接入该监听器。

新增四条真实挂载 App + Tiptap 场景：H2 编辑和延迟保存、保存期间 Undo 回旧稿、失败后继续保护及
dispose 无残留、desktop 不注册。原实现先复现无保护失败；随后 App 和 DraftController 两文件共
17 条测试通过（00:11:59 启动），Svelte 类型检查 0 errors / 0 warnings。
JSDOM 仅补缺失的 Range geometry，编辑、键盘 Undo、保存队列和事件处理都走实际实现。

```sh
pnpm --filter rambledesk-desktop test src/App.feedbackFlow.test.ts src/lib/workbench/draftController.test.ts
pnpm --filter rambledesk-desktop check
```

日志：`/tmp/quality-browser-leave-red.log`、`/tmp/quality-browser-leave-tests2.log`、
`/tmp/quality-browser-leave-check.log`。这些用例已包含在 00:17 的 1318 条整合结果中，不额外加总。
Chrome 实际标准确认、选择留在页面及保存后刷新已另行验证，见 [实机记录](NATIVE_BROWSER_ACCEPTANCE.md)。
保存过程中 Undo 回旧稿的精确交错仍是实际 App 测试证据，不用 JSDOM 的 cancelable event 代替手工验收。

### 账本收口时的轻量检查

9 月 10 日再次执行下表，没有重跑重型测试或干扰正在进行的 UI 性能测量：

| 检查 | 结果 | 日志 |
| --- | --- | --- |
| Svelte / TypeScript | 0 errors / 0 warnings | `/tmp/quality-final-check5.log` |
| 前端模块体量 | 624 files、700 行阈值、1 个既有例外，通过 | `/tmp/quality-doc-audit-frontend-size.log` |
| Rust 模块体量 | 251 files、800 行阈值，通过 | `/tmp/quality-doc-audit-rust-size.log` |
| 术语与 core 边界 | 通过 | `/tmp/quality-ledger-terminology.log` |
| 架构依赖方向 | 2 tests passed；已计入全量覆盖，不追加测试总数 | `/tmp/quality-ledger-architecture.log` |
| Rust 格式与 diff whitespace | 通过 | `/tmp/quality-ledger-rust-fmt.log`、`/tmp/quality-ledger-diff.log` |
| 验收与支持文档链接 | 01:20 审计时点：16 份文档、246 个本地链接；文件、Markdown anchor 和明确内联源码路径均无缺失 | `/tmp/quality-final-document-audit.json` |

较早前端体量检查为 621 files，最新增加测量入口等文件后为 624；较早结果保留在
`/tmp/quality-ledger-frontend-size.log`。Rust 的 250 / 251 文件也是两个检查时点：长 Agent history fixture
增加文件后，最新体量复查为 251。
较早 workspace/clippy/desktop 结果的作用域不因此扩大。该 fixture 的独立 example 测试与真实 HTTP/SQLite
核对见 [性能验收中的夹具记录](PERFORMANCE_ACCEPTANCE.md#独立的长-agent-历史夹具)，不与 workspace 401 或 desktop 130 重复加总。
本次账本检查日志的 SHA-256 保存在 `/tmp/quality-ledger-evidence.json`；本地 `/tmp` 文件可能被系统清理，
作用域、计数与失败边界以本记录及工作区内的证据文件保留。

## 最终资源实测的独立边界

[完整 30 分钟原始导出](evidence/web-resources-final.json) 与
[独立统计](evidence/web-resources-final-analysis.json) 已保存：IAB 1280 × 720、31 快照、354 记录循环、
phase done、hidden=0；同一普通视图始终 1417 DOM 节点 / 365 元素 / 2 个 ProseMirror / 1 个可编辑反馈。
used heap 从 20437700 增至 28520562 bytes，增加 8082862 bytes，低于预登记
`max(10 MiB, 起点的 10%) = 10485760 bytes`；相对增长实际为 39.55%，不写成低于 10%。

这是 DOM 和结束增长数值项满足预算。较低采样点仍从第 11 分钟 23721438 增至第 26 分钟
26330715 bytes；完整低点序列不是严格单调，但未建立稳定 retained heap。低谷观察项仍保留，
需要同条件的后续可达对象／retained heap 证据，不能把数值阈值内当作无泄漏。
ObjectURL 与 Worker 都未创建，相关生命周期不作通过判断；录音、附件预览循环、App 销毁与原生内存
也没有被此运行覆盖。旧 v2 超阈值与作废探针记录保留；两轮 viewport、harness 输出和 App 不同，
不声称严格同比或纯产品归因百分比。该实测不增加全量测试计数，也不扩大跨平台结论。

## 工程通过与产品验收的边界

本机编译、lint、测试和合同检查证明受测代码与协议合同，不能替代以下证据：

- `.github/workflows/ci.yml` 对本轮交付 commit 的远端 Linux / Windows / macOS 运行结果；当前功能分支 push 不触发现有 CI。
- 原生窗口、系统截图/音频权限及尚未执行的设备输入行为。主任务已实际用 Native 和 Chrome 文件选择器
  选择 51 B TXT，Chrome 下载 JSON、Safari 显示预置 Markdown 附件的 H2；这些具体观察不等于整个平台全流程通过。
  包核对已存档为 [Chrome](evidence/chrome-download.json)、[Safari](evidence/safari-download.json)；
  [最新 Native 包核对](evidence/native-package-final.json) 已确认 ordinary 和 cancel 目的的请求均为 completed / r4，
  另一个 long 请求为 cancelled / r4，已有邻居的包保持不变。
  旧结果因离线 `immutable=1` 漏读 WAL 作废，详见下文；具体原生提交/重启手势见 [实机记录](NATIVE_BROWSER_ACCEPTANCE.md)。
- Windows 真机、手机软键盘/触控/旋转/safe area：用户明确没有相应环境，保持待验；macOS 或桌面窄视口不能代替。
- 图片粘贴和其余未覆盖的媒体、手势及焦点路线。Chrome / Safari 双客户端 CAS、重新认证和断线恢复已在 build6 实测通过，具体范围见[原生与浏览器记录](NATIVE_BROWSER_ACCEPTANCE.md)。
- 完整真实 Agent 三轮闭环。当前实际运行有明确失败，见 [Agent 验收](AGENT_ACCEPTANCE.md)。
- 最终资源观察的已挂载 DOM 数量与结束 heap 增长数值阈值已满足；heap 低谷变化仍待观察，
  未触发的 ObjectURL / Worker 生命周期未验。这不是无泄漏或完整资源门禁通过，详见 [性能验收](PERFORMANCE_ACCEPTANCE.md)。
- 制品签名、公证、发布、更新链路和真实安装升级。

Rust 体量阈值通过不代表所有模块设计已得到满分；测试计数也不是质量评分。
真实 HTTP/SQLite/文件包证据见 [反馈验收](FEEDBACK_ACCEPTANCE.md)，附件生命周期见
[输入验收](INPUT_ACCEPTANCE.md)。最终是否完成 M6，以 [质量计划](../PROJECT_QUALITY_PLAN.md)
列出的必验项目和对应记录为准，不使用工程绿灯覆盖产品边界的待验或失败。

## 验收脚本的 WAL 修正

Native 正常 Cmd+Q 后仍有已提交 WAL。原离线 verifier 的 `immutable=1` 忽略 WAL，错误地返回旧 seed。
现仅在已停止所有写入宿主且 WAL 不存在/0 字节时使用 immutable；非空 WAL 使用 `mode=ro`，在线读取
也始终使用 `mode=ro`，不主动 checkpoint。在同一已停止宿主的隔离库重验，未传 `--allow-unpublished`：ordinary 与 cancel 目的
request 均为 completed / r4，最终正文包含 `Native final toolbar verification.`，包正文和 manifest
哈希一致。旧数据保留为 [invalidated](evidence/native-seed-read-invalidated.json)，新证据为
[native-package.json](evidence/native-package.json)，不覆盖失败历史。

新增 [Python 回归](../../scripts/feedback-acceptance-test.py) 使用真实 SQLite 和实际 verify：旧代码
读到 r1 而非 WAL 中 r2 的断言先失败，修正后 2 tests 通过，覆盖非空 WAL 及已 checkpoint 的无 WAL /
空 WAL 两个子例，DB/WAL 字节及存在状态不变。另对旧 Q02 无 WAL fixture 和 Native 保留 WAL fixture
重新执行严格离线包核对，均通过。它属于验收脚本增量，不计入 Rust 或前端全量数字。
日志为 `/tmp/quality-offline-wal-red.log`、`/tmp/quality-offline-wal-final.log`、
`/tmp/quality-offline-stopped-fixture.log`、`/tmp/quality-native-offline-verified.log`。
此修复不修改产品存储代码，也不宣称原生退出必定 checkpoint。

## 验收 launcher 的日志清理边界

交付卫生复查发现 `feedback-acceptance.py stop` 可直接删除 manifest 任意指定的 `logFile`。
现在在 `load_manifest` 限定实际 launcher 的系统临时目录根、绝对路径、随机日志命名和普通文件类型，
拒绝 symlink；已删除日志可缺失，正常 launcher 和 `--keep` 保持兼容。

[Python 定向回归](../../scripts/feedback-acceptance-test.py) 只操作自身创建的临时文件，真实调用 stop，
不 mock 删除操作，也不接触正在运行的 fixture。旧代码的无关文件、错误目录、symlink 三个拒绝合同先红；
修复后全部 **7 tests passed**，含 5 个 launcher 测试及此前的 2 个 WAL 测试。
日志为 `/tmp/quality-launcher-log-green.log`，完整边界见 [反馈验收](FEEDBACK_ACCEPTANCE.md#2026-09-10日志清理路径增量)。
这是验收工具的独立增量，不扩大任何前端/Rust 全量或真实设备验收结论。

## Unix 凭据与浏览器重连的最终整合（2026-09-10）

用户明确授权本轮完成后 commit / push。工程代码的提交及完整交付账本见
[全局计划](../PROJECT_QUALITY_PLAN.md#8-实施账本与交付方式)。
[本次源码与门禁清单](evidence/delivery-validation.json) 关联实际文件哈希、日志与产物，
此前 `final-source-provenance.json` / `final-validation.json` 保留为 build4 阶段快照，不覆盖成 build6。

| 检查 | 本次结果 | 日志 |
| --- | --- | --- |
| `cargo test --locked --offline --workspace` | 一次完整运行含 desktop：46 组，544 passed / 0 failed / 2 ignored | `/tmp/quality-delivery-rust.log` |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | exit 0 | `/tmp/quality-delivery-clippy.log` |
| `cargo test --locked --offline -p rambledesk-desktop web_access` | 24 passed，含 7 个真实文件合同与 1 个真实 HTTP/SQLite 生命周期组合 | `/tmp/quality-web-credential-rust.log` |
| `pnpm test` | 204 files / 1330 tests passed，另 1 opt-in live skipped | `/tmp/quality-delivery-final-frontend.log` |
| `pnpm check` | 0 errors / 0 warnings | `/tmp/quality-delivery-final-check.log` |
| 前端 / Rust module-size | 625 / 254 文件通过；阈值 700 / 800，前端沿用 1 个既有豁免 | `/tmp/quality-delivery-final-frontend-size.log`、`/tmp/quality-delivery-final-rust-size.log` |
| terminology / core boundary | 通过 | `/tmp/quality-delivery-final-terminology.log` |
| `python3 scripts/feedback-acceptance-test.py` | 7 passed | `/tmp/quality-delivery-python.log` |
| `cargo fmt --all --check` / `git diff --check` | 通过 | 最终交付执行记录 |

Cargo.lock 从 856 个包减至 827 个：移除 29 个旧 Unix keyring 传递依赖，没有新增包，也没有改变保留包的
版本、来源或 checksum。新增 desktop 到已锁定 `libc` 的必要直接依赖；`tempfile` 已在原锁文件中。
Windows keyring 继续保留。这里的 macOS 构建与文件权限测试不算 Linux / Windows 实机验收。

真实浏览器测试发现了此前遗漏的重连问题：断线创建的 ready 等待被 `#connect` 再次重建，
离线期间加入旧等待的保存无法结束。删除重复创建后，同一轮连接由构造 / disconnect 创建等待，
成功、失败或撤销都会结束它。先前 3 条回归均失败，修复后增加真实 DraftController / DraftSession /
WorkspaceSession 与 transport 替换组合，共 4 条新增回归；相关 6 文件 76 条通过。
这没有引入通用操作自动重放，认证后的保存仍经过原来的草稿保存入口。
红绿日志分别为 `/tmp/quality-browser-reconnect-red.log`、`/tmp/quality-browser-reconnect-final.log`。

build5 / build6 均成功生成 debug/ad-hoc 的 `RambleDesk Quality.app`，`codesign --verify --deep --strict`
通过，仍未公证。build5 证明不再等待 Keychain，同时暴露保存恢复失败；build6 使用修复后的前端，
实际两浏览器保存恢复、双向 CAS、令牌轮换结果见[原生与浏览器记录](NATIVE_BROWSER_ACCEPTANCE.md)。
生产版本号保持 rc3；本轮未创建 release、tag、安装器或更新源。

现有 CI 仅在 push main / pull_request 触发；单独 push 当前功能分支不会产生新的运行。
交付时核对远端分支 SHA，不将旧运行或本机日志写成该提交的 CI 通过。Windows、真实手机、
正式安装升级及其余原生设备项目继续按计划待验。
