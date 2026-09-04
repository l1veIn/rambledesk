# ADR 007：ACP 托管会话与反馈适配器分工

- 状态：Accepted / TARGET，尚未实现
- 日期：2026-09-04
- 术语源：[TERMINOLOGY.md](../TERMINOLOGY.md)
- 调研：[Codeg ACP 接入与设置页](../CODEG_ACP_RESEARCH.md)
- 交付顺序：[ACP 提交地图](../ACP_COMMIT_MAP.md)

## 背景

现有反馈适配器已经完成持久反馈闭环，但用户仍需分别启动工作台和 Agent Backend；Generic MCP
手动 continuation 会使用户往返两个界面。模型也可能按任务拆分 `host_session_id`，把同一 Agent
Session 的反馈分散到多个工作台会话。这些实际摩擦推动会话管理能力，宪章修订见
[CONSTITUTION.md](../CONSTITUTION.md) 的 2026-09-04 修订记录。

## 决策

### 1. 保留反馈适配器，新增会话管理能力

Backend Runtime 持有 Agent Session Management，通过 ACP Client 控制外部 Agent；反馈适配器
继续处理 Request / Package 与 continuation。ACP 不是新的全能 Adapter，也不替代反馈合同。

```text
Workbench Client
  → Application Transport
  → Backend Runtime
      ├─ Agent Session Management → ACP Client → ACP Server / Bridge → Agent Backend
      └─ Feedback application ← Feedback Adapter ← Agent 的反馈工具调用
```

core 定义 typed application contract 与 ports，ACP SDK、stdio、子进程管理和宿主特例在实现边界外。
Desktop 与 Web 调用同一 application 能力；首期不增加独立 daemon、网络 ACP listener 或 checkout 模型。
不提前承诺 crate 划分，ACP 的进程归属由 Backend Runtime 控制。

### 2. 身份、配置与运行资源分离

遵守术语表中的一会话对一 Agent Session 约束；多个任务与请求都留在该会话。CURRENT 的
`(host_id, host_session_id)` 是外部关联合同，历史分组保留。TARGET 暴露 `host_sessions.id` 为稳定
`session_id`，托管路径创建反馈时由 controller 注入会话归属；模型不得任意重选关联 id。

使用有分支的 `management`，区分 `external` 与带 `protocol`、`agent_config_id`、`cwd`、
`remote_session_id` 的 `managed`。启动配置与 Host Profile 分离；同一配置可用于多个会话，每个会话
固定自己的工作目录。运行实例身份和连接/执行状态属于 runtime 投影，反馈投递属于独立持久记录。

存储首选在现有 `host_sessions` 上增加托管扩展记录，而非复制一套会话表；具体 migration 与 Rust DTO
在持久化提交内定稿。创建前先保存本地会话，Agent 创建成功后绑定 remote id；失败也应能查看和清理。
重连不改变本地身份，恢复必须由后端能力支持，不能把新建的空白 Agent Session 冒充原上下文。

### 3. 首期独占实例，保留后续调整空间

每个托管会话启动并独占一个 ACP Instance。实例可以包含 bridge 和多个子进程；进程内部的实现拓扑
不是 RambleDesk 的会话模型。即使某后端支持一个 ACP 连接承载多个会话，首期也不实现连接池与跨会话共享。
未来若需要共享，扩展实例分配策略即可；不得让一个 Agent Session 同时绑定多个 RambleDesk Session。

### 4. 生命周期按用户动作表达

- 零反馈请求的会话可创建、显示；删除最后一个请求不隐式删除托管会话。
- 关闭 Tab、浏览器或 Web Access 不停止托管会话；退出拥有 runtime 的应用需要显式处理资源收尾与中断状态。
- 取消当前轮次、停止运行并保留历史、删除会话是不同动作。删除托管会话不以先结束/归档为前提。
- 删除操作先阻止新输入与反馈归属，停止本实例资源并处理待投递项，然后清理本地记录；失败必须可见、可重试，
  不得返回成功却遗留可继续工作的孤儿实例。不得清理其他会话或用户独立启动的共享服务。
- ACP permission 回调是会话交互，不是 Feedback Request；按关联 id 返回用户选择，不默认自动批准。

### 5. 反馈归属、等待与投递共同形成闭环

托管会话使用受会话作用域约束的反馈入口；其身份绑定不能依赖模型遵守提示词。Generic MCP 现有的外部
宿主确认等待说明不能直接套到托管路径，否则仍会要求人类回到宿主。首期非阻塞反馈调用返回后，让当前
执行轮次结束；用户提交后，Backend Runtime 等待会话可接收输入，再投递携带 `request_id` 的续接消息。
Agent 通过原反馈合同取得持久结果。原生适配器在 tool call 内等待时，不再额外发送一次 continuation。

反馈终态发布与待投递记录必须原子完成或可恢复对账。发送结果不明时不能盲目重放；重启后不能凭旧
connected 字段或 in-memory 去重集合宣称投递成功。具体状态迁移与失败证据随实现及测试交付。

## 参考取舍与验收

Codeg 的启动注册表、后端专属配置和设置页信息组织可参考；其 UI 框架、历史导入、自动安装与会话展示策略
不直接成为 RambleDesk 的需求。尤其要区分 Codeg 使用的 `deepseek-acp` 与官方 dsh ACP 入口，分别记录
版本和能力；先验证一个真实后端的 initialize、session/new、prompt、反馈工具可达性与资源清理。

后续采用小步、可合并的正式迭代：每步一个完整 commit，合同、migration、生成类型和相关测试同行。
首个完整闭环为：创建会话 → 发送任务 → 固定归属的反馈请求 → 人类提交 → 同一 Agent Session 继续 →
直接删除并清理。多项目并发不串会话、中断恢复不丢反馈是验收条件。CURRENT 外部适配器持续可用。

## 未进入首期的能力

共享实例池、任意远程 ACP transport、通用 Agent 安装器、ACP registry 市场、跨后端历史导入、自动迁移旧
会话分组与多 Agent 编排。Codeg 有相应实现时可作为后续参考，不能以此扩大本轮闭环。
