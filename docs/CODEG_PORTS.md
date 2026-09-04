# Codeg 移植记录

上游：<https://github.com/xintaofei/codeg>，固定 commit `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。
Codeg 作者和贡献者保留其版权；根仓库为 Apache-2.0，完整条款见 `licenses/codeg-APACHE-2.0.txt`。
本文件记录直接改写的来源与本项目变更；仅阅读过的候选来源见 [参考地图](CODEG_ACP_REFERENCE_MAP.md)。

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
| 托管 stdio 反馈 companion | `src-tauri/src/delegation/companion.rs`、`acp/connection.rs` | 复用实例私有 HTTP 归属与撤销，环境变量授权，三工具原样转发；CLI 和 Desktop 早分派。真实 CLI/两 scope/SQLite/HTTP 集成及 bounded input 用例通过；生产启动已自动选择 HTTP 优先、stdio 回退；原会话续接与双实例清理通过。 |
| 结构化提示输入 | `src-tauri/src/acp/types.rs`、`connection.rs` | 沿用共用发送/取消/continuation，独立完整输入与历史预览限制；协商能力、真实 stdio 图片和资源 4 个用例，详见 [输入记录](CODEG_TYPED_PROMPTS.md)。 |

Pi 专用 wrapper、托管扩展及反馈投递仍是 RambleDesk 自有实现，复用现有会话归属、撤销和持久投递合同。完整结果与证据边界见 [本轮验收](CODEG_ACCEPTANCE.md)。

第三方依赖各自遵循其许可证，不因 Codeg 根许可证而改变。
