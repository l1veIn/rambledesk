# RambleDesk 适配器兼容矩阵

> 状态：Generic MCP adapter accepted · Pi local API package baseline
>
> 实测日期：2026-07-29（macOS 客户端矩阵）· 2026-07-31（Windows 自动化）
>
> 环境：macOS arm64 本机 loopback；Windows x64 storage/MCP 自动化

## 1. 结论

RambleDesk 当前有两条接入路径：

- 通用 MCP adapter：MCP Streamable HTTP `2025-11-25` wire profile，工具面只暴露
  `request_feedback` / `get_feedback` / `cancel_feedback`；
- Pi 原生 adapter：`packages/pi-rambledesk` Pi package，直接调用本地 JSON API
  `/api/feedback/request|get|wait|cancel`，在 Pi tool call 内等待终态。

服务端由官方 Rust SDK `rmcp` 3.0.0 实现，同时具备 `2026-07-28` 协议支持，但不能
假设目标 Agent 声明 Tasks 能力。

M0 验收成立：

- MCP Inspector 2.0.0 可列出通用 MCP adapter 工具；
- Claude Code 2.1.207 可通过自定义 Authorization header 调用通用 MCP 工具；
- 未认证、错误 token、未知 Origin 和未知 Host 均被拒绝；
- 服务只绑定 IPv4 loopback；
- 通用 MCP adapter 已通过 Inspector、官方 Rust SDK 和 Claude Code 的短调用验证；
- Pi local API 的 `request → wait → submit → completed package` 已通过 Rust 黑盒测试；
- Windows x64 已通过 Feedback Package write-through 发布、幂等提交、启动对账
  和官方 Rust SDK MCP 黑盒测试；
- Tasks 只在客户端显式声明支持后作为增强路径。

## 2. 客户端实测

| 客户端 | Transport | 协商协议 | 自定义 header | Tasks 声明 | 工具 | 结论 |
|--------|-----------|----------|---------------|------------|----------------|------|
| MCP Inspector 2.0.0 CLI | Streamable HTTP | `2025-11-25` | `--header` 通过 | `true` | create/get/cancel 通过 | 协议 smoke 与 CI 验证器 |
| Claude Code 2.1.207 | HTTP MCP | `2025-11-25` | 配置 `headers.Authorization` 通过 | `false` | create/get/cancel 通过 | 通用 adapter：提交后人工恢复 |
| Pi 0.83.0 | Local JSON API via package | 不适用 | Bearer token + `X-RambleDesk-Host: pi` | 不适用 | request/get/wait 通过服务端黑盒；package JS 单测通过 | 原生 adapter 候选：Pi tool call 内等待 |
| Codex CLI / OpenCode | HTTP MCP 可配置性未完整复测 | 未验证 | 未验证 | 未验证 | 未验证 | 先按通用 adapter 处理 |

Claude Code 使用 `--strict-mcp-config` 和只允许
RambleDesk 工具的非交互调用完成验证。M0 health 验证观察到
`clientSupportsTasks: false`；M1 黑盒验证创建、查询并取消同一 request，
三个响应的 `request_id` 一致，因此不把 Tasks 作为正确性前提。

官方 Rust SDK 的集成测试还覆盖稳定结构化错误，以及 `get_feedback` 在
`completed` 后取得 manifest、Markdown 和附件路径；
Inspector smoke 会校验实际 snake_case wire 字段、三个工具列表、认证失败，
以及两次 `SIGKILL` 前后的 `waiting → cancelled` 恢复。
`request_feedback` 仍返回 `execution_mode: "poll"` 兼容 handle；
Pi local API `/api/feedback/wait` 的终态结果返回 `execution_mode: "wait"`。客户端
单方面声明 Tasks 能力不会启用尚未完成回归的 Tasks 路径。

2026-07-29 本机 `/opt/homebrew/bin/codex` 在 `codex --version` 阶段即失败：
其 npm wrapper 尝试启动的 arm64 vendor binary 不存在并返回 `ENOENT`。
这是测试机的 Codex 安装问题，不是 RambleDesk transport 握手失败。修复或重装
Codex 后，应补跑同一 health matrix。2026-08-02 的 adapter 实测使用
`/Applications/ChatGPT.app/Contents/Resources/codex` 0.146.0-alpha.9.2，可正常创建和恢复
thread；Codex MCP 完整矩阵仍未补跑。

## 2.1 CLI resume 探针（不作为产品 adapter）

2026-08-02 曾验证过从外部 CLI 向 Claude Code、Codex、Pi、OpenCode 发送 resume
prompt 的可行性。结论是：能创建新进程或恢复 transcript 不等于能唤醒用户离开前的
原宿主上下文，因此这些探针不再注册为产品 `WakeupAdapter`。可重复探针位于
`scripts/host-adapter-e2e.sh`，默认不进 CI，因为会调用本机真实宿主和模型。

| 宿主 | 本机版本 | session id 获取 | wake / resume 证据 | 当前结论 |
|------|----------|-----------------|--------------------|----------|
| Claude Code | 2.1.207 | `--session-id <uuid>` 可由调用方指定；真实会话也可从 `~/.claude/projects/.../*.jsonl` 中按 `request_id` 反查 | `claude --resume <uuid> -p --output-format json <prompt>` 可恢复 transcript，但原终端不会自动刷新 | 探针可用；产品降级为通用 MCP |
| Codex CLI | 0.146.0-alpha.9.2 | `codex exec --json` 输出 `thread.started.thread_id` | `codex exec resume <thread_id> <prompt> --json` 跨 cwd 复用同一 thread | 探针可用；产品降级为通用 MCP |
| Pi | 0.83.0 | `--session-id <uuid>` 可指定；`--mode json` session event 返回同一 id | `pi --session <uuid> --print` 需要在原 project cwd；非默认 session-dir 还需要 `--session-dir` | CLI wake 探针废弃；产品改为 Pi package 原地等待 |
| OpenCode | 1.18.11 | `opencode run --format json` 输出 `sessionID`（`ses_...`） | `opencode run --dir <project> --session <sessionID>` 通过；不带 `--dir` 的跨 cwd 恢复卡住且无 JSON 事件 | 探针可用；产品降级为通用 MCP |

设计修正：

- `request_feedback` 没有新增宿主专用参数；通用 MCP 仍使用既有 `agent`、
  `session_id`、`project.root_path`。
- `WakeupRouter::default()` 当前不注册 Claude/Codex/Pi/OpenCode 专用 CLI wake
  adapter；无法可靠自动恢复原宿主上下文时必须弹出通用恢复提示。
- Pi package 不依赖 `session_id` 唤醒；`session_id` 仅用于展示和幂等输入 hash。
  真正的等待发生在同一个 Pi tool call 内。
- `/api/feedback/*` 与 `/mcp` 复用同一个 bearer token 和 loopback 安全策略。

## 3. 超时与取消

- M0 health 是立即完成的只读工具；Inspector 与 Claude 均正常完成，没有触发超时。
- 官方 Rust 客户端在调用后执行连接取消/关闭，服务端可正常优雅退出。
- M0 不包含长任务，因而不伪造客户端超时与业务取消结论。
- Feedback Request 生命周期独立于 HTTP 调用；断线或客户端超时不得取消 request。
  通用 MCP 默认不等待；业务取消通过 `cancel_feedback`，查询恢复通过 `get_feedback`。
- Pi local API 已验证一次 `/api/feedback/wait` 在提交后被唤醒；真实 Pi 长时等待、
  取消传播和桌面重启仍需人工/自动化补测，不能由短时自动化结果代替。
- Tasks 路径启用前，必须针对目标客户端补充 task create/get/result/cancel
  兼容回归。

## 4. 安全验证

| 检查 | 结果 |
|------|------|
| 监听地址 | `127.0.0.1`，动态或配置端口 |
| 无 Authorization | HTTP 401 + `WWW-Authenticate: Bearer` |
| 错误 bearer token | HTTP 401 |
| 未允许 Origin | HTTP 403 |
| 非 loopback Host | HTTP 403 |
| token 熵 | 32 随机 bytes，64 位十六进制文本 |
| token 文件权限 | Unix `0600` |
| token 日志 | 默认不输出；Debug 实现始终脱敏 |
| 请求体上限 | 256 KiB |

## 5. 锁定版本

| 组件 | M0 版本 |
|------|---------|
| Rust toolchain | 1.91.1 |
| Cargo package rust-version | 1.88 |
| `rmcp` | 3.0.0 |
| Tauri Rust | 2.11.5 |
| Tauri CLI | 2.11.4 |
| Tauri JS API | 2.11.1 |
| Node.js | 22.23.0 |
| pnpm | 10.12.4 |
| Svelte | 5.56.8 |
| Vite | 8.1.5 |
| TypeScript | 6.0.3 |
| MCP Inspector | 2.0.0 |

版本由 `rust-toolchain.toml`、workspace manifests、`Cargo.lock` 和
`pnpm-lock.yaml` 固定。依赖升级必须重新运行本页的客户端矩阵。

## 6. 可重复验证

官方 Rust SDK 自检：

```bash
pnpm mcp:self-test
```

Inspector 完整 smoke（启动临时服务、验证 401、列工具、调用 health、清理）：

```bash
pnpm mcp:inspector-smoke
```

常规门禁：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
pnpm check
pnpm test
pnpm build
pnpm contracts:check
```
