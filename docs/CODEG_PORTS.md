# Codeg 移植记录

上游：<https://github.com/xintaofei/codeg>，固定 commit `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。
Codeg 作者和贡献者保留其版权；根仓库为 Apache-2.0，完整条款见 `licenses/codeg-APACHE-2.0.txt`。
本文件记录直接改写的来源与本项目变更；仅阅读过的候选来源见 [参考地图](CODEG_ACP_REFERENCE_MAP.md)。

| RambleDesk 模块 | 上游来源 | 修改及验收 |
| --- | --- | --- |

| 结构化会话记录与工具补丁 | `src-tauri/src/acp/session_state.rs`、`types.rs` | 适配持久化存储、旧历史兼容和大小限制；迁移、补丁、流式顺序与隔离测试通过，详见 [记录](CODEG_STRUCTURED_ACTIVITY.md)。 |

| Agent 输入器 | `src/components/chat/composer/*`、`src/lib/message-quote.ts` | 移植纯文本 Tiptap 配置、序列化、引用、IME 和快捷键；Svelte 包装接入会话草稿和发送/取消。48 个编辑器用例、58 个集成相关用例通过，详见输入器 README。 |

| Agent 目录、检测、安装 | `src-tauri/src/acp/{registry,preflight,binary_cache}.rs`、`commands/acp.rs` | 固定版本目录、独立安装代、原子发布、真实包入口检查、自有进程取消清理；7 项真实 Node 子进程 fixture 及 clippy 通过。 |

| Agent 管理与认证表单 | `src/app/settings/page.tsx`、`commands/acp.rs::agent_env_keys` | Svelte 主从列表、安装后台任务、真实版本检测及按智能体映射密钥/地址/模型；不写上游全局配置。取消竞争与清理 3 个 core 测试、跨客户端 44 个测试及 Svelte 检查通过。 |

后续每个移植提交添加实际条目。第三方依赖各自遵循其许可证，不因 Codeg 根许可证而改变。
