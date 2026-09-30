# 工作台文档入口

当前工作台采用第一方静态类型：共享请求、草稿、反馈列与提交生命周期，各类型拥有独立业务视图。新请求使用 `ramble`、`questions`、`document_review`、`web_review` 或 `terminal`；单题 `questions` 覆盖方案单选，旧 `single_choice` 仅保留合同兼容。

## 阅读路径

| 要做的事 | 阅读入口 |
| --- | --- |
| 理解产品范围及这些取舍的原因 | [产品宪章](../CONSTITUTION.md)、[ADR 008](../adr/008-typed-human-feedback-workbenches.md) |
| 区分类型、视图、Draft、Document、State、Result、Outcome | [术语表](../TERMINOLOGY.md) |
| 修改运行所有权、静态组合或客户端边界 | [架构](../ARCHITECTURE.md) |
| 创建请求、发现类型、读取结果或修改 wire 合同 | [反馈协议](../PROTOCOL.md)；具体类型 schema 由 `describe_workbench` 提供 |
| 判断当前已实现范围、扩展和验收边界 | [工作台实现状态](../WORKBENCH_EVOLUTION.md) |
| 修改文稿锚点、批注、删除建议或整稿判断 | [文稿审阅](document-review.md) |
| 接入网页、选择元素、批注和宽屏评审 | [网页评审](web-review.md) |
| 让用户试用 CLI、引用输出和保存试用记录 | [终端试用](terminal.md) |
| 修改正文、答案、批注的输入与异步回填 | [请求级共享输入](shared-input.md) |
| 修改 Rambelle 首次引导、重看和本地记录 | [工作台使用引导](onboarding.md) |
| 验证存储兼容或旧版本读取 | [数据兼容](../DATA_COMPATIBILITY.md) |
| 用真实 Agent 走完全部工作台 | [工作台 Playground](../../playground/workbenches/README.md) |

这些文档按职责维护，不另写一套竞争的字段定义：协议负责通用请求与结果；文稿审阅负责该类型的细节；共享输入负责编辑和采集；ADR 保存决策理由。类型字段及限制修改时同步生成 schema 和相关文档，不能只改示例。

## 开发时保留的边界

- 不认识的类型或版本只读保留，不提供可提交的自由正文回退。
- 正文与结构化状态一起保存、冻结和提交；补充正文不能替代必答项或整稿判断。
- 正文与业务字段共用 TipTap 基础，各可见字段保有自己的 Editor；采集会话、草稿队列和提交链路仍然共享。
- 语音和附件固定发起时的请求与目标；字段失效时报告失败，不转移到默认输入区。
- 文稿原稿保持只读。批注无解决状态；删除线表达建议，Agent 取得已提交结果后再修改原稿。
- 旧单选合同、旧批注和旧语音记录的兼容路径有数据保留用途，不能因新界面不再产生这些数据就删除。

早期容器接口、注册项、Agent 工具面、开放问题和三类型实验稿已由上述现行文档与 ADR 取代，已从当前文档树移除。需要追溯当时的备选方案时使用 Git 历史，不把历史提案作为待实现要求。
