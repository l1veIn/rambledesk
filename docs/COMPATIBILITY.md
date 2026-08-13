# RambleDesk 适配器验证矩阵

> 状态：Generic MCP Adapter 与 Pi Native Adapter 当前验证基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。

## 接入路径

RambleDesk 当前提供两类适配器：

| 适配器 | Transport | 等待模型 | continuation |
| --- | --- | --- | --- |
| Generic MCP Adapter | Streamable HTTP `/mcp` | 创建请求后立即返回 | 人类提交后复制 Resume Prompt，宿主调用 `get_feedback` |
| Pi Native Adapter | Local JSON API `/api/feedback/*` | Pi tool call 内调用 `wait` | 不需要额外 continuation |

两类 transport 都由 `rambledesk-local-server` 挂载，共用 loopback、bearer token、
Host header 和 Origin guard。MCP 只属于通用适配器，不是全局基础设施。

## 已验证能力

### Generic MCP Adapter

- MCP Inspector 可以列出并调用 `request_feedback`、`get_feedback`、
  `cancel_feedback`；
- Claude Code 可以通过自定义 Authorization header 调用工具；
- 官方 Rust SDK 集成测试覆盖工具调用、结构化错误和完成结果；
- 未认证、错误 token、未知 Origin 和非 loopback Host 均被拒绝；
- 断线不会取消已持久化 request；
- 终态 `get_feedback` 返回反馈包 metadata、Markdown 和附件路径。

通用适配器不承诺自动恢复某个宿主的原上下文。提交后的标准路径是工作台生成
Resume Prompt，由人类返回宿主后继续；支持原生交互确认工具（`ask`/`ask_choice`
类）的宿主可让智能体在工具调用内等待人类完成，点选确认后直接 `get_feedback`
继续，无需 Resume Prompt（见 PROTOCOL.md 的 Generic MCP Adapter 节）。

### Pi Native Adapter

- `packages/pi-rambledesk` 调用 request/get/wait/cancel；
- `X-RambleDesk-Host: pi` 将请求归属到 Pi Host Profile；
- request 重试复用同一 `request_id`，不会产生重复请求；
- 服务端黑盒测试覆盖 request → wait → submit → completed package；
- JavaScript 测试覆盖输入映射、token 读取、重试和取消。

Pi 的正常流程在同一个 tool call 内等待，因此无需提交后的 Resume Prompt。

## 当前客户端矩阵

| 宿主 | 适配器 | 状态 | 备注 |
| --- | --- | --- | --- |
| Pi 0.83.x | Pi Native Adapter | 自动化基线通过 | 真实长时等待与桌面重启需继续人工回归 |
| Claude Code 2.1.x | Generic MCP Adapter | 工具调用通过 | 提交后使用 Resume Prompt；自动配置时向 `.claude/skills` 注入 `ramble` skill |
| MCP Inspector 2.x | Generic MCP Adapter | smoke 通过 | 用于协议和安全门禁 |
| Codex CLI | Generic MCP Adapter | 待补完整矩阵 | 按通用适配器合同处理 |
| OpenCode | Generic MCP Adapter | 待补完整矩阵 | 按通用适配器合同处理 |
| Reasonix (Go, v1.8+) | Generic MCP Adapter | 自动检测+安装已实现 | 写入 `config.toml` 的 `[[plugins]]` HTTP 条目；持久会话下提交后"继续"即恢复；自动配置时向 `.agents/skills` 注入 `ramble` skill |
| Grok CLI | Generic MCP Adapter | 自动检测+安装已实现 | 写入 `~/.grok/config.toml`（或 `GROK_HOME`）的 `[mcp_servers.rambledesk]` HTTP 条目；提交后使用 Resume Prompt，或用 `ask_user_question` 等待后再 `get_feedback` |

版本号仅记录已测环境，不构成 RambleDesk 对第三方版本的长期保证。

## 反馈闭环（skill 注入）

目标场景：打开新会话 → 最小化宿主终端 → 之后所有需要用户参与的交互只出现在
RambleDesk。

通用 MCP 适配器在「自动配置 MCP」时，除写入 MCP server 配置外，还会把一个遵循
Agent Skills 开放标准（[agentskills.io](https://agentskills.io)）的 `ramble` skill
复制到各宿主的**全局 skill 目录**（`~/.claude/skills/ramble/SKILL.md` 等），由
宿主启动时自动发现、按需加载。skill 内容纯教宿主走 RambleDesk 反馈循环
（`request_feedback` → 等待 → `get_feedback` → 实现 → 必要时 `cancel_feedback`），
不包含会话恢复逻辑。

| 宿主 | skill 目录（home 相对） |
| --- | --- |
| Claude Code | `.claude/skills` |
| Codex | `.codex/skills` |
| Cursor | `.cursor/skills` |
| Gemini CLI | `.gemini/skills` |
| Grok CLI | `.grok/skills` |
| OpenCode | `.config/opencode/skills` |
| Reasonix | `.agents/skills` |

### 恢复与 continuation

- 断线/重启不改变已持久化 request 的生命周期；用相同 `request_id` 调
  `get_feedback` 即可读到服务端事实。
- 宿主恢复后「所有交互只走 RambleDesk」是否延续，取决于宿主是否保留会话上下文：
  - Reasonix 持久会话：提交后"继续"即恢复原上下文。
  - Claude Code 及其他靠 Resume Prompt 的宿主：恢复后需重新注入上下文。

## 安全基线

- listener MUST 只绑定 IPv4 loopback；
- 每个 `/mcp` 与 `/api` 请求 MUST 验证 bearer token；
- Host MUST 是允许的 loopback host；
- 浏览器来源请求 MUST 通过 Origin allowlist；
- token 文件 MUST 使用用户私有权限；
- 默认日志 MUST NOT 记录 token、反馈正文或附件内容；
- `host_id` 可以由可信安装入口或 `X-RambleDesk-Host` 覆盖，但不能作为认证凭据。

对应自动化位于：

- `crates/rambledesk-local-server/tests/http_security.rs`
- `crates/rambledesk-mcp/src/lib.rs`
- `packages/pi-rambledesk/test/`
- `scripts/mcp-inspector-smoke.sh`

## 失败与恢复

- HTTP 断线只终止当前 transport attempt，不改变 request 生命周期；
- 相同 `request_id` 和相同不可变输入重新请求会返回现有状态；
- 相同 `request_id` 和不同不可变输入返回 `REQUEST_CONFLICT`；
- 取消必须显式调用 `cancel_feedback`；
- Generic MCP Adapter 不维持长连接状态，断线后直接通过 `get_feedback(request_id)` 读取服务端事实；
- Pi Native Adapter 通过服务端 recovery contract 恢复原生等待，并可用相同 `request_id` 重新进入 `wait`；
- Pi 未提供 `request_id` 且同一 host session 存在多个候选时返回 `RECOVERY_AMBIGUOUS`，服务端不会猜测；
- SQLite 与不可变反馈包是恢复事实来源。

## 仍需人工验收

- Pi 真实 tool call 的长时间等待、取消传播和桌面重启恢复；
- Generic MCP Adapter 在 Codex CLI 与 OpenCode 中的安装、认证和完整请求闭环；
- macOS/Windows 安装包中的 token 权限、loopback 防护和 adapter 配置复制；
- tray 入口、Resume Prompt 复制和宿主返回后的完整人类路径。
