# M3 后端验收：持久化与生命周期

本记录对应 [PROJECT_QUALITY_PLAN.md](../PROJECT_QUALITY_PLAN.md) 的 Q10、Q11 后端部分。先复用已有覆盖，仅新增三个组合窗口的测试；这些受测场景保留了生产 Rust 实现。另一个 Q19 IPC 错误分类修复及真实 Agent 失败边界见 [Agent 验收](AGENT_ACCEPTANCE.md)，不混入本节结论。

## 受测版本与方法

| 项目 | 本次记录 |
| --- | --- |
| 日期 | 2026-09-09 |
| 基线 | `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce`，`v0.4.0-rc.3` |
| 工作区增量 | 本文、`deliveries/publication_replay.rs`、其 `mod` 声明、ACP process 新测试及 `protocol-init-hang` fixture、`managed_continuation.rs` 权限等待删除测试及 `managed_agent.mjs` 权限 fixture；均未提交 |
| 平台 | macOS 26.3.1（25D2128），arm64 |
| 工具 | rustc 1.91.1；Node v22.23.0 |
| 数据隔离 | 复用每例独立的临时 SQLite、资料目录、Node stdio fixture；不使用用户资料库 |
| 受测产物 | 上述工作区源码构建的 Rust test binaries；不是打包的 Desktop 安装产物 |

测试从仓库根目录运行。SQLite 测试使用真实事务、数据库重开与文件读写。managed runtime 的控制 driver 用于安排确定的异步顺序；ACP 与 local-server 集成另用实际 Node 子进程、stdio 和回环 HTTP。两类证据分别记录，不把控制 driver 当作真实 Agent。

## 已有覆盖地图

以下场景已存在，本轮保留实现并复用对应测试，没有再造平行测试框架。

| 风险窗口 | 已有证据与主要断言 |
| --- | --- |
| 多客户端写入、保存响应重试 | [requests.rs](../../crates/rambledesk-storage/src/sqlite/tests/requests.rs)：并发不同草稿只有一个 CAS 胜者；相同草稿重试收敛；request 身份重启后保持 |
| 发布、数据库终态、outbox | [publication.rs](../../crates/rambledesk-storage/src/sqlite/tests/publication.rs)、[deliveries.rs](../../crates/rambledesk-storage/src/sqlite/tests/deliveries.rs)：不可变包及 hash；文件已发布但 DB 未完成时重启恢复；delivery 插入失败后包与 outbox 一起恢复；批准与投递原子提交 |
| 发布与删除、清理失败 | [deletion.rs](../../crates/rambledesk-storage/src/sqlite/tests/deletion.rs)：持久化删除意图、文件失败可重试、清理后 DB 提交失败可恢复；排在删除之后的 publisher 不能重建旧包；目录链接不能越界清理 |
| prepared 首发与关闭 | [prepared.rs](../../crates/rambledesk-storage/tests/managed_runtime/prepared.rs)、[SQL prepared 测试](../../crates/rambledesk-storage/src/sqlite/tests/managed_sessions/prepared.rs)：首发/关闭只有一个生命周期胜者；同时首发不能写入两条 human message；晋升与首条消息原子回滚/重试；重启扫除 prepared 不影响 active |
| 启动中退出、旧 turn/取消回调 | [managed_runtime.rs](../../crates/rambledesk-storage/tests/managed_runtime.rs)、[session_recovery_runtime.rs](../../crates/rambledesk-storage/tests/session_recovery_runtime.rs)：owner shutdown 中断启动并拒绝新工作；旧 run/turn 与延迟取消不能关闭替代实例或后续 turn；重启记录一次 interruption，不隐式启动 |
| 投递完成写失败、结果未知 | [delivery_completion.rs](../../crates/rambledesk-storage/tests/delivery_completion.rs)、[deliveries.rs](../../crates/rambledesk-storage/src/sqlite/tests/deliveries.rs)：完成落盘失败重试同一 attempt；结果未知后不重发；退出未写完成态由启动恢复成 uncertain；旧 attempt 不能覆盖新 attempt |
| 真实进程、权限、邻实例隔离 | [process/tests.rs](../../crates/rambledesk-acp/src/process/tests.rs)、[runtime.rs](../../crates/rambledesk-acp/tests/runtime.rs)、[user_input.rs](../../crates/rambledesk-acp/tests/user_input.rs)：EOF、超时、Drop、初始化失败、关闭失败/无响应均回收所属资源；权限/input 只消费一次；取消旧 turn 后新 turn 可用；另一实例保持连接 |
| HTTP + stdio + SQLite 的组合 | [managed_continuation.rs](../../crates/rambledesk-local-server/tests/managed_continuation.rs)：空闲后原会话续接一次；Agent 读反馈后断开留下 uncertain，禁止盲重放；删除 turn 尚未结束的会话时清理自身并保留邻会话；新 runtime 能完成持久化删除意图 |

## 新增的最小缺口

### Q10：成功结果未被客户端观察，重启后两客户端重试

[publication_replay.rs](../../crates/rambledesk-storage/src/sqlite/tests/deliveries/publication_replay.rs) 新增 `lost_submit_response_replays_after_restart_without_republishing_or_reenqueuing`。

先通过真实 FeedbackApplication 完成提交，再通过真实 delivery repository 将同一 attempt 标为 Delivered，模拟后端已经完成而客户端没有收到提交结果。关闭数据库后，创建两个独立连接池，并发用旧 revision 重试，其中一个还携带不同的 cooked 内容。

预期与实际一致：两个调用都返回原终态；Markdown/manifest 字节和 SHA-256 保持不变；原 Delivered 记录及 attempt 保持不变；没有新增发布计划或 pending delivery；邻会话的待投递记录保持不变。该测试把此前分散的幂等、重启、包和 delivery 合同串在一起。

证据边界：在 application/repository 层丢弃调用结果，不制造真实 TCP 丢包，也不实际驱动 Agent 消费该条新测试的 delivery。真实 HTTP/stdio 断开后的 uncertain 行为由上表已有组合测试提供。

### Q11：实际 initialize 在途时取消 connect future

[process/tests.rs](../../crates/rambledesk-acp/src/process/tests.rs) 新增 `cancelled_initialize_reaps_its_tree_and_keeps_neighbor_connection_usable`；复用 [fixture.mjs](../../crates/rambledesk-acp/src/process/fixture.mjs)，仅增加 initialize 到达但不回复的模式与到达标记。

先建立邻 ACP 连接，再启动带有子进程的第二个 Node Agent fixture。确认 initialize 已实际到达后中止其 connect task，验证根进程与子进程都停止。随后邻连接仍能执行 `session/new`，并正常 shutdown、回收自身进程树。

预期与实际一致。这补充了原先只有显式初始化错误、超时和 owner Drop 的窗口：调用者直接取消尚未返回的 connect future。等待 initialize 到达使用文件握手，避免仅靠 sleep 猜测取消时机。

证据边界：本例直接取消真实连接 future；SessionApplication 的“启动中 owner shutdown”由既有控制 driver 测试验证。两者构成分层证据，不声明本例已覆盖打包 Desktop 的退出事件接线。

### Q11：确实等待权限时删除，旧回复与邻会话隔离

[managed_continuation.rs](../../crates/rambledesk-local-server/tests/managed_continuation.rs) 新增 `deleting_while_waiting_for_permission_rejects_late_answers_and_keeps_neighbor_usable`，复用 [managed_agent.mjs](../../crates/rambledesk-local-server/tests/fixtures/managed_agent.mjs) 的真实 Node / ACP / 私有反馈通道夹具。

两个独立会话都由 Node 发出 `session/request_permission`。测试等待各自的 interaction 到达，断言双方实际为 WaitingInput 后才删除第一会话，不用固定 sleep 猜测权限窗口。两个进程使用同一个 provider wire id，但应用层权限 id 独立。

实际结果符合预期：被删除会话从 SQLite 消失；旧 UI 权限回复返回 SessionNotFound；将旧权限 id 误指向邻会话也返回 InvalidInput。邻会话仍保持同一个 instance、连接与待处理权限，其权限可以通过原 stdio 通道正常批准。随后邻会话继续请求反馈、发布反馈，并完成一次 Delivered 续接，证明反馈 scope 也没有被误撤销。

此例补齐了冻结计划中“等待权限时删除”的精确交错，不能与原先的 busy turn 删除例混同。它验证真实 runtime、stdio、SQLite 与反馈通道组合；底层 OS 进程树回收由同记录中的 process 组另作直接观察。本次通过，没有修改生产删除或权限逻辑。

## 本次执行

| 命令 | 结果 |
| --- | --- |
| `cargo test -p rambledesk-storage --lib --test managed_runtime --test session_recovery_runtime --test delivery_completion` | 111 + 15 + 4 + 3 条通过；0 失败 |
| `cargo test -p rambledesk-acp --lib process::tests` | 9 条通过；0 失败；1 个子进程入口按设计 ignored，由其他测试单独启动 |
| `cargo test -p rambledesk-acp --test runtime --test user_input` | 5 + 5 条通过；0 失败 |
| `cargo test -p rambledesk-local-server --test managed_continuation` | 最终 5 条通过；0 失败；包含新增权限等待删除场景 |
| `cargo clippy -p rambledesk-storage -p rambledesk-acp --all-targets -- -D warnings` | 通过 |
| `cargo clippy -p rambledesk-local-server --test managed_continuation -- -D warnings` | 通过 |
| `rustfmt --edition 2024 --check crates/rambledesk-acp/src/process/tests.rs crates/rambledesk-storage/src/sqlite/tests/deliveries/publication_replay.rs` | 通过 |
| `node scripts/check-rust-module-size.mjs` | 通过；250 个 Rust 文件，限制 800 行 |
| `git diff --check -- crates/rambledesk-acp/src/process crates/rambledesk-storage/src/sqlite/tests/deliveries.rs` | 通过 |

三个新增场景各自先定向运行通过，随后包含在上表相关组中；不将重复执行计作新增覆盖。

相关组累计 157 条不同测试通过，0 失败；其中首次为 156 条，追加精确权限等待删除后为 157 条。另有 1 个按设计 ignored 的子进程入口。这是修改范围对应的本地验证，不是整个 Rust workspace 或平台 CI 矩阵的重跑。

## 剩余验收边界

- Windows/Linux 的进程回收实现本次未在对应系统执行；保留现有平台 CI 与实际发行环境的待验状态，不能从 macOS 通过推导跨平台通过。
- Node fixture 证明 stdio 协议和资源隔离，不证明某一真实 Agent、bridge 或模型版本可用；真实接入多轮验收仍属 Q19。
- Q09 前端 prepared 生命周期接线、输入保留和关闭视图行为不在本分支修改范围；接口稳定后仍须做对应 App 组合复验。
- 本次没有测量性能、真实设备、Tauri 窗口退出或安装升级；这些是计划中其他验收层级。本记录不把 Q10/Q11 后端通过等同于 M3 或全项目验收完成。
