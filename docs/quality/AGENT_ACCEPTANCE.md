# 既有 Agent 真实验收

> CURRENT：2026-09-09。Q19 的真实模型后端探针已执行；完整反馈闭环未通过。
> 登录、ACP 连接、实际模型调用、反馈提交与同一会话恢复分别记录，不互相代替。

## 范围与入口

使用 [managed_loop.rs](../../crates/rambledesk-local-server/examples/managed_loop.rs) 的显式 opt-in 入口，
通过实际 `SqliteFeedbackStore`、`SessionApplication`、`AcpSessionDriver`、本地反馈服务和 delivery worker，
为每种接入创建两个独立项目、两个 Agent 会话和一个新的临时数据库。每个实例的预期路线为：

1. 模型调用内置反馈命令创建请求，并结束当前轮次。
2. 提交各自唯一 marker，生产 continuation worker 唤回同一 Agent 会话；模型读取反馈包并回复 marker。
3. 停止/重连、关闭后重新打开数据库，再显式恢复原 remote session；模型重新读包。

探针还核对 sibling 隔离、删除清理和退出时停止子进程。遇到 Agent 权限请求即停止，
不自动批准工具，不使用 full-access 或自动审批策略来取得通过结果。

构建使用工作区锁文件：

```sh
cargo build --locked --offline -p rambledesk-local-server --example managed_loop
RAMBLEDESK_MANAGED_PROBE_RUN=1 \
RAMBLEDESK_MANAGED_PROBE_LAUNCH=/absolute/private/launch.json \
RAMBLEDESK_MANAGED_PROBE_RUN_DIR=/absolute/new/probe-directory \
target/debug/examples/managed_loop
```

`launch.json` 只包含已安装的 command、args、非敏感 env 和 label。凭据不放入 launch、数据库或报告；
每次使用新的项目/数据库路径，不复用日常 RambleDesk 数据库。此入口是验证设施，没有增加 headless 产品。

## 本机接入和实际结果

macOS arm64，基于 `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce` 之上的工作区。
没有安装 bridge，也没有修改用户 Agent 配置、登录或正在使用的实例。

| 接入 | 固定版本 / 实际模型 | 结果 |
| --- | --- | --- |
| 已安装 Claude ACP | `@agentclientprotocol/claude-agent-acp 0.69.0`；它实际使用 bundled Claude Code `2.1.232`；DeepSeek gateway，记录到 `deepseek-v4-pro` | 两实例取得不同 remote session；模型产生 Write/Bash 工具请求；探针在权限请求处停止，未提交反馈 |
| 已缓存 Codex ACP | `@agentclientprotocol/codex-acp 1.8.0`；bundled `codex-cli 0.152.1`；既有 ChatGPT 登录，记录到 `gpt-5.6-sol` | 两实例连接且模型执行内置反馈命令；首轮 IPC 被拒，完整闭环失败；分类修复后的复验见下 |
| PATH 中的独立 Codex CLI | npm 元数据 `0.125.0` | `--version` 即因 native binary 缺失返回 ENOENT；未安装或修复，实际探针使用上面的完整缓存 |

Claude PATH CLI 的版本是 `2.1.207`，不等同于 bridge 实际使用的 bundled 版本。
Claude 现有配置中的 provider env 只在启动进程内继承；临时 `CLAUDE_CONFIG_DIR` 未复制用户 hooks、
权限规则或历史。网关为 `https://api.deepseek.com/anthropic`。OAuth 登录状态本身并不证明这次调用走 OAuth。

Codex 使用独立进程的临时配置根，只继承既有 ChatGPT auth，临时认证文件权限为 `0600`，退出后移除。
没有复制用户配置或对话资料。显式 `INITIAL_AGENT_MODE=read-only`，实际记录为 `on-request/user`、
`workspace-write`、`network_access=false`；bridge 默认的 `agent` 模式有自动审批行为，本次未采用。
Codex 仍发现并只读加载了用户级 `~/.agents/skills/ramble`；实际调用使用 `$RAMBLEDESK_COMMAND feedback`，
没有切换为旧 MCP 协议。没有修改该 skill。

## IPC 失败诊断与修复

初次 Codex 运行的两个 `feedback request` 返回 `revoked_capability`。一次 `feedback recover` 同样失败，
最终探针超时；没有请求持久化成功，因此后续提交、自动 continuation 和第三轮恢复都未通过。
运行期间两个 instance-owned Unix socket 均仍存在，非沙箱进程只做 connect/close 均成功。
父进程没有继承的 `RAMBLEDESK_*` 能力，driver 为两实例分别创建并注入 channel。

[Feedback client](../../crates/rambledesk-feedback-client/src/channel.rs) 原来把全部 socket connect 错误映射为
“已撤销”，掩盖了权限边界。无模型 macOS Seatbelt 子进程对 live socket 的真实连接返回 `EPERM=1`；
相同 owner 在拒绝前后仍可服务，拒绝没有产生 attempted/handoff receipt。

修复只改变诊断：`PermissionDenied` 返回 `ipc_access_denied`，提示先处理本地 IPC 访问权限、沿用同一
request_id；`retryable=false`，明确请求尚未发送。缺失/失联 channel 仍返回 `revoked_capability`，HTTP
401/403 的撤销语义不变。不调整 Agent sandbox、审批策略、channel 权限或私有能力分配。

定向验证：旧实现真实 Seatbelt 测试红灯（实际返回 `revoked_capability`），修复后 feedback-client
10 条测试全部通过。覆盖真实 EPERM、同一 live owner、消息保留 request_id、关闭/替换 channel、HTTP
拒绝与不确定响应；macOS 原生 sandbox 测试按平台编译，Windows 的 PermissionDenied 分类共享实现，
没有据此声明 Windows 真机通过。

```sh
cargo test --locked --offline -p rambledesk-feedback-client
cargo clippy --locked --offline -p rambledesk-feedback-client --all-targets -- -D warnings
```

修复后按完全相同的 sandbox/审批配置，仅复验一次。两个新实例在 ACP initialize 阶段停止，
错误为 `ACP error 1001`；缓存 bridge 源码将该值定义为 `CODEX_PROCESS_EXITED_ERROR_CODE`。
没有取得 remote session、没有 activity，也没有第二次真实模型调用。probe log 未保留更低层诊断，
该次隔离 Codex logs 表为空，因此不能进一步断言退出原因。没有继续重试、重新登录或改变权限。

初次 Codex 的沙箱配置、live socket 与受控 EPERM 复现支持 IPC 权限拒绝这一诊断，但初次客户端丢失了
原始 OS errno；复验又在更早的启动阶段失败。因此新 `ipc_access_denied` 的真实模型侧回执仍未验证。
确定完成的是错误分类修复和真实 kernel 拒绝的无模型回归，完整闭环失败记录继续保留。

## 产物与证据

初次两种接入使用同一 probe binary SHA256：
`4ce413175366b70f171776bb3aaa71a621312c0de1504357cd538d8fec3ec6d5`。
修复后唯一一次复验使用 binary SHA256：
`b8a554fbd8522a1ae1dd53aa8252fd4d607f82b6f41a90f5bc74960cc4453a32`。

| 产物 | SHA256 |
| --- | --- |
| `managed_loop.rs` | `5f7799f971eea5d436ab001dd0d67f6634748d82ba6652089eb35b800e84cf42` |
| `Cargo.lock` | `8228f53afbbc07c2b8c776582aae50155d8dd07e364ec2705c6b30dd00f6a99d` |
| Claude bridge `dist/index.js` | `260aac90bf75f197b93640087c1de66441761d43c2784efa035fdcee60b5dacd` |
| Claude bundled binary | `7b39c1588df919d001dea3ffd5651adb682f2451b5a0e18d42d4233296b53cc7` |
| Codex bridge `dist/index.js` | `6c23657e055271f0f7cdbd655ae0787fa64c98e6d315a25760c1597fd4a88f56` |
| Codex bundled binary | `8194ea3181f330e63023b234b0b231855e5874e0331c5ef7cbc490591497a7bf` |

本次临时证据在系统临时目录下，以目录名区分，每个含 `provenance.json`、`report.json`、隔离数据库和
probe log。临时文件可能被系统清理；验收结论、版本、哈希和失败边界固定在本文件，原始凭据不归档。

- Claude 初次：`rambledesk-q19-claude-_p96hpvi`，23:41:52–23:42:02 UTC+8；`success=false`、`cleanup_complete=true`。
- Codex 初次：`rambledesk-q19-codex-fan48st7`，23:43:51–23:46:58 UTC+8；`success=false`、`cleanup_complete=true`。
- Codex 复验：`rambledesk-q19-codex-ads1t5me`，23:51:11–23:51:16 UTC+8；`success=false`、`cleanup_complete=true`，ACP 初始化停止。

三次 probe PID 均已退出，没有匹配各次 run 路径/已取得 remote session 的残留进程；临时认证文件均已移除。
只核对和清理本次创建的实例，没有停止用户正在使用的 Agent。

## 尚未通过的验收

本次只能确认真实模型参与和明确的失败边界，不能把 Q19、M6 或整个项目质量计划标记为完成。
完整的两项目三轮反馈、真实 Agent continuation/reopen/delete 路线仍须在既有权限合同得到满足时重验。
真实测试没有涵盖 Native UI、手机、其他系统、安装更新、ASR 或新增联网模式。
