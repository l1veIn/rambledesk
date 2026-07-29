# RambleDesk 项目结构与技术形态（MVP）

## 1. 总体形态

```
用户打开 RambleDesk（Tauri）
        │
        └── 工作台进程内启动本地 MCP（HTTP，如 127.0.0.1:<port>/mcp）
                    │
        Codex / Claude Code 配置并连接该 MCP
                    │
        request_feedback  → UI 通知 + holding（可落盘）
        notify_complete   → 标记结束 + 通知
        resume            → interrupted → waiting
```

- **Figma 模式**：工作台是 MCP 宿主；先开工作台再开 agent。  
- **无独立常驻网关**；无「agent 自动拉起工作台」。  
- 状态、UI、MCP、反馈与请求落盘均在工作台一侧。  

## 2. 建议仓库目录

```
rambledesk/
├── README.md
├── docs/
│   ├── PRODUCT.md
│   ├── ARCHITECTURE.md
│   └── INTERVIEW.md
├── src-tauri/                 # Rust / Tauri
│   ├── src/
│   │   ├── main.rs
│   │   ├── mcp/               # 本地 MCP server（HTTP）
│   │   ├── session/           # session / request 状态机与落盘
│   │   ├── feedback/          # 反馈文件夹路径与写入约定
│   │   ├── notify/            # 系统通知等
│   │   └── storage/           # 日志、interrupted 队列
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                       # 前端 UI
│   ├── routes/ 或 pages/
│   │   ├── home/              # 待命首页
│   │   ├── sessions/
│   │   ├── ramble/            # 单次请求工作区
│   │   ├── history/
│   │   └── settings/
│   └── ...
└── ...
```

具体前端框架（React / Svelte 等）实现阶段再定；目录按信息架构对齐即可。

## 3. MCP 工具（契约级，schema 可随实现微调）

### 3.1 `request_feedback`（holding）

**用途**：发起或恢复一次 User_0 / ramble 请求。

建议输入概念字段：

| 字段 | 说明 |
|------|------|
| `agent` | 如 `codex`、`claude_code` |
| `session_id` | agent 会话/任务 id |
| `what_happened` | 客观描述当前状态 |
| `actions` | 字符串数组，可执行步骤 1..n |
| `request_id` | 可选；新建可服务端生成，resume 时必填或可解析 |
| `resume` | 可选 bool；为 true 时按 request_id 恢复 interrupted |
| `context_refs` | 可选 |

**输出概念**：

| 字段 | 说明 |
|------|------|
| `status` | `completed` / `cancelled` / … |
| `feedback_dir` | 反馈文件夹绝对或约定路径 |
| `request_id` | 稳定 id |

调用在人类提交（或取消）前保持 holding；工作台重启导致断开的请求记为 `interrupted`。

### 3.2 `notify_complete`

**用途**：通知阶段/目标结束，非 holding（MVP）。

建议字段：`agent`、`session_id`、`summary`、可选 `next_steps`。

### 3.3 可选只读

- 列出 `waiting` / `interrupted` 请求，便于 agent 或人类触发 resume。

## 4. 持久化

- **请求记录**：至少含 request_id、agent、session_id、参数快照、状态、时间戳、草稿路径。  
- **反馈包**：目录内 `feedback.md` + 图片；提交后路径回传 agent。  
- **日志**：请求/响应/恢复事件，便于自举开发时排障。  

## 5. 生命周期

| 事件 | 行为 |
|------|------|
| 工作台启动 | 加载落盘请求；启动 MCP；托盘显示状态 |
| 工作台退出 | MCP 停止；`waiting` → `interrupted`（可配置） |
| Agent 重连后 resume | 新 tool call + resume 标记 → 再进入 waiting |

## 6. 实现优先级建议

1. Tauri 壳 + 托盘 + MCP 空服务（可被 agent 发现）  
2. `request_feedback` holding + 最小 UI 任务单 + 手动文本提交（先不语音）  
3. 反馈文件夹落盘与路径回传  
4. 请求落盘 + interrupted + resume  
5. 语音 / 截图 / 通知打磨  
6. `notify_complete` 与历史浏览  
