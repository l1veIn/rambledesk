# RambleDesk MCP 兼容矩阵

> 状态：M0 transport accepted · M1 persistent request tools validated
>
> 实测日期：2026-07-29
>
> 环境：macOS arm64，本机 loopback

## 1. 结论

RambleDesk 首发使用 MCP Streamable HTTP `2025-11-25` wire profile 和
polling 业务模式。服务端由官方 Rust SDK `rmcp` 3.0.0 实现，同时具备
`2026-07-28` 协议支持，但不能假设目标 Agent 声明 Tasks 能力。

M0 验收成立：

- MCP Inspector 2.0.0 可列出并调用 `rambledesk_health`；
- Claude Code 2.1.207 可通过自定义 Authorization header 调用该工具；
- 未认证、错误 token、未知 Origin 和未知 Host 均被拒绝；
- 服务只绑定 IPv4 loopback；
- M1 的 `request_feedback/get_feedback/cancel_feedback` 已按 durable request +
  polling 模式通过 Inspector、官方 Rust SDK 和 Claude Code；
- Tasks 只在客户端显式声明支持后作为增强路径。

## 2. 客户端实测

| 客户端 | Transport | 协商协议 | 自定义 header | Tasks 声明 | M1 polling 工具 | 结论 |
|--------|-----------|----------|---------------|------------|----------------|------|
| MCP Inspector 2.0.0 CLI | Streamable HTTP | `2025-11-25` | `--header` 通过 | `true` | create/get/cancel 通过 | 协议 smoke 与 CI 验证器 |
| Claude Code 2.1.207 | HTTP MCP | `2025-11-25` | 配置 `headers.Authorization` 通过 | `false` | `waiting/waiting/cancelled` 通过 | 首发必须支持 polling |
| Pi 0.79.6 | 无内建 MCP 配置入口 | 未验证 | 未验证 | 未验证 | 未运行 | 可由扩展接入，但本轮不安装扩展、不计为原生兼容 |
| Codex CLI（本机安装） | 未能启动 | 未验证 | 未验证 | 未验证 | 未验证 | 环境阻塞，不判定产品不兼容 |

Claude Code 使用 `--strict-mcp-config` 和只允许
RambleDesk 工具的非交互调用完成验证。M0 health 验证观察到
`clientSupportsTasks: false`；M1 黑盒验证创建、查询并取消同一 request，
三个响应的 `request_id` 一致，因此不把 Tasks 作为正确性前提。

官方 Rust SDK 的集成测试还覆盖稳定结构化错误；Inspector smoke 会校验实际
snake_case wire 字段、四个工具列表、认证失败，以及两次 `SIGKILL` 前后的
`waiting → cancelled` 恢复。
Inspector 虽声明 Tasks 能力，当前服务仍按 v1 合同返回
`execution_mode: "poll"`；客户端单方面声明能力不会启用尚未完成回归的增强路径。

本机 `/opt/homebrew/bin/codex` 在 `codex --version` 阶段即失败：
其 npm wrapper 尝试启动的 arm64 vendor binary 不存在并返回 `ENOENT`。
这是测试机的 Codex 安装问题，不是 RambleDesk transport 握手失败。修复或重装
Codex 后，应补跑同一 health matrix。

## 3. 超时与取消

- M0 health 是立即完成的只读工具；Inspector 与 Claude 均正常完成，没有触发超时。
- 官方 Rust 客户端在调用后执行连接取消/关闭，服务端可正常优雅退出。
- M0 不包含长任务，因而不伪造客户端超时与业务取消结论。
- M1 的 Feedback Request 生命周期独立于 HTTP 调用；断线或客户端超时不得取消
  request。业务取消通过 `cancel_feedback`，查询通过 `get_feedback`。
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
