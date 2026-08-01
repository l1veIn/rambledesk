# RambleDesk 产品文档（MVP）

> 工作名：RambleDesk  
> 历史方法论词汇：User_0（零号用户）
> 版本：开发基线 v1 · 2026-07-29

---

## 1. 背景

- Coding agent（Codex、Claude Code 等）能力持续增强，开发者角色从「写代码」转向「定目标、设约束、当第一个真实用户（User_0）」。
- 现有协作仍以聊天上下文为主：agent 提问淹没在对话里，人类反馈缺少正式通道与结构化产物。
- 开发者已有成熟习惯：长时间语音 ramble、边用边说、配截图；缺的是把这套习惯接进 agent 循环的工作台与协议。
- 开源侧已有听写工具、HITL MCP、语音笔记，但缺少「agent 下发任务单 → 人类体验式反馈 → 同一会话回传」的闭环产品。

---

## 2. 问题

1. **呼叫不正式**：agent 需要人类判断时，只能写在聊天里，无固定格式、无等待态、无明确行动清单。
2. **反馈不结构化**：人类回复多为碎片文字，缺少「按清单操作 + 语音 ramble + 截图」的统一产物。
3. **上下文易断**：反馈常靠事后贴文件或手动续跑，难以保证 agent 在同一 tool call / 同一会话中继续。
4. **体验者角色未产品化**：没有桌面级工具把待命、收请求、真实体验和交反馈变成默认工作流。

---

## 3. 方案

**RambleDesk**：面向 agent 的体验式反馈工作台。

- **桌面应用（Tauri）**：待命、收请求、ramble（语音 + 截图）、管理 session 与历史。
- **本地 MCP（工作台宿主，Figma 模式）**：agent 连接后可调用正式工具；工作台关闭则 MCP 不可用。
- **核心协议**：
  - `request_feedback`：幂等创建体验请求并立即返回 durable handle。
  - `wait_for_feedback`：一次挂起等待终态并返回完整反馈包，避免 token 空轮询。
  - `get_feedback` / `list_feedback_requests`：兼容、断线恢复和诊断查询。
  - `cancel_feedback`：显式取消。
  - `notify_complete`：目标结束或阶段完成时通知人类。
- **反馈产物**：每个请求对应一个不可变目录（`feedback.md` +
  `manifest.json` + 引用图片），URI 和路径作为 tool 结果返回。
- **产品语义**：工作台开启 = 人类处于可被呼叫的待命状态；不是「人必须一直在工作台里操作」。
- **连接语义**：请求状态先落盘，MCP 连接和工具调用只是交付方式；断线不删除或
  改写请求。

### 3.1 与 User_0 的关系

- **起源与主战场**：coding agent 时代，开发者成为零号用户。
- **能力本身**：可扩展的「Agent 呼叫人类做体验反馈」协议 + ramble 工作台，
  不绑定只能写代码。
- **边界**：User_0 只保留为方法论与 dogfooding 语境，不进入产品名、领域对象、
  MCP schema 或默认数据目录。现行约束见 [CONSTITUTION.md](CONSTITUTION.md)。

---

## 4. 范围（MVP）

| 模块 | 内容 |
|------|------|
| 平台 | 桌面：Windows / macOS / Linux（Tauri） |
| MCP | 同机 Streamable HTTP MCP，由工作台进程提供；引导配置到 Codex、Claude Code |
| 工具 | `request_feedback`、`wait_for_feedback`、`get_feedback`、`list_feedback_requests`、`cancel_feedback`、`notify_complete` |
| Ramble | 语音录入与转写、截图、编辑 MD、提交回传 |
| Session | 列表与详情（按 project + `agent` + `session_id` 区分）；当前未结束请求 |
| 存储 | 请求与状态落盘；Feedback Package；基础日志 |
| 恢复 | Request 持久化 + `request_id` 幂等重连；阻塞等待可安全重试，查询接口兜底 |
| 通知 | 系统通知 + 可选响铃；自定义 channel 预留 |
| 分发 | 安装工作台 → 保持开启 → 配置 MCP → 正常使用 agent |

---

## 5. 非目标（MVP 不做）

- 独立常驻 MCP 网关、agent 自动拉起工作台
- 依赖无限 HTTP 连接才能正确工作
- 多人类角色（User_1 / User_2）与权限体系
- 移动端完整 App
- 内置完整 agent runtime（不替代 Codex / Claude Code）
- 强制 Skill 注入（靠 tool description；Skill 可选后续）
- 云端同步、账号体系、多人协作
- LLM 后处理流水线（可后续加）
- 通用系统级听写（不做 Wispr 类竞品）
- 远程 Agent 与桌面之间的文件同步

---

## 6. 主流程

### 6.1 安装与待命

1. 安装并打开 RambleDesk  
2. 配置/复制 MCP 到 Codex 或 Claude Code  
3. 工作台保持开启（可托盘），进入待命  

### 6.2 请求反馈（主路径）

1. Agent 调用 `request_feedback`（含 `agent`、`session_id`、project、`what_happened`、`actions`，可选 `request_id`）
2. 工作台通知用户，展示任务单；状态为 `waiting`  
3. 用户按 `actions` 操作目标软件，边用边 ramble，按需截图  
4. 草稿持续落盘；提交时原子生成 Feedback Package
5. Agent 单次调用 `wait_for_feedback`；提交或取消后一次性取得完整结果
6. Agent 获得 package URI/路径，在同一任务中继续迭代

### 6.3 结束通知

1. Agent 调用 `notify_complete`（`summary` 等）  
2. 工作台通知并标记 session 结束  
3. 用户可查看历史，无需强制再回复  

### 6.4 异常与恢复

- 工作台未开或已关 → MCP 不可用，Agent 可稍后用相同 `request_id` 重试。
- MCP 断线或 Agent 超时只结束一次 Invocation Attempt；Feedback Request 状态不变。
- 工作台重启从 SQLite 和 draft 目录恢复未结束请求。
- 相同 `request_id` + 相同不可变输入重新调用会关联现有请求；输入不同返回 conflict。
- 已 `completed` 的请求返回原结果；已 `cancelled` 的请求不隐式重新打开。

状态机（简）：

```
waiting → in_progress → completed
   │            │
   └────────────┴──────→ cancelled
```

Request 的正确性不依赖单次 holding 连接。wait、Tasks、兼容查询和重试必须读取
同一持久化状态；客户端不得用固定间隔空轮询作为默认等待路径。

---

## 7. 信息架构（桌面 UI）

```
RambleDesk
├── 待命 / 首页
│   ├── 当前状态（MCP 在线与否、待处理数量）
│   └── 快捷入口（待处理请求）
├── 请求 / Session
│   ├── Session 列表（agent、session_id、状态、时间）
│   └── Session 详情
│       ├── 当前/历史 request（what_happened、actions）
│       └── 已提交反馈入口
├── Ramble（单次请求工作区）
│   ├── 任务单（actions 清单，只读）
│   ├── 语音录制 / 转写
│   ├── 截图管理
│   ├── feedback.md 预览与轻编辑
│   └── 提交 / 取消
├── 历史
│   └── 按时间/项目浏览反馈文件夹
├── 设置
│   ├── MCP 地址与配置引导
│   ├── 通知（响铃、系统通知、自定义 channel 预留）
│   ├── 反馈存储路径约定
│   └── 语音/转写相关选项
└── 托盘
    ├── 在线状态
    ├── 待处理角标
    └── 打开主窗口 / 退出
```

**关键对象**

- **Session**：`agent` + `session_id`，状态如 idle / waiting / completed / ended  
- **Request**：一次反馈请求，含 `request_id`、actions、状态（waiting / in_progress / completed / cancelled）
- **Invocation Attempt**：一次 MCP 调用尝试，用于诊断连接/取消，不决定 Request 状态
- **Feedback Package**：不可变目录（`feedback.md` + `manifest.json` + attachments），一次提交对应一份

### 反馈落盘约定（默认）

项目内优先：

```
.rambledesk/feedback/<timestamp>-<request-id>/
  feedback.md
  manifest.json
  attachments/...
```

项目不可写或未提供时落到 RambleDesk 应用数据目录。MVP 仅保证同机、共享文件系统
的 Agent 能访问返回路径。

---

## 8. 请求字段原则

- **请求侧固定、少发挥**：`what_happened` + 编号清晰的 `actions[]`，尽量可直接执行，减少人类思考。  
- **回复侧自由**：人类用 ramble 表达，产物为 MD + 图。  
- 多 agent：请求必须带 `agent`（如 `codex` / `claude_code`）与 `session_id`；
  二者只用于关联，不是认证信息。

---

## 9. 成功指标（草案）

- 完整闭环次数（请求 → 提交 → agent 继续）  
- 从通知到提交的中位时长  
- request_id 重试/恢复成功率
- agent 侧因工作台关闭导致的失败率（可观测即可，MVP 不优化到零）  

---

## 10. 竞品差异（摘要）

| 类型 | 解决什么 | 缺什么 |
|------|----------|--------|
| 听写 / Ramble 笔记 | 说得快、脑暴成文 | 无 agent 任务单与持久结果回传 |
| HITL MCP | agent 能喊人 | 多为短文本/审批，非体验式图文 |
| **RambleDesk** | agent 喊来的人，用真实使用 + ramble 交回正式产物 | — |
