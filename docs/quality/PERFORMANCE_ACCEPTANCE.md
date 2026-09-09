# Production Web 性能与资源验收

> CURRENT：初始与最终 reload / switch 已保存；真实 Chrome 60 次编辑保存、长历史四项各 30 次均达到预登记预算。Markdown tokenizer 累积已通过真实依赖回归及原场景复测。最终 30 分钟资源运行完整结束：DOM 稳定、结束 heap 增长在预登记数值阈值内；heap 低谷变化继续保留观察，未触发的 ObjectURL / Worker 生命周期仍未验，不宣称无泄漏。

## 范围与受测对象

[性能入口](../../apps/desktop/quality-benchmark.html) 动态加载真实 `main` / App，使用
[隔离 HTTP / SQLite 夹具](FEEDBACK_ACCEPTANCE.md)。它不导入 App 的 store 或私有 TipTap 实例，
不替换 transport，不向业务层注入 benchmark 数据。所有导航和编辑通过现有 DOM 完成。

受测普通草稿与 240 段长草稿来自同一固定 seed。这个反馈入口没有较长 ACP 历史、多托管页签、
附件预览开关或真实录音流程。长文档测量也不等于「含附件的长文档」测量；夹具的附件是另一条请求。
这些计划场景必须分别登记，不可由这里的结果替代。长历史另见本文末尾独立 managed_preview quality 夹具。

构建和启动方式：

```sh
RAMBLEDESK_QUALITY_BENCHMARK=1 pnpm --filter rambledesk-desktop build:web
python3 scripts/feedback-acceptance.py start --keep --dist /absolute/path/to/frozen-production-dist
```

先将本次构建完整复制到独立 frozen dist，再把它传给 launcher。按启动输出完成认证，使用英文，
结束 onboarding，保持 ordinary 与 long 两条请求在请求列表中可见，打开同源的
`/quality-benchmark.html`。直接使用产品首页不会加载 instrumentation。入口只在显式 opt-in build 中出现。

每份证据需附 launcher manifest、完整 dist 文件清单及哈希、HEAD / working diff 信息、设备和浏览器版本、
viewport、测试起止时间。单一 `App-*.js` 文件可能只是动态导入 shim，不能把其字节数称为整个 App bundle。
初始证据的 `frozenApp.bytes = 59` 就是这种 shim；性能文件名/该哈希只定位它自身，完整产物须另存清单。

## 预先固定的预算

以下新增预算在本轮正式运行前写入 harness，与原有 reload / switch 预算一起保存在输出 JSON。
它们是本机 Web 场景的验收目标，不是跨设备性能保证。P95 使用排序后第 `ceil(0.95 × n)` 项，
median 使用常规定义。30 个样本的 P95 是第 29 项；必须同时保留原始样本、最大值和失败状态。

| 指标 | 数量 | 预算 / 调查条件 |
| --- | --- | --- |
| 新文档 reload 到可编辑 DOM + 两帧 | 5 | median ≤ 1500 ms |
| 普通请求切换到正确编辑 DOM + 两帧 | 30 | P95 ≤ 300 ms |
| 240 段请求切换到正确编辑 DOM + 两帧 | 30 | P95 ≤ 500 ms |
| 普通正文编辑到 marker 可见 + 两帧 | 30 | P95 ≤ 100 ms |
| 长正文编辑到 marker 可见 + 两帧 | 30 | P95 ≤ 200 ms |
| 普通正文编辑到新 Saved revision 可见 + 两帧 | 30 | P95 ≤ 2000 ms |
| 长正文编辑到新 Saved revision 可见 + 两帧 | 30 | P95 ≤ 3000 ms |
| 同一普通视图热身基线 → 结束静置时的 ObjectURL outstanding 增量 | 30 分钟 | ≤ 0；若确有创建活动，核对逐分钟稳定性及 create/revoke 差额 |
| 同一视图的 Worker withoutObservedTerminate 增量 | 30 分钟 | ≤ 0；只适用于该页面实际创建、显式 terminate 的 Worker |
| 同一视图的 attachedEditorViews / editableFeedbackViews | 逐分钟及结束 | 全部 ProseMirror 相对热身基线增量 ≤ 0；可编辑反馈 DOM 为 1。只读 Markdown preview 也可能有 ProseMirror，不要求总数为 1 |
| 同一视图的 App 元素 / 全 DOM 节点增量 | 30 分钟 | ≤ 0；若不符，先解释真实可见内容差异再定位残留 |
| 可用时的 JS heap 增长 | 逐分钟及结束 | 结束相对基线超过 `max(10 MiB, 10%)` 触发复测；持续增长的低谷也需调查，不自动判断为泄漏 |

保存预算包含产品原有 autosave debounce、真实 HTTP / SQLite 保存以及 UI 更新。输入可见时间包含两帧，
不能称为 INP、纯 TipTap transaction 时间或键盘硬件延迟。堆内存预算是调查线；未强制 GC，
浏览器缓存、垃圾回收时机和 harness 自己保留的样本都会影响数值。

同条件 median 恶化超过 10% 时先复测排除噪声，确认后解释并处理；10% 不替代上面的绝对预算。
初始 frozen harness 没有新增资源追踪和编辑模块，新增入口加载成本也进入 reload 时间。
因此新旧 reload 差异不能全部归因于 App；需要严格归因时，应让两个 App 版本使用同一 harness 再比较。
没有测得问题时不进行拆包或其他性能优化。

## 三个相互独立的操作

### Reload / 请求切换

点击 `Measure fixture: 5 reloads + 60 switches`。入口通过 sessionStorage 跨五次 reload 恢复进度，
随后完成 30 次 long 和 30 次 ordinary 切换。完整结果在只读 `#quality-results`
（accessible name `Benchmark results`），也可点击 export 下载。

reload 从该次 document 的 navigation start 计时。认证与 onboarding 在测量前完成，服务端以 NoStore
读取固定 dist，但浏览器 cache 状态未控制。因此不能把它命名为 OS 冷启动、原生启动，
也没有分别证明稳定的 cold / hot cache 性能。DOM 正确且再经过两帧是完成判定；后台节流不属于目标场景。

### 30 分钟资源观察

展开资源区域，点击 `Start 30-minute resource observation`（`#quality-resource-run`）。
不会自动开始或在 reload 后自动续跑。先打开长、普通两条请求，静置 3 秒，在普通页建立资源基线；
之后每约 5 秒切换一次，每分钟回到普通视图，记录同一视图的快照。30 分钟后回普通页静置 3 秒，
记录最终快照。目标为起点、分钟 1–29 和终点共 31 个样本；实际时间戳和间隔写入 JSON，
若浏览器延迟导致错过分钟，入口不会伪造补齐的样本。

整个运行在同一已认证文档进行。保持该文档前台，不手动编辑、关闭请求或切换 locale。
`#quality-resource-stop` 可以提前结束；提前结束是 `stopped`，不能算作完成 30 分钟。
页面隐藏次数进入结果，发生过隐藏则结束状态为 `done-with-visibility-interruption`，应复测后再用于预算结论。
reload 会将上次未完成结果显示为 `interrupted`；单独启动新的运行。

默认模式在运行中只更新短进度，结束或停止后把所有分钟快照和 view cycle 延迟样本一次性写入
只读 `#quality-resource-results`（accessible name `Resource observation results`）。点击
`Export resource results`（`#quality-resource-export`）下载完整 JSON，也可正常读取 textarea 的 `value`。
不要从折叠摘要推断已经完成。中途 reload 会丢失尚在内存中的完整样本，不能拿 compact checkpoint 冒充原始结果。

资源追踪在动态 `import('../main')` **之前**安装：

- 包装当前 window 的 `URL.createObjectURL` / `revokeObjectURL`，记录已创建且没有观察到 revoke 的 URL。
  export benchmark JSON 自身创建的 URL 不计入 App 数目。初始 App import 之后的累计活动和热身后的增量都可见。
- 代理当前 window 的 Worker 构造，并包装原型上的 `terminate`。只保留 WeakMap 身份和数字 ID，
  不强引用 Worker 对象。`withoutObservedTerminate` 是字面观测值，不能称为操作系统真实存活 Worker 数。
- 按分钟采样可用时的 Chromium `performance.memory`；不可用则明确输出 `null`。
- 数 `#app` 下的所有节点（含文本/注释）、元素、`.ProseMirror` 与可编辑反馈 DOM，排除 harness 面板 DOM。

Worker 自行 `close()`、Worker 中创建的 Worker、其他 realm、SharedWorker、订阅、真实 ASR、原生进程、
已经脱离 DOM 但仍存活的 editor、浏览器外的内存都没有被这里观测。计数一直为 0 只能证明此切换流程没有
创建被监测对象，不能宣布该资源的完整生命周期通过。最后仍有一个可编辑反馈 DOM 是运行中 App 的预期状态；只读预览可以另外持有 ProseMirror DOM。
该流程没有退出整个 App，因此不证明 App 销毁时的全部释放合同。

### 真实编辑 → 保存结果

展开编辑区域，点击 `Append + save: 30 ordinary and 30 long edits`（`#quality-edit-run`）。
必须使用独立夹具，或在所有不改变正文的测量结束后运行；每条草稿会增加 30 个唯一 marker。
它不会静默清理 marker 或重置数据库；再次比较未编辑 seed 时创建新夹具。

每个样本先等待现有 `Saved · rN`，通过标准 DOM Selection 将光标定位到末段，然后调用浏览器
`execCommand('insertText')` 完成实际 contenteditable 编辑。它不是手写 `.textContent` 再派发假 input。
若浏览器拒绝编辑、没有真实观察到 input 事件、marker 未出现、保存版本不增长，或切换回来后 marker 丢失，
此轮直接失败。输入事件类型及 trusted 标志写入样本；不使用私有 editor API 或业务 store 兜底。
该 browser editing API 有兼容性限制，必须由受测浏览器实际成功执行才能得到结果。

样本记录 input 到可见 marker、input 到新保存 revision、前后 revision 和重新打开后 marker 存在。
这证明产品编辑 DOM 与可见保存状态的组合行为；结果本身不包含 SQLite 文件字节核对，后者使用夹具 verify
单独取证。它也不模拟逐字键入、IME composition、真实触摸键盘或用户拖选。
完整结果在只读 `#quality-edit-results`（accessible name `Edit and save benchmark results`），
点击 `Export edit results`（`#quality-edit-export`）下载同一 JSON；导出使用探针保存的原始 URL API，
其临时下载 URL 不计入 App 资源。该按钮只用于显式导出，不会自动发送结果。
发生页面隐藏时同样标记受干扰状态。三个测量按钮互斥，避免资源切换与写入采样互相改变目标。

## 已有结果与本轮状态

[初始 Web 原始样本](evidence/web-baseline.json) 来自 macOS 26.3.1 / arm64 Apple Silicon，
Codex in-app Chrome 152，931 × 865 viewport，`b7ab9d08 + Q01 working changes`，
固定产物在 `/tmp/rambledesk-quality-baseline-dist`，manifest 为 `rambledesk-feedback-acceptance-v1`。
这份临时目录不保证未来仍存在；仓库内 JSON 是已保存的样本证据。

| 初始样本 | n | median | P95 | max | 本机预算 |
| --- | --- | --- | --- | --- | --- |
| reload | 5 | 133.2 ms | 149.8 ms | 149.8 ms | 通过 |
| 普通切换 | 30 | 49.95 ms | 50.4 ms | 50.8 ms | 通过 |
| 长文切换 | 30 | 49.8 ms | 50.6 ms | 51.9 ms | 通过 |

本轮新增 harness 的 `pnpm --dir apps/desktop check` 为 0 errors / 0 warnings，
`node scripts/check-frontend-module-size.mjs` 与 `git diff --check` 通过。
这些是实现检查。最终 reload / switch、实际编辑 60 次与长历史结果已另存并在本文登记；
30 分钟资源按独立结果判断，不把尚未观察的资源或输入流程列为通过。性能修复有先前实测和红灯证据。

## 探针修订记录

最终 Chrome reload / switch 已保存为 [完整原始结果](evidence/web-final-switch.json) 与
[统计和边界](evidence/web-final-switch-analysis.json)。受测 `/tmp/rambledesk-quality-final-dist` 的
viewport 为 1200 × 820，5 次 reload median 149.9 ms / max 164.5 ms；30 次普通切换 median
49.9 ms / P95 60.1 ms / max 66.4 ms；30 次长文切换 median 49.85 ms / P95 65.7 ms / max 66.6 ms。
三个预登记绝对预算均通过。初始 IAB 基线为 931 × 865，harness/产物也不同，且 cache 未控制，
因此不按「同条件恶化 10%」比较，不从数值差异宣称产品改善或回归。该旧 reload/switch driver
没有记录 visibility transitions，原始 JSON 不能提供 hidden=0 的数值证明。

编辑 Export 在后续独立 `/tmp/rambledesk-quality-edit-export-dist` 构建加入，
[产物与新夹具 manifest](evidence/web-edit-export-provenance.json) 已保存。实际 App implementation
与 frozen-final 同为 `App-C89e5ByT.js` / 1654948 bytes / SHA-256
`2027319a79a770ebc83a084c02a7e01c794bae588613b30ae87fcebb23471860`；只修改 HTML 与 dev harness，
没有覆盖正在进行资源观察的冻结目录。新建反馈数据库供编辑采样，原资源数据库保持不变。

第一次资源观察约 172 秒、4 个样本 / 33 次循环后停止并保留原始记录。其第 2 分钟读到临时的
732 个节点 / 0 个编辑器，而稳定 ordinary 视图约 1417 个节点。这是探针在已处于 ordinary 时
重复发起导航，随后对旧 DOM 过早完成稳定判断造成的无效取样，不能作为产品资源回归证据。

`stable-view-v2` 不对已匹配的目标视图重复 click，等待条件会在连续帧重新检查，失配后重新累计稳定帧。
每次资源取样前再次等待 ordinary 和唯一可编辑反馈 DOM，并同步断言后才入样；超时或失配直接失败，
不默默记录 0。所有 `.ProseMirror` 另作总量观察，与同视图基线比较，不把只读 Markdown preview
误计为第二个可编辑反馈实例。旧 172 秒数据作废，30 分钟验收从修订后的新冻结构建重新开始。

## 独立的长 Agent 历史夹具

[managed_preview quality 模式](../../crates/rambledesk-local-server/examples/managed_preview.rs)
通过 [确定性 seed](../../crates/rambledesk-local-server/examples/managed_preview/quality.rs)
向真实 `SessionActivityRepository::append_activity` 写历史，再由现有 SessionApplication 和 HTTP 接口读取。
这不是 preview transport，也不绕过分页逻辑。它不发送模型 prompt；展示文字反复明确标识 synthetic fixture。
原有 local Node ACP fixture 仅提供协议连接，工具记录没有执行真正的文件读取或模型操作。

```sh
cargo build --locked -p rambledesk-local-server --example managed_preview
RAMBLEDESK_MANAGED_PREVIEW=1 RAMBLEDESK_MANAGED_PREVIEW_QUALITY=1 RAMBLEDESK_MANAGED_PREVIEW_DIST=/absolute/path/to/frozen-production-dist target/debug/examples/managed_preview
```

不设置 `RAMBLEDESK_MANAGED_PREVIEW_QUALITY=1` 时保留原有普通 preview 行为。
`RAMBLEDESK_MANAGED_PREVIEW_DIST` 可选，默认使用 `apps/desktop/dist`；性能取证时必须传独立 frozen dist。
没有参数可以接入日常数据库：每次都创建新的系统临时目录、SQLite 和独立项目子目录，Web 与本地 listener
使用随机 IPv4 loopback 端口。

质量模式输出 `PREVIEW` URL、`PREVIEW_QUALITY_MANIFEST` 和 `PREVIEW_TOKEN_FILE`。
凭证仅写本次临时文件，Unix 权限为 0600；质量模式不把凭证打印到日志。打开 URL 后使用该文件中的凭证认证。
manifest 包含 session id、数据规模、sequence、真实序列化活动哈希、dist、数据库、tokenFile 和 stopFile 路径。
测量前复制 manifest、记录本次 frozen dist / binary / source 哈希，避免只保留即将被清理的临时路径。

`Website project` 有 60 个固定 turn，每轮五条：user message、thought、typed tool call、agent message、status，
总计 300 条；既有 `CLI project` 作为相邻会话保留，用于切换。种子包含中英文、Markdown 标题/列表/代码块、
可折叠 thought，以及带 input / output / content 的 completed 工具详情。时间戳固定，文本结构固定；
session id 每次隔离生成，因此整份持久活动哈希会随实例身份变化，不能要求跨实例哈希相同。

本次实测种子为序号 1–300，完整序列化大小 344086 bytes；正文和 typed content 同时保存，不能将该字节数
等同于 HTTP 压缩传输或 DOM 大小。UI 初次获得最近 100 条，另外两页各 100 条可取到完整历史。
UI 仍有按 turn 展示窗口和默认折叠，数据库有 300 条不意味着同时挂载 300 个 activity DOM。

后续浏览器采样在正式测量前固定以下预算：进入/重新打开该长历史 P95 ≤ 500 ms；实际加载一页早期历史并可见
P95 ≤ 750 ms；展开一个已加载 turn 的过程或其工具详情 P95 ≤ 200 ms，每类至少 30 次。
保存原始次数、最大值与前后 DOM 标识，记录本地已加载数据与真实 HTTP 页的区别。
打开和切换必须确认标题与目标 session；分页必须确认更早的 turn/activity 真正出现，不能只等按钮恢复。
展开可利用现有 `data-turn-id`、`aria-expanded`、`data-turn-process` 和 `data-tool-id` 判定内容真正挂载。
折叠/展开状态可能保留，点击前应读取状态；已打开的详情不能被误计为一次新的展开。

若需要重复相同的第一页条件，应在每轮重新建立该会话的 UI history 状态，并验证起始窗口一致；
不要在全部历史已经加载时反复点击或把仅展开本地窗口称为 HTTP 分页。本节的夹具验证只证明 seed 与读取合同，
实际 30 次 UI 结果在后文单独登记；它们均不推断真实 Agent 生成速度。

停止只创建本次 manifest 的 `stopFile` 或向该前台实例发送 Ctrl+C。现有退出流程关闭 SessionApplication、
Web server、本地 server 和 SQLite，随后删除整个临时目录；不要按端口寻找或终止其他实例。
因此需要保留的 manifest、结果和截图必须在停止前复制到证据目录。

本机验证（2026-09-10）：

- `cargo test --locked -p rambledesk-local-server --example managed_preview`：1 passed。既有真实 Node 配置/活动测试
  增加了 SQLite seed、最近 100 条快照、按 20 turns 向前分页、300 条无重复且完整的断言。
- 旧 test 的 usage=4608 断言已按现有单次缺失 handoff 提醒修为 5120；fixture 不 handoff 时会收到两次 prompt。
  这是既有测试期望修正，没有改变生产 driver 或把展示 fixture 当成真实反馈交付测试。
- 单独启动质量实例后通过认证 HTTP 读取 100 + 100 + 100 条，序号 1–300、唯一 id 300、typed tool 60；
  所有 HTTP 字段和 content 与只读 SQLite 查询逐条一致。
- 创建 stopFile 后进程正常退出，临时目录（SQLite、凭证、manifest）全部删除。
- [HTTP / SQLite 夹具原始证据](evidence/managed-history-fixture.json) 仅证明以上夹具行为；真实 UI 性能结果另存。

### Managed history: opt-in DOM sampling

The separate [history driver](../../apps/desktop/src/dev/qualityManagedHistoryBenchmark.ts) adds an independent
button to the existing quality HTML; it does not change Vite inputs or the ordinary/resource drivers.
Build with `RAMBLEDESK_QUALITY_BENCHMARK=1`, freeze a separate history dist, and pass it to the managed preview
quality mode above. Never overwrite a dist used by an ongoing resource observation.

After authentication, use English and open the Website project and CLI project scopes, then use each
existing `View Agent` action to create their actual `Website project · Agent` and
`CLI project · Agent` workspace tabs. The `dsh`-labelled scope tabs are different surfaces and do not qualify.
Expand `Managed history · separate quality fixture`, then click `#quality-history-run`.
The complete JSON is in readonly `#quality-history-results` (accessible name `Managed history benchmark results`);
`#quality-history-export` downloads the same JSON. This run is manual and never sends an Agent prompt.
All benchmark runs share the same mutual exclusion guard.

Each iteration switches through the neighboring Agent tab, so actual keyed ManagedSessionSection navigation
recreates the history owner. Entry timing waits for the correct session id, heading, selected tab, and exactly
20 user/Agent turns numbered 041-060. The latest process is explicitly collapsed before its measured expansion;
completion requires both `data-turn-process` and the typed tool to mount. The tool is also collapsed before
measurement; completion requires the actual Input section and synthetic output text. These are real DOM clicks,
not private controller/store writes.

Pagination starts only from that exact recent100 DOM window. After clicking `Load earlier messages`, the driver
requires exactly 40 user/Agent turns numbered 021-060. A readonly PerformanceObserver must additionally observe
exactly one new `/api/application/listManagedSessionActivity` fetch with `transferSize > 0`, and status 200 when
responseStatus is available. The driver does not issue requests itself or replace fetch. Each accepted page
stores before/after activity and turn IDs, resource path, start time, duration, transferred bytes and status.

If older content appears without adequate network evidence, it is retained as `unverifiedPage`; pagination
becomes `unmeasured` or `partial`, and further pagination samples stop. Entry and expansion samples may continue.
If a reopened view cannot restore the exact initial window, or a required DOM transition times out, the run
fails with existing raw samples preserved. Cached/local expansion is never recorded as an HTTP page.
Only 30 completed entries, process expansions, tool expansions and proven HTTP pages yield `phase: done`.
Visibility interruptions receive a separate phase and require a clean repeat for budget conclusions.

Implementation checks: Svelte/TypeScript 0 errors and 0 warnings; frontend size and diff checks passed.
The operator must save actual browser results separately. These checks do not stand in for the 30 UI samples.

Preflight revision: `managed-history-v2-agent-view-preflight` requires actual Agent tabs and matching
managed session heading/session-id DOM. Missing/ambiguous Agent tabs or failure before the first complete
entry are `setup-required`, not product performance failures. The observed `dsh`-labelled tabs were Ramble
scopes with a `View Agent` action; they cannot substitute for actual Agent history tabs.

## 2026-09-10 实际长历史结果与待定位项

[Chrome 下载的完整原始结果](evidence/managed-history-ui.json) 已完成四项各 30 个样本，
`phase=done`、`hiddenTransitions=0`，viewport 为 1200 × 820。30 次分页均有真实 HTTP 200、
115072 transferred bytes，并逐次验证相同 session 的 041–060 起点与 021–060 结果窗口。
这证明测量流程和真实分页完成；**进入及分页延迟尚未达到预登记预算**。

| 指标 | n | median | P95 | max | P95 预算 | 判定 |
| --- | --- | --- | --- | --- | --- | --- |
| 进入长历史 | 30 | 558.55 ms | 950.2 ms | 1000.2 ms | 500 ms | 未达 |
| 展开过程 | 30 | 50.0 ms | 50.9 ms | 50.9 ms | 200 ms | 达标 |
| 展开工具详情 | 30 | 49.9 ms | 50.6 ms | 50.7 ms | 200 ms | 达标 |
| HTTP 分页后内容可见 | 30 | 798.75 ms | 1403.7 ms | 1515.3 ms | 750 ms | 未达 |

分页 HTTP resource duration 本身 median 11.05 ms、P95 15.4 ms、max 16.1 ms。
可见时间的前 5 次 median 为 263 ms，后 5 次为 1360.3 ms；进入前 5 / 后 5 次 median 为
200.9 / 639.9 ms。过程和工具展开约 50 ms，未呈现同样增长。由此只能将调查重点放到客户端
及探针开销，不能直接断言后端慢或某个产品组件泄漏。

[分析与受测产物哈希](evidence/managed-history-analysis.json) 固定了
`/tmp/rambledesk-quality-history-dist` 的 HTML、driver、App implementation、import shim、bootstrap 和 Vite manifest。
实际 driver 是 `managed-history-v1`；受测 App 为 `assets/App-Bjox7Vhr.js`，1653011 bytes。
该 frozen Web build 不包含后续 UI/source 细节修改，不能把此次测量作为最终 Desktop / release binary 的验收。
原始 JSON 数值保持原样。

[初始 Web 基线](evidence/web-baseline.json) 也已补 `frozenAppImplementation`：
`assets/App-DgPhunIe.js`，1654830 bytes，SHA-256
`225c91151871e9822a710e0d86f76766ddeb31c6a26a3b4ee64ab1d0ecbd8c2f`。
原 `frozenApp` 的 59-byte shim、文件名和哈希全部保留并标明角色，未更改任何 reload / switch 样本。
两个 implementation chunk 都另外依赖其他 chunk，以上数值不是完整客户端下载总量。

### 输出开销的受控复测

旧 history/resource driver 会在循环中反复 stringify 完整结果、同步写 sessionStorage，并把不断增长的
JSON 放入可见 textarea。history 还将 page 同时序列化在 pages 和 cycles 中。该开销必须与真实 App
行为分开验证；下述 A/B 证明移除输出后仍有增长，随后用真实依赖测试定位到产品解析器的注册累积。

`managed-history-v3-output-control` 与 `stable-view-v3-output-control` 默认只更新小型进度状态和 compact
checkpoint；完整原始对象保留在内存，结束或停止后才一次性写入 readonly textarea 和 sessionStorage。
history cycle 使用 pageIndex 引用，避免将相同 page 重复写入最终 JSON。
reload 只能恢复 compact checkpoint，不能恢复尚未输出的完整在途样本；它会明确显示中断。

同一新 frozen build 的 `?quality-output=live` 保留实时完整 JSON 输出作为显式对照，默认 URL 为
`outputMode=final-json`。两种模式必须各自从 fresh document、相同 fixture 和 viewport 起跑。
该变量只改变探针输出，未修改产品、HTTP 或 history controller。真实浏览器由主线程通过 CUA 执行，
结果保存于 [final-json](evidence/managed-history-final-output.json) 与
[live-json](evidence/managed-history-live-output.json)，[统计](evidence/managed-history-output-control-analysis.json)
保留原始文件哈希。两次均为同一 frozen App、1200 × 820、30 次完整流程、hidden=0。

| 输出模式 | 进入 median / P95 | 分页可见 median / P95 | 进入前 5 / 后 5 median |
| --- | --- | --- | --- |
| 仅结束输出 | 358.5 / 567.6 ms | 328.05 / 546.5 ms | 150.7 / 550 ms |
| 每轮输出 | 466.65 / 999.7 ms | 459.55 / 963.7 ms | 184.6 / 967.3 ms |

仅结束输出依然超过进入预算，并呈逐轮增长，因此输出不是充分解释。两次顺序测量没有控制所有系统负载，
不能把差值精确称为 stringify 的耗时；后续默认保留仅结束输出，以减少测量干扰。

### 已复现的 Markdown 注册累积及修复

`feedbackEditorExtensions.ts` 原先每次转换都创建新的 TipTap `MarkdownManager`，编辑器的 Markdown
扩展也会创建 manager；默认均使用 marked 的全局单例。TipTap 注册自定义 tokenizer 时调用
`marked.use()`，闭包绑定该 manager，销毁 Editor 不会从全局解析器删除它们。

[真实依赖回归测试](../../apps/desktop/src/lib/feedbackEditorExtensions.lifecycle.test.ts) 修复前三条均失败：

- 20 次 parse + serialize 使全局 block tokenizer 从 0 增至 120，inline 从 0 增至 80。
- 再创建并销毁 20 个真实 TipTap Editor，block 从 120 增至 180，inline 从 80 增至 120。
- 一次 `breaks: true` 转换后，下一次默认转换错误地产生 hardBreak，证明全局选项也串扰。

现为每个 Editor 的 Markdown 扩展、每个短期转换 manager 注入独立 `Marked`。桌面显式依赖
`marked@17.0.6`，与已锁定的 TipTap 间接依赖相同，没有升级解析器版本。单个局部类型适配说明了
TipTap 接口声明要求 callable singleton，但实现只使用 Marked 支持的实例方法。
测试同时验证多个 Editor 不共享 parser、存活邻实例的 tokenizer 数量不因其他 Editor 创建/销毁而增长。

定向验证：`pnpm --dir apps/desktop exec vitest run src/lib/feedbackEditorExtensions.lifecycle.test.ts
src/lib/feedbackEditorExtensions.test.ts src/lib/agents/chat/activity-markdown.test.ts
src/lib/editor/MarkdownPreview.test.ts src/lib/editor/RichFeedbackEditor.test.ts`，5 files / 18 tests 通过；
`pnpm --dir apps/desktop check` 为 0 errors / 0 warnings。这证明注册与语义问题已在调用路径修正；
完整浏览器延迟和 30 分钟 heap 是否恢复，仍以修复后原场景复测为准。

复测冻结产物 `/tmp/rambledesk-quality-final-dist` 与三个相互隔离的 SQLite 夹具已登记
[完整产物清单、哈希和 manifest](evidence/web-final-provenance.json)。资源夹具只切换，编辑夹具可变更正文，
长历史夹具只读 synthetic 活动；它们共用同一只读 dist，避免编辑改变资源 run 的固定内容。
该 Web 产物与最终原生包分别验收，不互相替代。

### 修复后真实 Chrome 长历史复测

[完整下载原始结果](evidence/managed-history-parser-fixed.json) 与
[统计、哈希和逐页证据核对](evidence/managed-history-parser-fixed-analysis.json) 均已保存。
同为 `managed-history-v3-output-control` / `final-json`、1200 × 820 viewport，
`phase=done`、hidden=0，四项各 30 次，预算在测量前保持不变。

| 指标 | median | P95 | max | P95 预算 | 判定 |
| --- | --- | --- | --- | --- | --- |
| 进入长历史 | 100.4 ms | 117.9 ms | 118.6 ms | 500 ms | 达标 |
| 展开过程 | 50.0 ms | 50.7 ms | 50.8 ms | 200 ms | 达标 |
| 展开工具详情 | 49.85 ms | 50.2 ms | 50.8 ms | 200 ms | 达标 |
| HTTP 分页后内容可见 | 74.65 ms | 83.2 ms | 84.1 ms | 750 ms | 达标 |

进入前 5 / 后 5 次 median 为 116.4 / 100.1 ms，分页为 77.6 / 73.5 ms，
本轮不再呈逐轮上升。30 次分页逐一保持起点 041–060、结果 021–060、同一 session、
HTTP 200 和 115072 transferred bytes；HTTP duration 自身 median 14.7 ms / P95 16.2 ms。

相对修复前相同 final-json 输出模式，进入 P95 从 567.6 降至 117.9 ms（观察降幅 79.2%），
分页可见 P95 从 546.5 降至 83.2 ms（84.8%）。两个产物还包含同期同请求刷新修复，
所以这些是候选版整体的实测差异，不能当作单独 parser CPU 节省比例。独立的真实依赖红绿测试
已经证明 tokenizer 累积和换行选项串扰消失，浏览器原场景复测则证明其长期变慢症状未再次出现。
该结论仅限上述 30 轮 Web 场景；30 分钟资源与原生、真机、真实 Agent 生成仍分别验收。

### 最终真实 Chrome 编辑与保存

[Chrome 导出的原始 60 次样本](evidence/web-final-edits.json)、
[既有 live HTTP / SQLite verifier](evidence/web-final-edits-store.json) 与
[本次独立统计及逐 marker 存储核对](evidence/web-final-edits-analysis.json) 已保存。
viewport 为 1200 × 820，`phase=done`、hidden=0；普通与 240 段长文各 30 次，四个预算均通过。

| 指标 | median | P95 | max | P95 预算 |
| --- | --- | --- | --- | --- |
| 普通正文编辑到 marker 可见 | 31.9 ms | 32.7 ms | 32.8 ms | 100 ms |
| 长文正文编辑到 marker 可见 | 31.5 ms | 32.9 ms | 32.9 ms | 200 ms |
| 普通正文编辑到 Saved revision 可见 | 748.65 ms | 749.4 ms | 749.6 ms | 2000 ms |
| 长文正文编辑到 Saved revision 可见 | 764.2 ms | 765.3 ms | 765.5 ms | 3000 ms |

60 个样本各观察到一个 `insertText` / `isTrusted=true` input，全部在切换离开并重开后保留 marker。
两份草稿均逐次从 before revision 1–30 到 after revision 2–31，没有跳号或伪装同一保存结果。
这些事件来自浏览器编辑命令，不能因此声称真实键盘、IME 或触摸输入已经测试。

另开只读、正常读取 WAL 的 SQLite 连接（`mode=ro`，`query_only=ON`，显式读事务），仅查询
ordinary / long 的 drafts 行。二者最终均为 r31，document 哈希与前一份 HTTP / SQLite verifier 一致；
每份草稿的 30 个完整 marker 均在结构化正文 text 节点和 `body_markdown` 中各出现一次，
无缺失、重复或串入另一份草稿。分析 JSON 保存全部 60 个 marker 的逐项计数与 document/body 哈希。
这一步未查询或修改后来用于其他验收的 cancel 请求，也未使用 `immutable=1` 漏掉在线 WAL。

原 verifier 的 `sourceHead=null` 保持原样：该夹具直接启动已有 binary，不能凭空补成 commit-only 归属。
本次以 [edit-export provenance](evidence/web-edit-export-provenance.json) 的完整 frozen Web 清单，
及 [final provenance](evidence/web-final-provenance.json) 的 fixture binary 哈希关联实际产物；
再次读取 App 和 binary 字节确认哈希一致。身份是基于 HEAD 加未提交工作区的冻结产物，
不是证明某个 Git commit 单独重建即可得到这些字节。

### 首次有效 30 分钟资源观察

[完整数值投影](evidence/web-resources.json) 保留 stable-view-v2 的 31 次快照；353 次切换完成，
全部快照 visible，隐藏次数为 0。每次普通视图均为 1417 个节点、365 个元素、两个 ProseMirror
（一个只读预览和一个可编辑反馈），说明这个固定视图没有已挂载 DOM 数量增长。

JS heap 从 33832895 增至 70649895 bytes，净增长 36817000 bytes，超过预登记调查线。
这份测量没有 GC 后 retained heap 或订阅数量证据，并且仍带上述实时 JSON 输出开销，
该旧 run 的数值项确实超过调查线，原始记录不改为通过，也不能仅凭这些采样认定产品泄漏；
后续修复与独立复测结果见下文。
ObjectURL 和 Worker 均为 0 次创建：该流程未触发它们，生命周期不作通过判断。

### 最终 30 分钟资源复测：数值阈值内，低谷观察仍保留

[完整导出原始数据](evidence/web-resources-final.json) 为 60986 bytes，SHA-256
`b22de355c52b1a96dde01d39c762f7a3f3f364ee2df00006ad76c9e8bc92b25b`；
[独立分析](evidence/web-resources-final-analysis.json) 核对全部 31 个快照和 354 个记录循环。
本次为 IAB Chromium、1280 × 720、`stable-view-v3-output-control` / `final-json`；
`phase=done`、hidden=0，全部快照 visible，基线到最终快照间隔 30 分 3.159 秒。
循环延迟 median 55.45 ms / P95 63.7 ms / max 113.4 ms；这些附带样本不代替独立 30 次请求切换测量。

| 观察项 | 全部快照 / 最终结果 | 判定 |
| --- | --- | --- |
| App DOM 节点 / 元素 | 始终 1417 / 365 | 同视图增长预算通过 |
| 全部 ProseMirror / 可编辑反馈 | 始终 2 / 1 | 已挂载 DOM 数量预算通过 |
| Used JS heap | 20437700 → 28520562 bytes，增加 8082862 bytes（约 7.71 MiB） | 结束增长数值阈值内 |
| ObjectURL | created / revoked / outstanding 全为 0 | 未触发，生命周期未验 |
| Worker | created / explicitlyTerminated / withoutObservedTerminate 全为 0 | 未触发，生命周期未验 |

结束 heap 增长约为起点的 39.55%，**不是小于 10%**；预登记条件取
`max(10 MiB, 起点的 10%) = 10485760 bytes`，本次 8082862 bytes 未超过该数值调查线。
运行中最高 used heap 为 30029847 bytes。DOM 恒定只证明已挂载节点数稳定，不证明脱离 DOM 的对象已释放。

低谷观察不能随数值预算一起关闭：第 11 分钟的较低快照为 23721438 bytes，第 26 分钟为
26330715 bytes，相差 2609277 bytes。所有局部低点和六个五分钟窗口最小值都已写入分析，
它们并非严格单调上升，但也没有建立稳定的 retained heap 下界。采样未控制 GC，不能把这些下降点
标成已确认的 GC 后存活内存；既不能据此断言仍有泄漏，也不能证明没有泄漏。
按预登记的「持续增长低谷需调查」要求，保留这一观察项，后续需同条件的可达对象／retained heap
证据或更长观察来区分缓存、测量对象和实际残留。当前只关闭已满足的 DOM 与结束增长数值项，
不把整个资源生命周期验收写成完成，也不凭未定位的增长继续做优化。

旧 v2 的超阈值结果与早先作废的探针结果全部保留。旧有效 run 为 931 × 865、循环中输出完整 JSON，
新 run 为 1280 × 720、仅结束输出；App、harness 与系统运行条件也不同。
因此不计算严格同条件或纯 parser 的 heap 改善百分比。此运行没有录音、附件预览循环、订阅计数、
App 销毁或原生进程内存证据，这些范围保持各自边界。
