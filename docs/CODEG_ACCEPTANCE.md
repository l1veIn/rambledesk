# Codeg 深度移植验收

范围：`ccbc083` 之后的 Agent 管理、Chat、结构化输入、反馈接入扩展。
参考 Codeg `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`；实现分独立提交，保留原有反馈与会话身份合同。
实跑环境为 Windows，2026-09-04 至 2026-09-05。没有修改用户数据库或重发用户历史任务。

## 已实现的产品行为

- 内置 16 个 Agent 入口，npm 分发可安装到自有目录、检测实际版本和更新；非 npm 入口显示官方安装引导。
- 普通连接字段按智能体提供，命令/参数/环境变量保留在高级配置；后台安装支持取消、失败回滚和跨页进度。
- 聊天支持持续更新的 Markdown、思考、工具输入/输出/状态、文件差异、引用、多行输入和每会话草稿。
- 发送立即清空当前草稿，后续编辑不会被迟到的发送结果覆盖；附件受实际 ACP 能力和大小限制约束。
- 模型/模式/其他配置来自 Agent，等待确认后更新；拒绝或重新连接会显示当前真实值。
- 持久结构化历史支持游标分页；初次挂载 60 条，按需展开并保持阅读位置。
- HTTP MCP 优先，stdio companion 回退；Pi 使用自有 RPC wrapper 加载托管扩展。各通道共用归属、撤销、原会话自动续接和删除合同。

## 真实程序与网络证据

| 路径 | 实际完成 | 尚未声称完成 |
| --- | --- | --- |
| DeepSeek ACP 0.8.0 | 自有 npm 安装、Managed 检测、真实 bin.js 握手与退出；前一阶段已完成双项目真实模型反馈闭环 | 其他版本/平台完整闭环 |
| Codex ACP 1.8.0 | 自有 npm 安装、dist/index.js 启动及 bundled Codex 0.152.1 握手，确认 HTTP、load/resume，正常退出 | 模型、反馈与恢复端到端 |
| 原生 Pi 0.83.0 | 离线加载生产托管扩展、原生参数校验、实际私有请求入库；未调用模型 | pi-acp 桥接后的真实模型闭环 |
| stdio 与 Pi 协议夹具 | 真实 CLI/wrapper/Node/HTTP/SQLite，自动续接、原 ID load/resume、删除 A 保留 B、启动未 ACK 时停止及孙进程回收 | 将协议夹具等同于真实 Agent 推理 |

网络安装最初遇到一次 ECONNRESET，有限重试后通过；Codex 探针补建自己的 CODEX_HOME 后握手通过。
这些修正未更改全局代理、npmrc、用户认证或生产安装算法。
网络报告位于本机临时目录 `rambledesk-catalog-network-LAuW8u/report.json`。
可复现 gate：`cargo test -p rambledesk-acp --test catalog_network -- --ignored --nocapture`；默认测试不会下载或启动真实 Agent。
真实 Pi gate 为 CLI 的 `installed_native_pi` ignored test，需要显式指定隔离 Pi 路径。

## 自动化与界面验证

- 前端：115 个文件、673 项测试通过；Svelte 0 errors / 0 warnings。
- Rust workspace（排除同名桌面目标）：28 个测试套件、295 项通过，3 项按预期忽略；随后 Pi 修复的 5 项实际 CLI 集成单独复验通过。
- Desktop：103 项通过，包含 93 个 lib、原生事件/合同/跨客户端一致性和两个新增入口测试；真实 debug exe 的 MCP initialize/tools/list/EOF 与 Pi RPC/扩展/EOF 均通过，未创建 Tauri 用户数据。
- Pi 扩展 24 项、dsh 扩展 27 项通过；真实 Pi 离线 gate 额外运行通过。
- 全 workspace/all-targets clippy、Rust 格式、模块大小、术语/依赖边界、生成合同一致性检查通过。
- Web production build 通过；许可证、第三方声明、移植记录进入 Web manifest，Tauri resources 显式随包分发。
- 隔离浏览器验收：亮/暗主题、760px 聊天宽度、流式到结束无需切页、草稿保留、模型确认、工具 diff，以及 1,105 条记录初次只挂 60 条、展开到 120 条且保持位置。
- 开发预览复现见 `apps/desktop/src/dev/README.md`，所有 Agent 与安装交互均为内存夹具。

## 实施边界

安装、认证、ACP 握手和完整反馈闭环是不同证据。目录没有把未验证入口标记为完整兼容。
OpenClaw 的固定版本不接受所需 MCP 入口，仍明确不可用于托管反馈。
本机没有可用的 Unix 运行环境；Unix wrapper 改为 exec 保持外层进程组，实跑由已配置的 Linux/macOS CI 承担。
当前没有远程 ACP、共享 Agent 进程池、外部历史批量导入或 Codeg 的多 Agent 编排。
这一轮没有生成发布签名安装包，也没有执行原生桌面整个用户流程的人工验收。
