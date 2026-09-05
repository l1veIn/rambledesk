# Codeg 移植记录

上游：<https://github.com/xintaofei/codeg>，固定 commit `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。
Codeg 作者和贡献者保留其版权；根仓库为 Apache-2.0，完整条款见 `licenses/codeg-APACHE-2.0.txt`。
本文件记录直接改写的来源与本项目变更；仅阅读过的候选来源见 [参考地图](CODEG_ACP_REFERENCE_MAP.md)。

2026-09-05 体验重设计已实现，Windows 自动化与隔离浏览器验收已完成。旧通道及其测试作为历史移植证据保留；当前生产反馈使用应用内置 command，完整架构与未验证边界见 [本轮计划](ACP_EXPERIENCE_REDESIGN_PLAN.md)。

Web 构建输出和 Desktop 安装资源均包含 `THIRD_PARTY_NOTICES.md`、
`licenses/codeg-APACHE-2.0.txt` 与本文件 `docs/CODEG_PORTS.md`。
Vite 构建与 Tauri 资源映射直接读取仓库中的原文件，不维护额外的源码副本。

| RambleDesk 模块 | 上游来源 | 修改及验收 |
| --- | --- | --- |
| 结构化会话记录与工具补丁 | `src-tauri/src/acp/session_state.rs`、`types.rs` | 适配持久化存储、旧历史兼容和大小限制；迁移、补丁、流式顺序与隔离测试通过，详见 [记录](CODEG_STRUCTURED_ACTIVITY.md)。 |
| Agent 输入器 | `src/components/chat/composer/*`、`src/lib/message-quote.ts` | 移植纯文本 Tiptap 配置、序列化、引用、IME 和快捷键；Svelte 包装接入会话草稿和发送/取消。48 个编辑器用例、58 个集成相关用例通过，详见输入器 README。 |
| Agent 目录、检测、安装 | `src-tauri/src/acp/{registry,preflight,binary_cache}.rs`、`commands/acp.rs` | 固定版本目录、独立安装代、原子发布、真实包入口检查、自有进程取消清理；7 项真实 Node 子进程 fixture 及 clippy 通过。 |
| Agent 管理与认证表单 | `src/app/settings/page.tsx`、`commands/acp.rs::agent_env_keys` | Svelte 主从列表、安装后台任务、真实版本检测及按智能体映射密钥/地址/模型；不写上游全局配置。取消竞争与清理 3 个 core 测试、跨客户端 44 个测试及 Svelte 检查通过。 |
| 动态会话配置 | `src-tauri/src/acp/{connection,types}.rs` | Agent 确认的 options、模型、模式缓存，兼容 legacy models；现代 option 完整替换。3 个真实 stdio 用例覆盖 ACK、拒绝、推送、原会话恢复与取消；原 runtime/stdio 7 个回归通过。 |
| Chat 时间线与工具卡片 | `src/components/message/*`、`ai-elements/reasoning.tsx`、`src/lib/{line-diff,unified-diff-generator}.ts` | Svelte 消息/思考/工具/差异卡片、安全 Markdown、引用输入器；渲染与补丁/差异测试、历史游标与滚动锚点回归通过；隔离浏览器验证实际卡片与 60→120 条展开。 |
| 托管 stdio 反馈 companion（历史通道） | `src-tauri/src/delegation/companion.rs`、`acp/connection.rs` | 曾改写实例私有 HTTP 归属、撤销、环境授权与三工具转发，并完成 CLI/两 scope/SQLite/HTTP 等测试。现已退出生产 ACP 启动选择；保留来源声明，不把旧测试视为统一 command 的模型验收。 |
| 按轮次的过程折叠、最终回答与页脚 | `src/components/message/completed-turn-content.tsx`、`turn-stats.tsx`、`live-turn-stats.tsx` | Svelte 适配真实 turn ID 和持久起止标记；运行中默认展开、结束自动收起，手动选择优先；最终回答独立、复制和完成时间、未知耗时隐藏、跨页轮次与延迟挂载。定向回归已通过，性能对照尚待记录。 |
| 输入器上下文占用 | `src/components/chat/composer-context-usage.tsx` | 参考上下文占用呈现，接入 ACP 实际 `usage_update` 的 used/size；无上报隐藏，实例更换后等待新值。不推算累计 token、费用或任务消耗。 |
| 结构化提示输入 | `src-tauri/src/acp/types.rs`、`connection.rs` | 沿用共用发送/取消/continuation，独立完整输入与历史预览限制；协商能力、真实 stdio 图片和资源 4 个用例，详见 [输入记录](CODEG_TYPED_PROMPTS.md)。 |

统一反馈命令、HTTP 归属、prepared 生命周期、草稿控制器与持久投递是 RambleDesk 自有实现，继续使用既有撤销、幂等和恢复合同。Pi 专用 wrapper/托管扩展不再作为生产 ACP 反馈路径；外部 Pi/dsh 适配器保留。历史移植验收见 [CODEG_ACCEPTANCE.md](CODEG_ACCEPTANCE.md)，本次最终 QA 见体验重设计计划。

第三方依赖各自遵循其许可证，不因 Codeg 根许可证而改变。
