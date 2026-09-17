# 工作台先导文档（讨论稿）

状态：**讨论稿，未实现**。这一组文档的目的不是规定实现，而是**在动手前把问题想全面**：
多工作台是较大的重构，影响协议、客户端布局、Agent 用法与术语，因此先落讨论稿、再定路线。

- 术语以[术语表](../TERMINOLOGY.md)为准，本文档组不建立第二份词汇表。
- 迭代路线与阶段验收以[工作台演进路线](../WORKBENCH_EVOLUTION.md)为准；本组文档只补充"怎么做/有什么坑"。
- 协议字段以[反馈协议](../PROTOCOL.md)与 `crates/rambledesk-core/src/feedback/model.rs` 为准（本文档只做引用与现状摘录）。

## 已经落地的部分（讨论的前提）

| 阶段 | 状态 | 证据 |
| --- | --- | --- |
| 阶段 0：输入集中到 TipTap | 已完成 | 提交 `3affba8` 一带的编辑面 |
| 阶段 1：Ramble 布局（反馈列右移） | 已完成 | `e35ddde`、`e6eeff1`、`34ea6fe` |
| 阶段 2：提取容器与反馈列（第一刀） | 已完成 | `cd76c53`（`WorkbenchContainer` + `RambleWorkbench`）、`5dcc8ea`（隐藏全屏简报视图） |
| 阶段 2 收尾（目标显式化、全屏视图去留） | 未开始 | 见 [开放问题](open-questions.md) |
| 阶段 3 及以后 | 未开始 | [工作台演进路线](../WORKBENCH_EVOLUTION.md) |

## 这组文档

| 文档 | 回答什么 |
| --- | --- |
| [contract.md](contract.md) | 工作台与公共容器之间**最小接口**是什么：工作台只拿只读上下文 + 三个回调；谁拥有编辑器、采集、草稿、提交。 |
| [registry-and-data.md](registry-and-data.md) | 注册项长什么样、今天 Ramble 的输入字段实际是什么、**共享材料与类型专属字段**怎么分、幂等与版本怎么办。 |
| [agent-surface.md](agent-surface.md) | 站在 Agent 视角：今天 3 个工具与字段、多工作台如何路由（类型是数据）、上下文分几层、结果怎么读。 |
| [open-questions.md](open-questions.md) | **头脑风暴清单**：动手前要一起想清楚的问题与风险，按协议/界面/Agent/兼容/测试/产品分组。 |

## 建议的阅读顺序

1. [open-questions.md](open-questions.md)：先看问题全貌，判断哪些需要现在就定。
2. [registry-and-data.md](registry-and-data.md) 与 [agent-surface.md](agent-surface.md)：数据与 Agent 视角是一体两面，一起读。
3. [contract.md](contract.md)：最后看接口，因为接口应当由前两者的结论推出，而不是反过来。
