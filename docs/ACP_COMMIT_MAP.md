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
| 14 | `feat(sessions): recover interrupted managed sessions` | 根据后端能力恢复；保留请求、草稿、结果与待投递，不把旧 connected 或不明发送结果当作成功。 |
| 15 | `test(acp): verify the managed feedback loop` | 补齐跨模块与真实后端验收：两个项目并发、完整反馈续接、直接删除、断开/重启；发布实际支持矩阵与使用说明。 |

步骤 9 从原来的一个 UI 提交拆为 9a、9b，共 16 个预期 commit；自动安装/更新 Agent、ACP registry 导入
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
- 步骤 2–15（含 9a / 9b）：尚未开始。
