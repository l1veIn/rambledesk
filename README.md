# RambleDesk

面向 Agent 的本地体验式反馈工作台。

Agent 通过本地 MCP 下发结构化请求；人类按清单真实使用、自由 ramble、配截图；
反馈以带图 Markdown package 回传，Agent 在原任务中继续迭代。

## 一句话定位

> Agent 呼叫你做真实使用反馈，你用 ramble 交回图文结果。

## 文档

| 文档 | 说明 |
|------|------|
| [产品宪章](docs/CONSTITUTION.md) | North Star、不可妥协原则、User_0 边界 |
| [产品文档](docs/PRODUCT.md) | 背景、问题、方案、范围、非目标、主流程、信息架构、恢复机制 |
| [架构基线](docs/ARCHITECTURE.md) | apps/crates monorepo、组件边界、运行时与一致性 |
| [MCP 与反馈协议](docs/PROTOCOL.md) | 工具 schema、幂等性、状态、错误与安全 |
| [开发计划](docs/DEVELOPMENT.md) | 技术栈、数据基线、里程碑和验收门 |
| [MCP 兼容矩阵](docs/COMPATIBILITY.md) | M0 实测客户端、协议、认证与执行模式 |
| [Kotone 复用审计](docs/KOTONE_REUSE.md) | 可迁移语音组件、必须修改项与许可证门禁 |
| [设计访谈纪要](docs/INTERVIEW.md) | 历史决策上下文，不作为现行规范 |

## 状态

M0 技术与协议尖峰已完成。M1 已落地 SQLite 持久请求内核、
`request_feedback/get_feedback/cancel_feedback` polling 工具、幂等/取消语义
和重启恢复；MCP Inspector 与 Claude Code 实测通过。下一步继续
[M1 的 Inbox、Draft 与纯文本提交闭环](docs/DEVELOPMENT.md#m1纯文本纵向闭环)。

## 本地验证

要求 Rust 1.91.1、Node.js 22.23.0 和 pnpm 10.12.4。

```bash
pnpm install --frozen-lockfile
cargo test --workspace --all-targets
pnpm check
pnpm test
pnpm build
pnpm mcp:inspector-smoke
```

无桌面壳的 MCP 自检：

```bash
pnpm mcp:self-test
```

## 许可证

待定。
