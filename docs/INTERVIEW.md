# 设计访谈纪要（RambleDesk / User_0）

> 来源：与 Grok 的多轮产品讨论（2026-07-29）  
> 用途：保留决策上下文，便于后续 agent / 人类开发时对齐意图。
>
> **文档地位**：历史输入，不是现行产品或工程规范。若与
> [CONSTITUTION.md](CONSTITUTION.md)、[PROTOCOL.md](PROTOCOL.md) 或
> [ARCHITECTURE.md](ARCHITECTURE.md) 冲突，以后三者为准。User_0 是项目起源
> 方法论，不是 RambleDesk 核心领域名称。

---

## 1. 起点：vibe coding 与协作演进

- 提问者长期实践 vibe coding，并使用过从 Copilot 到后续几乎全部主流 coding agent 产品。  
- 认为「vibe coding」一词已偏旧，社区更多转向 agentic / harness 等表述。  
- 个人工作流特点：语音长时间 ramble；工作流 skill 化；重视移动端验收；CLI 迭代最轻；生产级项目谨慎，多为探索性项目。  

## 2. User_0 / User_1 模型

- **User_0**：构建者自己作为第一个真实用户，使用并提交反馈。  
- **毕业到 User_1**：系统具备完善日志、埋点、关键信息导出与上报后，再引入外部用户。  
- 结论倾向：开发者应把自己放到「零号用户」；最会用 AI 的人，也是最懂用 AI 替代自己的人。  
- 高复杂度 harness / graph engineering 完善后，人类上游工作变少——产品化「被呼叫的体验反馈」是主动设计这一角色，而非被动等待替代。  

## 3. 产品构想成形

- 将 User_0 理念扩展为：skill/工作流 + ramble 工具 + agent 上下文与通信组件。  
- Agent 朝目标自主迭代；需要人类时像调 API 一样请求 User_0。  
- 人类收到通知 → ramble → 产出图文 MD → agent 继续。  
- 「把人类包装成 API」是明确的设计隐喻。  

## 4. 协议与通道决策

| 议题 | 决策 |
|------|------|
| 请求通道 | 正式 CLI/MCP，而非仅聊天上下文 |
| 请求形态 | 固定：`what_happened` + 可执行 `actions[]`，减少人类思考 |
| 超时 | 不做强制超时；对齐 agent 可无限等人类的习惯 |
| 并发 | 架构可允许多请求；User_0 阶段同一 session 实际单 holding 即可 |
| 图片 | 反馈为文件夹：MD + 引用图；回传文件夹路径 |
| 落盘位置 | 默认项目内 `.user_0/feedback/...`，可选全局 |

## 5. MCP vs CLI；Skill vs Tool

- **MCP 为主**：与 daemon/工作台适配更好；工具为 agent 可发现的一等能力。  
- CLI 非必须（曾讨论作兜底，MVP 可省略）。  
- Skill 非必须：清晰 tool description 即可；Skill 可后续增强用法。  
- 闭环需要至少两工具：`request_feedback`（holding）+ `notify_complete`（结束通知）。  

## 6. 桌面应用与启动模式

- 需要 Tauri 跨平台桌面应用：语音、截图、session、日志、通知。  
- 曾讨论「薄 stdio 网关按需拉起工作台」vs「Figma 式工作台宿主 MCP」。  
- **MVP 选定 Figma 模式**：降低复杂度；工作台保持开启 = 待命；关闭则 MCP 不可用，agent 自行处理。  
- 多 agent 用 `agent` + `session_id` 区分。  

## 7. 命名

- 名字留宽，不把场景锁死在「仅开发」。  
- 弃用易冲突的 RambleStudio（已有设计师工作室占用）。  
- **产品名：RambleDesk**。  
- User_0 保留为协议与方法论用语。  

## 8. 竞品扫描结论（摘要）

- 语音 → MD / checklist（如 voice-to-md、Nodemind）、名为 Ramble 的 iOS webhook 笔记、开发者听写、user-feedback-mcp 等 HITL 均存在。  
- 缺口：结构化 agent 任务单 + 桌面体验式 ramble（语音+图）+ 同一 tool 回传。  

## 9. 自举开发与 holding 恢复

- 用 RambleDesk 开发 RambleDesk 时，应用/agent 重启会导致内存 holding 丢失。  
- **方案**：请求落盘；断开记为 `interrupted`；agent 再次 `request_feedback` 并带 `resume` / `request_id`，重建 waiting。  
- 草稿应一并恢复。已 completed 不可随意 resume。  

## 10. 明确不做的（提醒）

- 不把早期个人「Skill 教 CLI」项目架构强行绑到本产品。  
- 不追求第一版移动端。  
- 不自建完整 agent 内核。  

---

## 附录：核心用户原话取向（意译）

- 请求应走正式通道，格式固定，尽量把人类当 API，直接列出要做的 1..5。  
- 人类回复是 ramble，本身很自由。  
- 工作台开启的语义是「在等消息」，不是人必须泡在工作台里干活。  
- 关工作台则 MCP 挂掉是合理语义。  
- 恢复 holding 用「再调一次 + 恢复标签」即可接上，不必复活旧连接。  
