# 工作台文档入口

当前工作台采用第一方静态类型：共享请求、草稿、反馈列与提交生命周期，各类型拥有独立业务视图。对象定义和 wire 字段已经进入权威文档，历史讨论不再作为实现依据。

| 内容 | 现行入口 |
| --- | --- |
| 产品范围与持久化人类回答 | [产品宪章](../CONSTITUTION.md)、[ADR 008](../adr/008-typed-human-feedback-workbenches.md) |
| 类型、视图、Draft、Document、State、Result、Outcome 的边界 | [术语表](../TERMINOLOGY.md) |
| 运行所有权、静态组合与客户端边界 | [架构](../ARCHITECTURE.md) |
| 发现目录、请求、幂等与结构化返回 | [反馈协议](../PROTOCOL.md) |
| 实施阶段、待完成内容与验收要求 | [工作台演进路线](../WORKBENCH_EVOLUTION.md) |
| 文稿审阅 v1 输入、锚点、批注和结果 | [文稿审阅](document-review.md) |
| 打开目录，用一句提示词体验全部现有类型 | [工作台 Playground](../../playground/workbenches/README.md) |
| 三种类型实验的起点与试用背景 | [三种工作台实验记录](experiment.md) |

## 历史讨论

下列文件保留最初的备选方案、问题与迁移顺序。它们关于“尚未实现”、`ownedFields`、统一输入目标、自由正文降级和 AskQuestion 形状的描述均是当时的讨论，不能覆盖现行合同。

- [容器接口讨论](contract.md)
- [注册项与数据讨论](registry-and-data.md)
- [Agent 工具面讨论](agent-surface.md)
- [开放问题清单](open-questions.md)

尤其注意：**不认识的类型或版本采用只读保留，而非可提交的自由正文回退。** 已知类型的正文与结构化输入一起保存，但正文不能替代必填答案。文稿审阅保留原稿，由 Agent 取得提交结果后修改；它不把审阅过程变成直接文档编辑。
