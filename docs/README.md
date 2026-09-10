# RambleDesk 文档

第一次使用从[项目 README](../README.zh-CN.md)开始。这里维护当前使用说明、实现合同和仍需验证的事项；完成的实施计划与逐轮排查交给 Git 历史。

## 使用与接入

| 想做什么 | 入口 |
| --- | --- |
| 连接 Coding Agent、开始会话、恢复失败 | [ACP 使用指南](ACP_MANAGED_SESSIONS.md) |
| 从自己的 Agent 应用接入反馈 | [外部适配器兼容性](COMPATIBILITY.md) |
| 使用浏览器或手机访问工作台 | [Web Access 支持矩阵](WEB_ACCESS_SUPPORT_MATRIX.md) |
| 排查问题、导出诊断包 | [问题反馈与诊断](RC_FIELD_FEEDBACK.md) |
| 升级、迁移数据或回退版本 | [数据兼容说明](DATA_COMPATIBILITY.md) |
| 查看版本变化 | [更新日志](CHANGELOG.md) |

## 开发与设计

| 想理解什么 | 入口 |
| --- | --- |
| 本地运行、检查和修改流程 | [开发指南](DEVELOPMENT.md) |
| 产品使命、主要旅程与范围 | [产品说明](PRODUCT.md)与[产品原则](CONSTITUTION.md) |
| 唯一术语及身份边界 | [术语表](TERMINOLOGY.md) |
| 模块所有权、持久化与客户端生命周期 | [架构](ARCHITECTURE.md) |
| 外部反馈请求、幂等与交付协议 | [协议](PROTOCOL.md) |
| 如何欣赏和沿着代码阅读 | [反馈链路示范](FEEDBACK_FLOW_WALKTHROUGH.md)与[全局职责地图](quality/QUALITY_WALKTHROUGH.md) |
| 为什么作出这些架构选择 | [架构决策 ADR](adr/)；当前实现以架构为准 |
| 宣传页设计、资源和维护 | [设计说明](design/README.md)与[网页开发指南](../web/README.md) |

## 质量与发布

- [质量与待验清单](quality/README.md)：现有证据的边界、未完成里程碑与历史记录入口。
- [隔离反馈验收](quality/FEEDBACK_ACCEPTANCE.md)、[性能验收](quality/PERFORMANCE_ACCEPTANCE.md)、[真实 Agent 探针](ACP_BACKEND_PROBE.md)：可重复执行的方法。
- [发布检查清单](RELEASE_CHECKLIST.md)：版本、构建产物、平台与升级验证。

## 来源与维护规则

第三方版权和采用边界见[第三方声明](../THIRD_PARTY_NOTICES.md)、[Codeg 来源](CODEG_PORTS.md)；品牌图片见[素材来源](social/README.md)。`CHANGELOG.md` 和 `CODEG_PORTS.md` 同时被发布或打包代码读取，移动前需同步消费者。

一个事实只在对应指南维护，其他文档链接它。ADR 保留当时的决策及后续取代关系；临时计划、原始截图和逐次测试输出先放本机 `.local-artifacts/`，交付时只选入必要结论与可追溯证据。不能把“自动化通过”“浏览器模拟通过”和“真机通过”合并成同一种状态。

整理前已提交的文档可从[固定 Git 快照](https://github.com/l1veIn/rambledesk/tree/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs)追溯；它是历史资料，不是当前操作指南。
