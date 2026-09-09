# 隔离反馈验收入口

> CURRENT：Q02 的可重复 HTTP / SQLite 夹具。它提供测试设施，不增加 headless、LAN 或新的产品启动方式。

## 证明范围

`feedback_acceptance` 使用真实 `SqliteFeedbackStore`、`ApplicationChangeHub`、
`ApplicationCommandFacade`、`WebSessionManager` 和 Web Access server。
它沿用 [Web Access server 测试](../../crates/rambledesk-local-server/tests/web_access_server.rs)
与 [managed preview](../../crates/rambledesk-local-server/examples/managed_preview.rs) 的装配方式。
这个更小的入口只创建外部反馈请求，不启动 ACP、Local Integration listener、Tauri、模型或设备。

每次运行创建全新的系统临时目录和 SQLite，反馈包与附件也只写在该目录内；端口由 IPv4 loopback
listener 随机分配。无选项可以接入日常数据库。凭证随机生成，只写临时文件，不输出到日志。

浏览器从指定 dist 按需读取真实构建产物。HTML、JS、CSS 均使用 `NoStore`，新建的独立 HTML
验收入口也能按原路径访问。可以重建后刷新；性能比较必须固定一份 dist，记录变化后的新证据。
不要把此处的无缓存测试配置当成生产启动、缓存性能或安装包性能证据。

## 启动、浏览器操作与停止

需要仓库既有 Rust / Cargo、pnpm 和 Python 3.9+；Python 脚本只用标准库。先在仓库根目录构建前端：

```sh
pnpm --filter rambledesk-desktop build:web
python3 scripts/feedback-acceptance.py start --keep
```

`start` 运行 `cargo build --locked -p rambledesk-local-server --example feedback_acceptance`，
使用 workspace 的 Cargo.lock 与 target，不创建临时 Cargo 项目、不改依赖版本。
第一次构建可能下载锁定依赖；不要把下载缺失或构建锁等待误报成产品失败。
`--no-build` 可复用已构建的 example，manifest 会明确记录该限制。

启动输出包含 `url`、`manifestFile`、`database`、`tokenFile`、`stopFile`、PID 和四条 seed 的 id。
打开 URL，将 `tokenFile` 中的测试凭证填入现有 Web Access 认证界面。不要把凭证放进截图或验收记录。
每条 seed 使用独立外部 Host Session；要选择对应的 Session，才能看到它的请求。

| seed | 内容与用途 |
| --- | --- |
| ordinary | 二级标题、两段中英文结构化草稿；编辑、自动保存、刷新恢复、提交 |
| long | 同一标题结构加 240 段，约 48 KB document JSON / 35 KB Markdown；长文档输入与切换 |
| attachment | 一份请求端 Markdown 附件、一份反馈端文本附件及 Editor 中的 attachmentFile 引用 |
| cancel | 独立可编辑请求；用于取消和终态确认 |

`document_json` 与 Markdown 同时通过真实保存 API seed，初始 revision 与内容哈希写入 manifest。
请求/附件 UUID 每次不同；普通与长文档的文字和段数固定。若需要图片粘贴、原生文件选择或截图，
在浏览器/原生验收中实际操作，不能用这里的文本附件替代。

停止时复制本次输出的 `manifestFile`，不要沿用上次运行的路径：

```sh
python3 scripts/feedback-acceptance.py stop /absolute/path/to/acceptance.json
```

脚本创建本实例的停止 marker，server 正常关闭 listener 与 SQLite；不按端口搜索或杀死其他进程。
读取 manifest 时也校验 launcher 日志：必须是当前系统临时目录根下、符合 launcher `mkstemp`
命名的绝对路径，且不能是符号链接或其他非普通文件。错误路径在任何停止/删除动作前被拒绝。
默认运行停止后删除临时数据与 launcher 日志。`--keep` 保留 DB、包、manifest 和日志，删除已失效的
credential 文件，并把状态标为 `stopped`。保留的目录可以在完成取证后自行删除。
前台直接运行 example 时也支持 Ctrl+C；强杀进程不是正常清理或重启验收。

## 自动验收与包核对

先启动一个新的实例，再用其 manifest 执行。`smoke` 会修改并终结全部四条 seed；不要对正在用于
浏览器手动检查或性能测量的实例执行。

```sh
python3 scripts/feedback-acceptance.py smoke /absolute/path/to/acceptance.json --output /absolute/path/to/http-results.json
python3 scripts/feedback-acceptance.py stop /absolute/path/to/acceptance.json
```

`smoke` 通过真实 HTTP 读取 generation，编辑结构化草稿并保存，证明过期 revision 返回冲突，
提交普通/长文档/附件请求并重放提交，最后取消独立请求。随后 `verify` 核对：

- HTTP workspace 与 SQLite 的正文、结构化文档、revision 和终态一致。
- 终态 revision 与包的 source/draft revision 一致。
- 磁盘 manifest 的 SHA-256 与 SQLite 一致；正文、原稿、两类附件的大小/哈希符合 manifest。
- 认证后的 published projection 与磁盘包内容一致；DTO 合同允许省略的 null/default 字段作明确归一化。
- 外部夹具不会产生托管 feedback delivery。取消依旧有持久终态包，不被误判为“不得返回外部终态”。

`verify` 是只读检查，不改草稿、不提交、不重试操作。尚未发布时可核对 seed 与当前草稿：

```sh
python3 scripts/feedback-acceptance.py verify /absolute/path/to/acceptance.json --allow-unpublished
```

对 `--keep` 且已正常停止的实例，确认其他使用这份临时数据库的宿主也已退出后，可以离线核对 SQLite 和包：

```sh
python3 scripts/feedback-acceptance.py verify /absolute/path/to/acceptance.json --offline --output /absolute/path/to/offline-results.json
```

离线读取的前提是所有写入宿主已经停止。存在非空 WAL 时使用 SQLite `mode=ro`，读取仍保留的已提交
记录；此时不能使用 `immutable=1`，否则会把提交错读为旧 seed。只有 WAL 不存在或为 0 字节时，才使用
`mode=ro&immutable=1`，兼容不能在不创建 sidecar 的情况下读取已 checkpoint 数据库的 SQLite 构建。
在线核对始终使用 `mode=ro`。验证器不会主动 checkpoint，也不假定 Native Cmd+Q 后 WAL 必然消失；
把仍在写入的库交给 `--offline` 不符合调用前提。
这证明停止后数据仍可读，不等同于进程崩溃后的恢复验证。失败返回非零；没有任何已发布包且没传
`--allow-unpublished` 也返回非零，避免把“只启动了服务”当作闭环通过。每条请求的终态仍须查看结果记录。

WAL 回归可独立运行，不启动服务或模型：

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/feedback-acceptance-test.py
```

两个测试覆盖非空 WAL，以及已 checkpoint 后 WAL 不存在和 0 字节两个子例，均实际调用 verify，并
断言最新文档/revision 正确、DB/WAL 字节及存在状态不变。非空 WAL 用例把 revision 2 只提交到 WAL；
旧实现实测返回 revision 1，修复后通过。真实 Native 留存库的旧 seed 读数保留在
[已作废记录](evidence/native-seed-read-invalidated.json)，以 [WAL 正确读取结果](evidence/native-package.json)
为准：ordinary 与 cancel 目的的请求均实际完成，revision 4，正文与包哈希核对通过。

## 核对真实浏览器下载

现有浏览器下载是 `REQUEST_ID.rambledesk-feedback.json`，包含 manifest、markdown 和可选原稿，
不是服务器目录或 ZIP。完成实际下载后：

```sh
python3 scripts/feedback-acceptance.py verify /absolute/path/to/acceptance.json --request-id REQUEST_ID --download /absolute/path/to/REQUEST_ID.rambledesk-feedback.json --output /absolute/path/to/browser-results.json
```

核对实际导出中的正文/原稿字节及 manifest 语义，并继续验证原始磁盘 manifest 的持久哈希。
只运行 HTTP smoke 不证明浏览器下载手势、TipTap 输入、焦点或移动键盘；这些必须另记真实 UI 操作。

## 复用到性能基线与其他 HTML 入口

```sh
python3 scripts/feedback-acceptance.py start --keep --dist /absolute/path/to/frozen-production-dist
```

manifest 记录 dist 目录与 index 哈希、受测 HEAD、tracked diff 哈希、工作树状态、example 源码哈希、
binary 哈希和构建方式。未跟踪文件不包含在 tracked diff 哈希中。独立性能入口和它的依赖产物应由
性能记录再固定哈希；重建 dist 或更换测试二进制后重新建立记录。

较长 Agent 历史需要另用 managed preview / Agent 夹具，本入口的外部反馈请求不伪造 ACP 历史。
真实模型、设备、安装升级与手机真机的验收仍依照
[支持矩阵](../WEB_ACCESS_SUPPORT_MATRIX.md) 和 [全局质量计划](../PROJECT_QUALITY_PLAN.md)。

## 2026-09-09 本地验证

在 macOS Apple Silicon、Python 3.9、workspace locked Rust build 上完成以下检查：

| 检查 | 结果 |
| --- | --- |
| example 编译；独立 tempDB 和随机 loopback 启动 | 通过 |
| 四条 seed 的真实 HTTP / SQLite 读取及初始内容哈希 | 通过 |
| HTTP 编辑、CAS 冲突、三条提交及幂等重放、一条取消 | 通过 |
| 包正文/原稿/manifest/两类附件的持久哈希 | 通过 |
| 正常停止后离线核对；保留构建来源；删除测试凭证 | 通过 |
| 默认停止清除本实例目录与 launcher 日志 | 通过 |
| 故意损坏夹具包正文后检查必须失败 | 通过，检测失败后已恢复测试文件 |
| 指定独立 dist 的 `quality-benchmark.html` | HTTP 200，响应与该 HTML 文件字节一致 |

这些是夹具及 HTTP/持久化入口的验收结果。浏览器、性能、设备与模型的通过状态分别记在对应记录中。

## 2026-09-10：日志清理路径增量

交付卫生检查发现，原 `stop` 直接使用 manifest 的 `logFile` 删除文件，未像 database/stopFile/tokenFile
一样约束路径。用测试自行创建的临时 victim 和已停止的隔离 manifest，直接执行真实 `stop`：旧实现接受
普通无关文件、临时根以外的同名日志、日志符号链接，三个拒绝合同均为红灯；没有对工作区、用户文件或
正在运行的验收实例执行复现。

现在 `load_manifest` 接受 launcher 原本生成的 `rambledesk-feedback-acceptance-<8 位随机名>.log`，
同时核对临时目录根和文件类型。macOS `/var` 与 `/private/var` 的系统目录别名仍可用；已经清理的日志
允许不存在，`--keep` 仍保留合法日志。既有无 `logFile` 的离线 manifest 继续适用于只读验证。

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/feedback-acceptance-test.py
```

此次定向结果：**7 tests passed**，日志 `/tmp/quality-launcher-log-green.log`。其中 5 个 launcher 测试
覆盖错误位置/名称、symlink 拒绝、正常清理、keep 和已清理日志；原 2 个 WAL 验证测试保持通过。
测试不启动服务或模型，不 signal 任何实例，只删除自身生成的临时日志。
