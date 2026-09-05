# Codeg ACP 源码借鉴地图

> 日期：2026-09-04；状态：固定版本的源码参考地图，另记本轮采用情况；未运行 Codeg 应用或测试。
> 关联：[ACP 提交地图](ACP_COMMIT_MAP.md)、[首轮 Codeg ACP 调研](CODEG_ACP_RESEARCH.md)。
> 术语仍以 [TERMINOLOGY.md](TERMINOLOGY.md) 为准。

## 本轮采用结果

RambleDesk 的 ACP 托管闭环已为 CURRENT，操作与实测支持见 [使用指南](ACP_MANAGED_SESSIONS.md)。
下方候选表保留调研时的取舍，不应把候选项整体视为已经实现：

- 已按本项目边界实现独占实例、进程树回收、有界脱敏诊断、权限队列、受会话约束的反馈命令、配置变化提示，
  以及 Svelte Agent 设置与工作区。活动落库后通过现有 invalidation + snapshot 恢复，未引入 Codeg 的 replay buffer。
- 持久 Request/Package/Delivery、发送结果不明状态、删除意图和 run/turn 恢复检查点使用 RambleDesk 的领域合同。
  `delivered` 的自动路径等到续接轮次成功结束；恢复失败保留绑定，不采用 resume/load 失败后静默 new 的回退。
- 动态会话配置、用户触发的安装管理、结构化 Chat、prepared 草稿与两种视图已在后续授权中实现；未引入客户端文件/终端执行、registry 市场、历史导入、连接池或 Codeg 的 UI/数据库。
- 后端兼容性以 [实机报告](ACP_BACKEND_PROBE.md) 为准。社区 `deepseek-acp@0.8.0` 和官方 dsh 的实测
  结论不来自 Codeg 的旧注释。旧 MCP/Pi 验收只作为历史证据；当前统一 command 的真实模型、Linux/macOS 与性能对照仍待验证，见[体验重设计计划](ACP_EXPERIENCE_REDESIGN_PLAN.md)。

这些是本地实现与验收结果；不声称 Codeg 的测试已经在 RambleDesk 运行，也不表示整文件移植。

## 本地参考库

- 已完整 clone `https://github.com/xintaofei/codeg.git`，本机位置为 `C:\Users\A\Desktop\codeg`，与 RambleDesk 并列。
- 本次 HEAD：`3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`，与首轮 ACP 调研一致；clone 后工作区干净。
- 它是独立参考库，不是 RambleDesk 的 submodule、workspace 成员或构建依赖。本次未安装依赖、未启动应用。
- 后续读取先确认参考版本；若更新上游，旧结论仍以本文件固定链接为依据，用 git history/diff 检查变化。

从 RambleDesk 目录可直接定位历史实现：

```powershell
git -C ../codeg rev-parse HEAD
git -C ../codeg show 3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1:src-tauri/src/acp/feedback.rs
```

## 候选总览

“提取移植”表示函数或局部状态机值得连同测试一起改造成我们的模块，不表示整个文件可以原样接入。
“行为复刻”表示沿用交互与失败处理语义，用 RambleDesk 的 Rust application contract / Svelte 实现。

| 内容 | 已看到的实现与价值 | 采用方式 | 对应步骤 |
| --- | --- | --- | --- |
| 启动与 Windows 兼容 | 可执行程序解析、无额外控制台窗口、stdio 分离、cwd、环境覆盖；vendored 启动器还处理 UNC + batch launcher。见 [process][process]、[AcpAgent][spawn]。 | 提取适用的小函数与测试；只保留首个后端需要的启动规则。 | 2、5 |
| stderr 与失败证据 | 有界尾部缓冲、UTF-8 安全截断、写入前脱敏、结构化协议解析错误摘要，避免“无输出但成功”无法诊断。见 [StderrTail][stderr]。 | 高优先级提取移植候选；诊断脱敏规则随我们实际协议字段调整。 | 2、5、6 |
| 进程资源收尾 | graceful disconnect、限时等待、进程树兜底；记录真实 spawn / reap，覆盖退出期间才生成 child 的竞态。见 [ChildGuard][child]、[disconnect_all][shutdown]。 | 复刻资源归属与测试场景，接入我们的独占实例 supervisor。 | 5、13 |
| 并发权限队列 | responder 与展示队列共用锁；多个审批不覆盖，回答后推进，取消统一 drain，重复响应幂等。见 [PermissionQueue][permission]。 | 高优先级提取状态机与测试；保持 ACP permission 与 Feedback Request 分离。 | 7 |
| MCP 工具注入与会话绑定 | 每次启动注册 token → parent connection / cwd，工具按 token 定位会话，断开时撤销；按启用功能暴露工具组。见 [注入入口][inject]、[TokenRegistry][tokens]。 | 复刻归属机制，结合现有 Local Integration Server；不必同时复制它的 UDS/named-pipe companion。 | 10、13 |
| 反馈交付提交时点 | 读 pending 不消费；准备响应后，成功写到 Agent 管道才触发 `after_relay` commit；取消时不 commit。见 [SpawnResult][relay]、[feedback contract][feedback]。 | 借鉴发送与确认分离；增加我们自己的持久 outbox 和结果不明状态。 | 11、12 |
| 会话事件与断线重连 | 事件序号、有界 replay buffer、同一状态锁内 snapshot + subscribe，缺口过大回退 snapshot。见 [event_stream][events]、[handle_attach][attach]。 | 提取缓冲/游标算法，接入现有 Application Transport；不再建立平行应用事件体系。 | 6、8、9b |
| 配置变更提示 | 比较运行实例启动时与现在的配置 fingerprint；更改后提示需要重新应用，恢复原配置清除提示。见 [staleness][stale]、[设置过期提示][stale-ui]。 | 行为复刻：保存配置不等于已影响运行会话；重连应用要以恢复能力为前提。 | 3、9a、9b、14 |
| 结构化检查与修复入口 | `CheckItem` 有检查 id、状态、说明和动作；bridge 与厂商 CLI 独立展示。见 [preflight][preflight]。 | DTO 形状和 UI 信息组织可借鉴；安装检查不能被显示为 ACP/反馈闭环验证。 | 2、9a |
| 会话级配置控件 | 按 `configOptions` 展示选择项、分组、当前值与说明；模型、模式等不全部硬编码在设置页。见 [session config selector][selector]。 | 复刻能力驱动的交互，用 Svelte 实现；只支持已实现/协商的选项。 | 6、9b |
| 恢复与历史重放 | 协商后尝试 resume/load，区分 replay 与 live，识别会话不存在、归档、被占用等错误。见 [恢复入口][resume]、[错误分类][load-failure]。 | 借鉴分类与重放去重；不采用自动用空白会话替换失败绑定的策略。 | 14 |
| 历史记录与吞吐隔离 | transcript 单线程顺序写、分会话缓冲与写入回执；lifecycle 只把需要落库的事件交给会话 worker，避免文本流堵塞关键事件。见 [transcript writer][transcript]、[lifecycle][lifecycle]。 | 借鉴高频流与持久事实分离；Request/Package/Delivery 不能采用队列满即丢弃。 | 6、11、14 |

## 最值得直接提取的三个单元

1. **权限队列及其不变量**。同一个 Agent Session 也可能同时申请多项权限，单个 `pending_permission`
   UI 槽位会覆盖早先请求。提取 `PermissionQueue<R>` 的 admit / resolve / drain 与并发、迟到响应测试，
   比从一个弹窗开始再补队列更稳妥。其锁顺序和发布时点也是合同的一部分，不能只拷贝容器字段。[源码][permission]
2. **诊断缓冲与字符串处理**。`StderrTail` 的容量上限、按轮次截取、先脱敏后截断及 UTF-8 边界处理相对独立。
   可以先用于 smoke probe，让首个后端失败时能给出有效证据。它的错误摘要字段白名单需要适配我们的 SDK，
   不能把“已有脱敏”理解为任意日志都可无条件展示。[源码][stderr]
3. **事件缓冲与游标测试**。`RecentEventsBuffer` 和 attach 决策可提取成较小的模块；最重要的是完整状态与
   订阅注册的时序关系。首期可先采用 snapshot 重建，有实际流量需求时再加增量 replay。[源码][events]、[attach][attach]

启动器/进程回收也值得移植，但平台和 SDK 耦合更强。Codeg 在 Cargo 中 patch 了 `sacp-tokio`；选择我们
的 SDK 时应比较上游与这些补丁，不因 Codeg 使用某版本就连同整份 vendored 实现和 unstable feature 一起引入。
其回收竞态测试部分限定 Unix，需要补自己的 Windows 验证。[依赖配置][cargo]、[进程回收测试][shutdown-tests]

## 一个需要前置的协议边界：谁执行文件与终端操作

Codeg 不仅发送 prompt，还能承接 ACP `fs/*` 与 `terminal/*` 回调。`HostToolsPolicy` 决定这些能力
是否宣告，以及收到调用后是否承接；文件服务与终端服务各有独立实现。[策略][host-tools]、[文件服务][fs]、[终端服务][terminal]

对本轮的建议是：步骤 2 就明确首个后端依赖哪些 Client capabilities，仅宣告已实现的能力。若后端自己执行
文件与命令，首期可以不接管这两类回调。若要接管，应单开完整切片处理目录、权限、输出限制、取消与进程清理；
不能认为 Agent 自己的进程沙箱自动覆盖由 RambleDesk 执行的操作。这不修改我们“外部 Agent 负责推理与工具
执行引擎”的定位，而是要求对可选的客户端执行服务做明确选择，不直接继承 Codeg 的默认策略。

## 不能照搬的四处语义

- **Live feedback 不是持久反馈工作流**。Codeg 的留言保存在 SessionState，新 UserMessage 会清空。
  `after_relay` 标记的是管道写入后的提交，也不是模型完成消费的业务确认。我们仍需持久 Request、Package、
  Delivery，以及写出后断线的结果不明处理。[状态更新][feedback-clear]、[relay 边界][relay-boundary]
- **恢复失败不能静默换身份**。Codeg 有 resume → load → new 的回退，并为部分自定义 Agent 连接历史。
  我们应保留原绑定与失败证据，不能把新的空白 Agent Session 当作原上下文恢复成功。[恢复与分类][load-failure]
- **业务事实不能随遥测一起丢弃**。Codeg transcript 的有界写队列满时会丢记录并记录日志，lifecycle 写入
  重试耗尽后也会继续。可以借鉴其隔离吞吐的思路，不能拿来保存我们的唯一反馈结果与待投递事实。[writer][transcript]、[lifecycle][lifecycle]
- **提取边界，不整体搬迁大文件**。本快照 `connection.rs` 有 21,254 行、`manager.rs` 有 8,176 行、
  `acp-agent-settings.tsx` 有 11,988 行（均含测试或辅助内容）。它们混合了大量后端特例、UI 与协议工作。
  Rust 小模块可改造移植，React 页面按交互复刻到 Svelte；数据库、历史导入、任务编排和安装系统继续按首期范围取舍。

## 把上游回归案例转成我们的验收用例

| 场景 | Codeg 阅读入口 | 我们的验收要求 |
| --- | --- | --- |
| 退出开始时还没拿到 child pid，宽限期内才 spawn | `disconnect_all_backstop_reaches_a_child_that_spawns_during_the_grace_window`，[manager tests][shutdown-tests] | 不能遗留本实例新生成的进程，也不清理其他会话。 |
| child 已被 reap，pid 不再有效 | vendored `acp_agent.rs` 的 ChildGuard tests，[源码][child-tests] | 清理不得继续使用过期 pid；退出通知与真实资源状态一致。 |
| 一会话多个权限请求、取消与回答交错 | `permission_queue_*` tests，[connection][permission-tests] | 所有请求都有归宿；没有不可点击的残留卡片或永久等待的 responder。 |
| 两个客户端回答同一权限 | `PermissionQueue::resolve`，[源码][permission] | 最多响应一次；迟到响应不推进错误的请求。 |
| 用户改配置又改回去 | `refresh_connection_staleness_flags_only_drifted_running_sessions`，[manager][stale-tests] | 仅对应配置的实例标记变化；还原后提示清除，不自动重启。 |
| 反馈读取取消、写出失败、错误 token 提交 | companion / listener feedback tests，[companion][relay-tests]、[listener][feedback-tests] | 不错误消费反馈，不跨会话提交；RambleDesk 额外验证重启后的持久恢复。 |
| snapshot 与实时事件交错，游标超出缓存 | [ws_attach][attach] 与 [event_stream][events] tests | 不漏当前状态、不重复追加内容，旧实例事件不覆盖新实例。 |
| 凭据跨截断边界、协议错误内含原始 payload | [stderr_tail tests][stderr-tests] | 摘要保留故障位置，避免回显凭据或原始内容，内存保持有界。 |

这些是源码中已有的案例与我们拟采用的验收要求；此次没有运行它们，不能据此宣称 RambleDesk 或 Windows 已通过。

## 移植记录约定

直接移植时在对应实现 commit 记录来源 commit、原文件/符号、保留测试和本地修改；随代码保留适用的许可证、
归属声明与变更说明，涉及 NOTICE 内容时一并处理。Codeg 根仓库为 Apache-2.0；vendored `sacp-tokio`
声明 MIT OR Apache-2.0，应单独追踪来源。[Codeg LICENSE][license]、[vendored manifest][vendor-license]、[Apache 条款][apache]

初次调研只增加参考地图；后续实现与测试按 [ACP 提交地图](ACP_COMMIT_MAP.md) 交付，当前采用范围见本文开头。

[process]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/process.rs
[spawn]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/vendor/sacp-tokio/src/acp_agent.rs#L451
[child]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/vendor/sacp-tokio/src/acp_agent.rs#L561
[stderr]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/stderr_tail.rs#L89
[shutdown]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/manager.rs#L2354
[permission]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs#L2333
[inject]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs#L4401
[tokens]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/delegation/listener.rs#L64
[relay]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/delegation/companion.rs#L326
[feedback]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/feedback.rs
[events]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/event_stream.rs#L91
[attach]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/web/ws_attach.rs#L123
[stale]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/manager.rs#L646
[stale-ui]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src/components/chat/session-config-stale-banner.tsx
[preflight]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/preflight.rs#L37
[selector]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src/components/chat/session-config-selector.tsx#L33
[resume]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs#L5282
[load-failure]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs#L8121
[transcript]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp_transcript.rs#L742
[lifecycle]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/lifecycle.rs
[cargo]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/Cargo.toml#L170
[shutdown-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/manager.rs#L3754
[host-tools]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/host_tools_policy.rs#L45
[fs]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/file_system_runtime.rs
[terminal]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/terminal_runtime.rs
[feedback-clear]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/session_state.rs#L1084
[relay-boundary]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/delegation/companion.rs#L846
[child-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/vendor/sacp-tokio/src/acp_agent.rs#L1077
[permission-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs#L13375
[stale-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/manager.rs#L3873
[relay-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/delegation/companion.rs#L2910
[feedback-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/delegation/listener.rs#L1983
[stderr-tests]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/stderr_tail.rs#L622
[license]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/LICENSE
[vendor-license]: https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/vendor/sacp-tokio/Cargo.toml#L32
[apache]: https://www.apache.org/licenses/LICENSE-2.0
