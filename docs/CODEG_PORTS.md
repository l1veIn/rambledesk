# Codeg 移植记录

上游：<https://github.com/xintaofei/codeg>，固定 commit `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。
Codeg 作者和贡献者保留其版权；根仓库为 Apache-2.0，完整条款见 `licenses/codeg-APACHE-2.0.txt`。
本文件记录直接改写的来源与本项目变更；仅阅读过的候选来源见 [参考地图](CODEG_ACP_REFERENCE_MAP.md)。

| RambleDesk 模块 | 上游来源 | 修改及验收 |
| --- | --- | --- |

| 结构化会话记录与工具补丁 | `src-tauri/src/acp/session_state.rs`、`types.rs` | 适配持久化存储、旧历史兼容和大小限制；迁移、补丁、流式顺序与隔离测试通过，详见 [记录](CODEG_STRUCTURED_ACTIVITY.md)。 |

后续每个移植提交添加实际条目。第三方依赖各自遵循其许可证，不因 Codeg 根许可证而改变。
