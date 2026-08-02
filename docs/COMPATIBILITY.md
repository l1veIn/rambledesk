# RambleDesk MCP 兼容矩阵

> 状态：M0 transport accepted · M1 persistent request + blocking wait validated
>
> 实测日期：2026-07-29（macOS 客户端矩阵）· 2026-07-31（Windows 自动化）
>
> 环境：macOS arm64 本机 loopback；Windows x64 storage/MCP 自动化

## 1. 结论

RambleDesk 使用 MCP Streamable HTTP `2025-11-25` wire profile 和 durable
blocking-wait 业务模式。服务端由官方 Rust SDK `rmcp` 3.0.0 实现，同时具备
`2026-07-28` 协议支持，但不能假设目标 Agent 声明 Tasks 能力。

M0 验收成立：

- MCP Inspector 2.0.0 可列出并调用 `rambledesk_health`；
- Claude Code 2.1.207 可通过自定义 Authorization header 调用该工具；
- 未认证、错误 token、未知 Origin 和未知 Host 均被拒绝；
- 服务只绑定 IPv4 loopback；
- M1 原有 polling 工具已通过 Inspector、官方 Rust SDK 和 Claude Code；新增
  `wait_for_feedback` 已通过官方 Rust SDK 黑盒测试，目标宿主长等待仍需补测；
- Windows x64 已通过 Feedback Package write-through 发布、幂等提交、启动对账
  和官方 Rust SDK MCP 黑盒测试；
- Tasks 只在客户端显式声明支持后作为增强路径。

## 2. 客户端实测

| 客户端 | Transport | 协商协议 | 自定义 header | Tasks 声明 | M1 工具 | 结论 |
|--------|-----------|----------|---------------|------------|----------------|------|
| MCP Inspector 2.0.0 CLI | Streamable HTTP | `2025-11-25` | `--header` 通过 | `true` | create/get/cancel 通过 | 协议 smoke 与 CI 验证器 |
| Claude Code 2.1.207 | HTTP MCP | `2025-11-25` | 配置 `headers.Authorization` 通过 | `false` | `waiting/waiting/cancelled` 通过 | 首发必须支持 polling |
| Pi 0.79.6 | 无内建 MCP 配置入口 | 未验证 | 未验证 | 未验证 | 未运行 | 可由扩展接入，但本轮不安装扩展、不计为原生兼容 |
| Codex CLI（本机安装） | 未能启动 | 未验证 | 未验证 | 未验证 | 未验证 | 环境阻塞，不判定产品不兼容 |

Claude Code 使用 `--strict-mcp-config` 和只允许
RambleDesk 工具的非交互调用完成验证。M0 health 验证观察到
`clientSupportsTasks: false`；M1 黑盒验证创建、查询并取消同一 request，
三个响应的 `request_id` 一致，因此不把 Tasks 作为正确性前提。

官方 Rust SDK 的集成测试还覆盖稳定结构化错误，以及单次
`wait_for_feedback` 在 `waiting → operator submit → completed` 后取得 manifest、
Markdown 和附件路径；
Inspector smoke 会校验实际 snake_case wire 字段、四个工具列表、认证失败，
以及两次 `SIGKILL` 前后的 `waiting → cancelled` 恢复。
`request_feedback` 仍返回 `execution_mode: "poll"` 兼容 handle；
`wait_for_feedback` 的终态结果返回 `execution_mode: "wait"`。客户端单方面声明
Tasks 能力不会启用尚未完成回归的 Tasks 路径。

2026-07-29 本机 `/opt/homebrew/bin/codex` 在 `codex --version` 阶段即失败：
其 npm wrapper 尝试启动的 arm64 vendor binary 不存在并返回 `ENOENT`。
这是测试机的 Codex 安装问题，不是 RambleDesk transport 握手失败。修复或重装
Codex 后，应补跑同一 health matrix。2026-08-02 的 adapter 实测使用
`/Applications/ChatGPT.app/Contents/Resources/codex` 0.146.0-alpha.9.2，可正常创建和恢复
thread；Codex MCP 完整矩阵仍未补跑。

## 2.1 专有 wake adapter 实测（2026-08-02）

本轮验证目标不是 MCP transport 本身，而是提交终态后能否把“继续”送回原宿主会话。
可重复探针位于 `scripts/host-adapter-e2e.sh`；默认不进 CI，因为会调用本机真实宿主和模型。
完整 Claude MCP 闭环使用 `RAMBLEDESK_E2E_MCP=1 scripts/host-adapter-e2e.sh claude-full`。

| 宿主 | 本机版本 | session id 获取 | wake / resume 证据 | 当前结论 |
|------|----------|-----------------|--------------------|----------|
| Claude Code | 2.1.207 | `--session-id <uuid>` 可由调用方指定；真实会话也可从 `~/.claude/projects/.../*.jsonl` 中按 `request_id` 反查 | `claude --resume <uuid> -p --output-format json <prompt>` 可恢复同一会话并拉取 completed package；`--background` 会创建独立 background agent，不会把输出写回原 TTY | B 级：可自动投递；原终端不会自动刷新，需重新打开/查看 transcript 才能看到恢复结果 |
| Codex CLI | 0.146.0-alpha.9.2 | `codex exec --json` 输出 `thread.started.thread_id` | `codex exec resume <thread_id> <prompt> --json` 跨 cwd 复用同一 thread | B 级：可自动投递；session id 需从 CLI 事件或宿主上下文登记 |
| Pi | 0.83.0 | `--session-id <uuid>` 可指定；`--mode json` session event 返回同一 id | `pi --session <uuid> --print` 需要在原 project cwd；非默认 session-dir 还需要 `--session-dir` | B/C 边界：可自动投递，但 adapter 必须带 project cwd，定制 session-dir 需显式配置 |
| OpenCode | 1.18.11 | `opencode run --format json` 输出 `sessionID`（`ses_...`） | `opencode run --dir <project> --session <sessionID>` 通过；不带 `--dir` 的跨 cwd 恢复卡住且无 JSON 事件 | B/C 边界：可自动投递，但必须带 project dir 或 attach 到正确 server |

设计修正：

- `request_feedback` 没有新增宿主专用参数；仍使用既有 `agent`、`session_id`、`project.root_path`。
- Claude Code 不能信任模型主动填入的 `session_id`：真实 dogfood 中模型传入了
  `test-session-001`，导致 adapter resume 到无效目标。Claude adapter 现在会按
  `request_id` 扫描对应项目 transcript，反查真实 `sessionId`；只有找不到时才退回
  MCP payload 中的 `session_id`。
- Claude Code 的自动恢复使用 print mode，而不是 background mode。若原 transcript
  显示该会话已处于 `bypassPermissions`，adapter 会沿用
  `--permission-mode bypassPermissions`；否则只允许
  `mcp__rambledesk__get_feedback` 并使用 `--permission-mode dontAsk`，避免后台卡权限确认。
- `agent` 不能只依赖 `X-RambleDesk-Host` 自动覆盖；真实 Claude MCP 路径可完成调用，但不应假设所有客户端转发自定义 header。因此专用安装/提示仍要求宿主传入稳定 host id。
- `project.root_path` 已进入 wake payload，供 Pi/OpenCode 这类按项目 cwd 解析 session 的宿主使用；wake payload 仍不携带 Package 正文。
- Pi 的非默认 session-dir 通过 `RAMBLEDESK_PI_SESSION_DIR` 或 `PI_CODING_AGENT_SESSION_DIR` 传给 adapter。

## 3. 超时与取消

- M0 health 是立即完成的只读工具；Inspector 与 Claude 均正常完成，没有触发超时。
- 官方 Rust 客户端在调用后执行连接取消/关闭，服务端可正常优雅退出。
- M0 不包含长任务，因而不伪造客户端超时与业务取消结论。
- M1 的 Feedback Request 生命周期独立于 HTTP 调用；断线或客户端超时不得取消
  request。默认等待通过 `wait_for_feedback`，超时后可安全重试；业务取消通过
  `cancel_feedback`，查询恢复通过 `get_feedback`。
- 官方 Rust SDK 已验证一次等待在提交后被唤醒；Claude Code、Codex 和 Inspector
  的长时超时上限、取消传播仍需分别实测，不能由短时自动化结果代替。
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
