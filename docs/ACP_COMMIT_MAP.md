# ACP 托管会话提交地图

> 状态：计划；本文件随已完成步骤更新，不表示所有目标均已实现。
> 分支：`codex/acp-managed-sessions`；从更新后的 main `367eb09` 开始。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)；决策：[ADR 007](adr/007-acp-managed-sessions.md)。

分支按正式迭代维护，以合并 main 为目标。每步一个完整 commit；单步仍过大时继续拆分，不能为了表格
编号凑成大提交。每步包含该能力需要的合同、migration、生成类型与相关验证，测试不集中拖到最后。
未完成的产品入口不对用户宣称可用，既有反馈适配器保持可用。

| 步骤 | Commit 主题 | 完整结果与验收边界 |
| --- | --- | --- |
| 1 | `docs: define managed session terminology and architecture` | **术语表更新是核心交付**：厘清会话/任务/轮次/请求、反馈适配器/会话管理、ACP Client/Bridge/Instance；同步宪章与 ADR，记录 Codeg 参考和本地图。仅文档。 |
| 2 | `feat(acp): add stdio client and smoke probe` | 基于稳定 ACP 合同完成 initialize、能力读取、session/new、一次真实 prompt 与清理；分别检查官方 dsh 与 Codeg 的 bridge 入口，选定首个闭环后端，记录版本和反馈工具可达性。握手成功不等于闭环通过。 |
| 3 | `feat(sessions): persist managed session records` | 稳定 `session_id`、typed management、启动配置引用与 remote binding；migration、Rust DTO、前端生成类型同提交，旧数据按 external 读取。可创建有效配置供后续 supervisor 使用。 |
| 4 | `fix(sessions): support sessions without feedback` | 会话列表支持零请求、独立标题/时间；删除最后一条请求保留托管会话。验证旧外部分组行为。 |
| 5 | `feat(acp): supervise dedicated instances` | 每会话独占实例，拥有启动/连接/退出/清理边界；一个实例失败不影响其他实例。 |
| 6 | `feat(sessions): send prompts and record activity` | 在目标会话发送输入并持久化必要的活动记录；连接状态与执行状态分开；验证流式更新与并发归属。 |
| 7 | `feat(sessions): handle permissions and cancellation` | 权限交互与响应按会话/请求关联；取消当前轮次，断开时收尾挂起交互。 |
| 8 | `feat(api): expose managed session operations` | 将已有会话与启动配置操作接入 application facade，Tauri 与 Web 语义一致，不在 transport 层实现业务规则。 |
| 9a | `feat(ui): add agent configuration settings` | 独立设置页提交：Agent 列表、启用、启动配置编辑与检查结果；参考 Codeg 信息组织。配置保存、启动检查与已有会话状态分别表达。 |
| 9b | `feat(ui): add managed session workspace` | 新建会话、选配置/目录、发送任务、查看活动、处理权限与停止运行；零请求可见，关闭 view 不停止实例。 |
| 10 | `feat(feedback): bind requests to managed sessions` | 受会话作用域约束的反馈入口；controller 固定归属；托管等待说明不要求外部宿主确认，避免 continuation 死锁。 |
| 11 | `feat(feedback): persist pending deliveries` | 反馈终态与投递记录原子完成或可恢复对账；去重不依赖内存，支持一个会话多次反馈。 |
| 12 | `feat(sessions): continue after feedback submission` | 待会话可接收输入后续接；Agent 取得反馈后在同一上下文工作；处理取消/失败及发送结果不明，不盲目重放。 |
| 13 | `feat(sessions): delete managed sessions directly` | 运行中与空闲会话均可直接删除；停止所属资源、撤销反馈绑定、处理待投递并清理记录，失败可见可重试，不影响其他会话。 |
| 13b | `fix(feedback): route managed continuation through runtime` | 整合审阅补充：Desktop 后端按持久请求标记绕过旧 Host continuation router；同 host label 下 external 与 managed 正确分流。 |
| 14 | `feat(sessions): recover interrupted managed sessions` | 根据后端能力恢复；保留请求、草稿、结果与待投递，不把旧 connected 或不明发送结果当作成功。 |
| 15 | `test(acp): verify the managed feedback loop` | 补齐跨模块与真实后端验收：两个项目并发、完整反馈续接、直接删除、断开/重启；发布实际支持矩阵与使用说明。 |

步骤 9 从原来的一个 UI 提交拆为 9a、9b，整合审阅增加 13b；自动安装/更新 Agent、ACP registry 导入
和各后端全部设置不进入这两个 UI 提交。首期使用已安装的可执行程序与明确的参数/环境配置。

## Codeg 源码阅读配套

本地参考库与按步骤整理的源码入口见 [Codeg ACP 源码借鉴地图](CODEG_ACP_REFERENCE_MAP.md)。后续每步
先阅读对应实现和回归案例，再决定提取移植或按我们的合同实现；不把 Codeg 当作运行依赖，也不沿用其全部默认值。

步骤 2 需额外明确 ACP Client 的文件/终端回调能力：只宣告已实现的能力；若首个后端需要由客户端承接执行，
应另拆完整切片处理，而不是在连接探针中临时兜底。步骤 7 参考并发权限队列，步骤 10–12 参考会话作用域凭据
与写出后提交的顺序，同时保留 RambleDesk 自己的持久反馈与投递语义。此处补充阅读与验收输入，不改变既有步骤编号。

## 第一步验收

- 上述易混术语只有 [TERMINOLOGY.md](TERMINOLOGY.md) 一处权威定义；其他文档引用并与之保持一致。
- 明确 CURRENT / TARGET，不改运行代码、数据库或现有协议字段。
- Codeg 参考固定到 commit；区分源码观察、设计取舍与需要真实后端验证的假设。
- 通过术语/包边界检查和 `git diff --check`；仅提交本步骤文档，不夹带工作区其他改动。

## 执行记录

- 步骤 1：完成术语表、基线文档、ADR、Codeg 调研与提交地图；术语/包边界检查、本地文档链接检查和 diff 空白检查通过。
- 步骤 2：新增 `rambledesk-acp`（官方 SDK 2.0.0、稳定协议 v1），stdio 初始化/能力/创建/输入/取消/原 ID load 或 resume/显式 close 与回收。自动 fixture 覆盖取消、权限默认拒绝、恢复能力与 close 顺序；crate tests、clippy、术语/包边界检查通过。社区 0.7.0、0.8.0 与官方 dsh 的真实 MCP、上下文恢复证据见 [后端探针](ACP_BACKEND_PROBE.md)；Rust smoke 另验证实际回复和同 ID 恢复。首条 UI 验收采用社区 0.8.0，官方 dsh 保留作为 resume 差异对照。未宣告 client fs/terminal 能力。
- 步骤 3：启动配置 CRUD、稳定本地会话 ID、typed management、一次性 remote binding 与配置引用保护已完成；migration 0011 保留历史 external 语义，生成 TS 合同同步。51 项 storage 测试（含 v10→v11 migration）与 core 凭据 Debug 脱敏测试通过。
- 步骤 4：列表暴露 `session_id` / management；零请求托管会话支持标题、时间、搜索和现有会话操作，删除末条反馈保留本地与远端绑定。旧外部聚合语义保持；53 项 storage 与 21 项导航 TS 测试通过。
- 步骤 5：`SessionApplication` 与 driver port 创建/重试/启动/停止独占实例，进程细节留在 ACP 库。Windows 暂停启动后加入私有 Job Object 再执行；Unix 持有未回收 leader 的进程组，避免按过期 PID 清理。正常 EOF、悬挂后代、超时、Drop、握手失败、close 拒绝/超时与另一实例隔离共 6 项过程测试通过；失败保留会话、并发启动幂等、关闭 runtime 中断启动共 3 项应用测试通过，clippy 通过。Unix 代码尚待相应平台 CI 实跑。
- 步骤 6：输入先持久化，再进入目标实例；运行状态与连接状态分开。Activity repository 提供幂等序号、串行文本合并、工具归属、最近窗口及游标读取，原会话恢复不重复写入重放。新增按会话失效资源键，流式更新不刷新全部导航。7 项 activity 存储测试、2 项真实 stdio driver/application 集成（两实例输出隔离、忙时拒绝重复输入、停止隔离、重开数据库与原 ID 恢复）通过，core/ACP/storage clippy 通过。
- 步骤 7：权限队列按本地会话与随机请求 ID 关联，校验选项并只消费一次；取消清空挂起权限，未及时完成时停止所属实例。取消超时按 instance 与 turn 双重关联，避免误停后续任务。5 项 runtime 集成测试、2 项 stdio 测试与 6 项进程测试通过，覆盖跨会话拒绝、非法选项、并发权限、取消后新任务及不合作后端的隔离收尾。
- 步骤 8：11 项配置/会话命令通过同一 application facade 暴露到 Tauri 与 HTTP，新增稳定错误映射；所有 9 项 mutation（含连接检查）受 runtime generation 约束。3 项真实 HTTP/facade、失效 generation 与 Tauri 注册/输入 parity 测试通过，Desktop 编译通过；前端 106 项合同/transport/投影测试通过。
- 步骤 9a：Desktop/Web 独立 Agent 设置页，支持配置列表、编辑、启用与连接检查；命令参数按数组保存，环境值默认遮蔽，配置草稿按 ID 保留，界面卸载不操作后端。20 项配置/控制器/SSR 测试与 Svelte 检查通过。
- 步骤 9b：导航接入新建配置/目录会话与 Agent 工作区；零反馈默认显示 Agent，反馈与 Agent 面板可切换，权限/取消独立响应，关闭视图仅释放订阅。会话 ID 固定控制器，精确失效与并发快照合并避免串会话。Agent UI 共 30 项测试、Svelte 0 error / 0 warning 通过；直接删除入口留待步骤 13 接入。
- 步骤 10：托管 MCP 固定请求归属，独立随机凭据与独立 transport session manager；撤销等待已进入操作退出，跨 scope、伪造身份与混用 MCP session 均拒绝。实例启动注入 HTTP MCP，恢复轮换凭据，失败/停止撤销；Desktop 共享同一 provider/listener。4 项 HTTP 作用域测试、16 项 MCP、7 项原 HTTP 安全回归、4 项 runtime（含凭据生命周期）通过；Desktop 编译与相关 clippy 通过。
- 步骤 11：所有托管反馈终态与 outbox 同事务写入，重复提交/发布恢复幂等；attempt CAS 防止并发重复认领，重启 sending 转 uncertain，只能显式重试或确认。迁移补齐已有托管终态，按会话丢弃与跨作用域校验完善。8 项 outbox 测试及 storage 共 74 项回归通过。
- 步骤 12：runtime worker 只向空闲且连接有效的原会话续接；发送前持久认领，正常轮次终态后标记 delivered，断开/异常标记 uncertain 并停止自动重放。Desktop/Web 同时展示投递状态与显式重试/确认，托管请求隐藏旧的返宿主续接流程。真实 stdio→专属 MCP→提交→outbox→同上下文 get_feedback 的 2 项集成通过（含忙时等待、多次反馈、重复提交与读反馈后断线），HTTP resolve/generation、前端 115 项及 clippy 通过。断线测试推动修正 SDK task 与底层 EOF 的存活判断差异。
- 步骤 13：运行中、空闲和零反馈会话均可直接删除；持久删除意图阻止新工作，先撤销 scope/停止所属实例，再丢弃投递并清理文件与记录，失败保留可重试状态。严格验证所属目录并拒绝越界/junction，发布与删除共用锁阻止旧任务重建文件；已删会话的迟到事件不重建 runtime。8 项存储删除测试、4 项跨模块闭环/删除/重启后删除测试、HTTP 204/404 与 generation/parity、前端删除/只读/标签隔离验收通过。
- 步骤 13b：Desktop terminal observer 读取持久 managed 标记，在进入旧 Host continuation router 前完成分流；归属读取失败不推断为 external。新增同 host label 下 managed/external/未知请求的路由归属测试通过。
- 步骤 14–15：推进中，逐步验收后记录。
