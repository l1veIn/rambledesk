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

M0 技术与协议尖峰已完成。M1 已在 macOS 完成验收，并落地 SQLite 持久请求内核、
`request_feedback/wait_for_feedback/get_feedback/cancel_feedback` durable wait 工具、Inbox、Markdown
Draft 自动保存和 crash-safe Feedback Package 提交；MCP Inspector、Claude Code
与官方 Rust SDK 实测通过。新请求系统通知采用显式授权且不显示工作内容。
Windows Feedback Package 发布使用独立平台兼容层和 write-through 目录移动，
跨层提交、幂等与启动恢复测试已通过。M2 的图片粘贴/拖放/选择、全局快捷键内置区域
截图、附件不可变发布、历史查询、`list_feedback_requests`、Windows 托盘待处理徽标
和 MCP 配置复制已经完成 Windows 人工签收；M3 的 Sherpa 中文流式转写和主 Ramble
流程也已签收。当前正在把语音、截图与剪贴板收敛为可暂停、继续的统一 Ramble 状态。

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
