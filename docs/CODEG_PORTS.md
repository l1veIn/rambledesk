# Codeg 移植记录

上游：<https://github.com/xintaofei/codeg>，固定 commit `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。
Codeg 作者和贡献者保留其版权；根仓库为 Apache-2.0，完整条款见 `licenses/codeg-APACHE-2.0.txt`。
本文件记录实际改写的来源与本项目变更；仅调研而未采用的候选不作为移植内容。

当前生产反馈使用应用内置 command，现行合同见 [ACP 指南](ACP_MANAGED_SESSIONS.md)，验收状态见[质量清单](quality/README.md)。历史通道的来源声明保留，不把当时的测试当作当前路径已通过的证据。

Web 构建输出和 Desktop 安装资源均包含 `THIRD_PARTY_NOTICES.md`、
`licenses/codeg-APACHE-2.0.txt` 与本文件 `docs/CODEG_PORTS.md`。
Vite 构建与 Tauri 资源映射直接读取仓库中的原文件，不维护额外的源码副本。

| RambleDesk 模块 | 上游来源 | 修改与采用边界 |
| --- | --- | --- |
| 外观配置（2026-09-07） | `src/lib/theme-presets.ts`、`src/app/globals.css`，参考 `appearance-provider.tsx`、`font-presets.ts`、`workspace-background.ts` | 13 套配色（含 RambleDesk 原色）及完整明暗语义变量；调整部分主按钮文字以保持对比度。Svelte 独立外观页，主窗口 WebView 缩放、界面/代码字体；默认花纹、纯色、本地图片背景。偏好校验、IndexedDB 图片存储和生命周期为本项目实现；字体独立 OFL 许可证随包提供。 |
| 结构化会话记录与工具补丁 | `src-tauri/src/acp/session_state.rs`、`types.rs` | 适配持久化存储、旧历史兼容和大小限制；采用边界见下文 [Structured session activity](#structured-session-activity)。 |
| Agent 输入器 | `src/components/chat/composer/*`、`src/lib/message-quote.ts` | 移植纯文本 Tiptap 配置、序列化、引用、IME 和快捷键；Svelte 包装接入会话草稿和发送/取消。 |
| Agent 目录、检测、安装 | `src-tauri/src/acp/{registry,preflight,binary_cache}.rs`、`commands/acp.rs` | 固定版本目录、独立安装代、原子发布、真实包入口检查、自有进程取消清理。 |
| Agent 管理与认证表单 | `src/app/settings/page.tsx`、`commands/acp.rs::agent_env_keys` | Svelte 主从列表、安装后台任务、真实版本检测及按智能体映射密钥/地址/模型；不写上游全局配置。 |
| 动态会话配置 | `src-tauri/src/acp/{connection,types}.rs` | Agent 确认的 options、模型、模式缓存，兼容 legacy models；现代 option 完整替换，保留 ACK、拒绝、推送、原会话恢复与取消合同。 |
| Grok 会话配置扩展（2026-09-07） | `src-tauri/src/acp/connection.rs` 中 `x.ai/sessionConfig` 与模型 metadata 映射 | 改为 RambleDesk 的 Model / Reasoning effort 配置项，标准 options 优先；按模型更新思考选项，通过 `session/set_model` 回传。权限模式不从 effort 推断。 |
| Grok 问答与计划审批（2026-09-07） | `src-tauri/src/acp/question.rs`、`plan_approval.rs`、`connection.rs` | 参考请求与响应映射，重写为原生 ACP pending queue、类型化输入响应与 Svelte 表单；共享现有会话生命周期。Grok 的问题答案以题目文本为键，取消使用其原生 outcome。 |
| Chat 时间线与工具卡片 | `src/components/message/*`、`ai-elements/reasoning.tsx`、`src/lib/{line-diff,unified-diff-generator}.ts` | Svelte 消息/思考/工具/差异卡片、安全 Markdown、引用输入器；渲染与补丁/差异测试、历史游标与滚动锚点回归通过；隔离浏览器验证实际卡片与 60→120 条展开。 |
| 托管 stdio 反馈 companion（历史通道） | `src-tauri/src/delegation/companion.rs`、`acp/connection.rs` | 曾改写实例私有 HTTP 归属、撤销、环境授权与三工具转发，并完成 CLI/两 scope/SQLite/HTTP 等测试。现已退出生产 ACP 启动选择；保留来源声明，不把旧测试视为统一 command 的模型验收。 |
| 按轮次的过程折叠、最终回答与页脚 | `src/components/message/completed-turn-content.tsx`、`turn-stats.tsx`、`live-turn-stats.tsx` | Svelte 适配真实 turn ID 和持久起止标记；运行中默认展开、结束自动收起，手动选择优先；最终回答独立、复制和完成时间、未知耗时隐藏、跨页轮次与延迟挂载。 |
| 输入器上下文占用 | `src/components/chat/composer-context-usage.tsx` | 参考上下文占用呈现，接入 ACP 实际 `usage_update` 的 used/size；无上报隐藏，实例更换后等待新值。不推算累计 token、费用或任务消耗。 |
| 历史预取与向上翻页 | `src/stores/conversation-runtime-store.ts`、`src/components/message/virtualized-message-thread.tsx` | 参考 turn 窗口和向上跨过 240px 触发；使用自有 SQLite 游标、默认 20 轮展示/历史查询、1000 活动及约 2MiB 单页预算。实时快照仍为 100 活动，历史只在打开时预取一次，加载前捕获锚点；复用未变化轮次，折叠过程延迟挂载。 |
| 结构化提示输入 | `src-tauri/src/acp/types.rs`、`connection.rs` | 沿用共用发送/取消/continuation，独立完整输入与历史预览限制；采用边界见下文 [Typed prompt content](#typed-prompt-content)。 |

统一反馈命令、HTTP 归属、prepared 生命周期、草稿控制器与持久投递是 RambleDesk 自有实现，继续使用既有撤销、幂等和恢复合同。当前分支已移除旧 Pi wrapper/托管扩展及 `managed-mcp-stdio` 转发入口；CLI 和桌面二进制均通过内置 `feedback` 命令处理 ACP 反馈。外部 Pi/dsh 适配器、普通 MCP 服务与安装流程、现存托管 HTTP scope 隔离和撤销合同保留。现行验证与剩余项目统一见[质量清单](quality/README.md)。

第三方依赖各自遵循其许可证，不因 Codeg 根许可证而改变。

## Structured session activity

固定来源为上述 Codeg commit 的 [`session_state.rs`](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/session_state.rs)（`LiveContentBlock`、`ToolCallState`、`upsert_tool_call`、`push_tool_call_ref_if_absent`）及 [`types.rs`](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/types.rs) 的事件内容字段。采用其有序内容块、工具首次出现位置和保留缺省字段的补丁规则；未采用瞬时会话所有权或厂商 raw-input 分块猜测。

RambleDesk 围绕自己的 `SessionActivityRepository` 重写持久化：同一会话与轮次中的工具 ID 保持一条记录和原序号，显式空集合可清除字段；结构化内容兼容旧文字历史。原始输入/输出仅作有界展示，截断预览不作执行输入或历史重放载荷。实现与边界常量分别位于 [core activity content](../crates/rambledesk-core/src/sessions/activity_content.rs) 和 [ACP 映射](../crates/rambledesk-acp/src/activity_content.rs)，避免在来源记录中维护另一套可漂移的数值合同。

迁移、字段补丁、文本/工具交错、UTF-8 与媒体上限、跨会话隔离和迟到事件有自动化回归。它们证明持久化与映射合同，不代表某个真实后端或原生界面已验收。

## Typed prompt content

固定来源为上述 Codeg commit 的 [`connection.rs::map_prompt_blocks`](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs) 及 `types.rs`。RambleDesk 使用自己的输入校验和既有发送、取消、恢复与反馈续接流程，不采用厂商 capability 覆盖或二进制资源 fallback。

有序文字、图片、资源链接和嵌入文字资源由 [core prompt content](../crates/rambledesk-core/src/sessions/prompt_content.rs) 校验，再经 [ACP 映射](../crates/rambledesk-acp/src/prompt_content.rs) 发送。能力不足、格式或大小不合格在开始轮次和保存用户消息前拒绝；接受的输入不截断。图片限制按编码后的 base64 字节计算，媒体签名检查不等于完整图片解码；URI 不由映射层自动打开。持久化历史仍使用独立的有界预览，不能把预览当成重放载荷。

真实 stdio fixture 覆盖有序映射、图片原样传递、能力拒绝、大图片与有限历史、取消和旧纯文本入口。该证据不扩展为所有 Agent、模型或人工 UI 均已通过。
