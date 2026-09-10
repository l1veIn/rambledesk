# 外部反馈适配器兼容性

本文说明用户自己运行的 Agent 如何接入 RambleDesk。由工作台启动和管理的 Agent 使用 [ACP 托管会话](ACP_MANAGED_SESSIONS.md)，不套用这里的外部身份与续接方式。术语以 [TERMINOLOGY.md](TERMINOLOGY.md) 为准，当前实测与未验项见[质量清单](quality/README.md)。

## 三个适配器、两种传输

| 适配器 | Transport | 创建与等待 | 提交后的继续方式 |
| --- | --- | --- | --- |
| Generic MCP Adapter | Streamable HTTP `/mcp` | `request_feedback` 创建后立即返回 | 将 Resume Prompt 带回原宿主，再调用 `get_feedback` |
| Pi Native Adapter | Local JSON API `/api/feedback/*` | 原生 tool call 内 request + wait | 同一次调用收到反馈，无额外托管续接 |
| dsh Native Adapter | Local JSON API `/api/feedback/*` | Cordis 插件的 tool call 内 request + wait | 同一次调用收到反馈，可按原 request ID 恢复 |

两种传输由 `rambledesk-local-server` 的 Local Integration Server 提供，共用 loopback、bearer token、Host 和 Origin guard。Web Access 是另一种客户端接入边界；适配器兼容不代表浏览器拥有系统截图、全局快捷键、托盘、更新器或本地文件路径能力，见 [Web Access 支持矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)。

## 安装与使用

在「设置 → 外部适配器」检测并安装对应入口。Generic MCP 自动配置写入宿主的 MCP 配置，并安装共享 `ramble` skill；dsh 原生安装也提供该 skill，插件在重启 dsh 后生效。实际目标路径与冲突以安装界面的检查结果为准，不把宿主目录布局当作长期协议。

- **Generic MCP**：适用于 Claude Code、Codex CLI、OpenCode、Reasonix、Grok 等支持相应 HTTP MCP 配置的宿主。可列出并调用 `request_feedback`、`get_feedback`、`cancel_feedback`；终态读取返回反馈包 metadata、Markdown 和附件路径。安装入口存在、工具可列出、真实反馈闭环通过是三种不同结论。
- **Pi**：使用 [`packages/pi-rambledesk`](../packages/pi-rambledesk/README.md)。原生工具在调用内等待，支持按 request ID 读取、恢复和取消。
- **dsh**：使用 [`packages/dsh-rambledesk`](../packages/dsh-rambledesk/README.md)。`request_ramble_feedback` 创建并等待，`resume_ramble_feedback` 恢复等待，另有读取和取消工具；共享 `/ramble` 用于当前任务，`/ramble_on`、`/ramble_off` 控制插件的持续模式。

共享 skill 根据当前可用工具选择原生等待或 Generic MCP 手动续接。它帮助 Agent 遵循流程，不保证每个模型都会执行，也不会把 Generic MCP 变成阻塞到人类提交的工具调用。

支持独立交互确认工具的宿主可以在创建请求后，用自己的 ask 类工具等待人类确认，再读取原 request 的反馈；等待由宿主负责。标准后备路径仍是 Resume Prompt。宿主关闭或重启后是否保留原任务上下文由宿主决定，RambleDesk 不承诺一句“继续”即可恢复；合同见 [PROTOCOL.md](PROTOCOL.md)。

## 安全与恢复合同

- Local Integration Server 只绑定 IPv4 loopback；`/mcp` 与 `/api` 验证 bearer token、允许的 Host 和浏览器 Origin。token 文件使用用户私有权限，默认日志不记录 token、反馈正文或附件内容。
- `host_id` 和 `X-RambleDesk-Host` 用于来源归属，不是认证凭据。
- 相同 request ID 与相同不可变输入重试返回已有状态；不同输入返回 `REQUEST_CONFLICT`，不覆盖原请求。
- HTTP 断线或中断当前等待不自动取消持久请求；取消必须显式调用对应取消工具。读取和恢复使用原 request ID，SQLite 与不可变反馈包是事实来源。
- 原生等待可重新进入；缺少 request ID 且同一 host session 有多个候选时返回 `RECOVERY_AMBIGUOUS`，不猜测归属。
- 外部原生等待与托管投递分开路由，一次反馈不会同时走原生返回和 ACP 自动续接。

## 验证入口与限制

协议、安全门禁、幂等和恢复可通过以下代码复验：

- [HTTP 安全测试](../crates/rambledesk-local-server/tests/http_security.rs)与 [MCP 实现及测试](../crates/rambledesk-mcp/src/lib.rs)。
- [Pi 测试](../packages/pi-rambledesk/test/)和 [dsh 测试](../packages/dsh-rambledesk/test/)。
- [MCP Inspector smoke](../scripts/mcp-inspector-smoke.sh)。

这些自动化不等于所有宿主版本和平台均已通过人工闭环。Pi/dsh 的真实长时等待、取消传播和重启恢复，Generic MCP 在各宿主中的安装、认证、请求、提交与返回原任务，以及正式安装包中的 token 权限与配置写入，均按[质量清单](quality/README.md)登记具体构建和结果。既有版本的成功不能外推到下一版本；Desktop/Browser 媒体与设备能力另按支持矩阵验收。
