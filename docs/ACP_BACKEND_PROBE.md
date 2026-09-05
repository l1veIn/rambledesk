# ACP 后端实机探针

状态：历史实机报告。下列协议探针、真实双项目托管反馈闭环、正常关闭后的恢复与删除隔离，验证的是 2026-09-04 的 MCP 托管实现。

2026-09-05 的生产路径已改为应用内置 `feedback request/get/recover` 命令与 `/agent-feedback/*` HTTP JSON，ACP 不再注入 MCP server 或 Pi 专用扩展。prepared 生命周期、两种视图与真实 usage 已实现，Windows 自动化与隔离浏览器验收已完成，见 [体验重设计计划](ACP_EXPERIENCE_REDESIGN_PLAN.md)。本报告不构成新 command 路径的真实模型、Linux/macOS、安装包或性能验收。

验证日期：2026-09-04。环境：Windows、Node.js 24.18.1。

## 结论

当时的 `deepseek-acp` 社区 bridge 和官方 `dsh --profile acp` 均通过真实模型、MCP 工具调用与进程重启后恢复上下文验证。社区 `deepseek-acp@0.8.0` 与官方 dsh 的结果分别记录，不能外推为其他版本或新的反馈入口已通过。

**不能依据 Codeg 的旧注释断言官方 dsh 不支持 MCP 或会话恢复。** 本次记录的官方 `0.1.2-rc.1` 支持 MCP、list/resume/close 和标准工具生命周期更新，不提供 `session/load` 与历史更新重放。这是指定版本的实机结果；当前 RambleDesk 的统一反馈命令不再以 MCP capability 作为可用条件。

参考代码固定为 Codeg `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。官方文档固定为 DeepSeek Harness `76fda729799fe9b3848dbe2c211d4b231032b81e`，其 [ACP 合同](https://github.com/deepseek-ai/deepseek-harness/blob/76fda729799fe9b3848dbe2c211d4b231032b81e/packages/acp/acp/README.md) 区分已提交语义更新、resume 与历史重放。Codeg 的 [DeepSeek 注册项](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/registry.rs#L1265) 可用来理解社区 bridge 的产品取舍，但其官方后端对比不是当前能力真值。

## 入口与版本

| 入口 | 本机初始状态 | 本次执行版本 | 启动参数 |
|---|---|---|---|
| `deepseek-acp` | 已安装 0.7.0 | 0.7.0、隔离安装的 0.8.0 | 无参数，stdio |
| 官方 `dsh` | PATH 中未安装 | 隔离安装 `@deepseek-ai/dsh@0.1.2-rc.1` | `--profile acp` |
| `pi-acp` | 已安装 0.0.33 | 未运行协议探针 | 待单独验证 |
| `pi` | 已安装 `@earendil-works/pi-coding-agent@0.83.0` | 未运行协议探针 | 待单独验证 |

官方包所带 `@deepseek-ai/dsh-acp` 的包版本为 `0.1.2-rc.1`，但握手的 `agentInfo` 为 `deepseek-harness-acp / 0.0.1`。因此必须分别记录发行版本与协议自报版本，不能假定相等。

社区 0.8.0 和官方 dsh 均安装在系统临时目录，使用 `npm install --prefix ... --ignore-scripts --no-audit --no-fund`，没有升级全局安装。现有 DeepSeek 凭据只读入探针内存并传入子进程环境，未输出或写入报告。每次执行有独立工作目录、`DSH_HOME` 和会话记录目录；没有接入用户已在运行的 Agent 实例。

## 实际执行结果

| 行为 | 社区 0.7.0 / HTTP MCP | 社区 0.8.0 / stdio MCP | 官方 0.1.2-rc.1 / HTTP MCP |
|---|---|---|---|
| `initialize` | v1 成功 | v1 成功 | v1 成功 |
| `session/new` | 成功 | 成功 | 成功 |
| MCP 初始化与工具发现 | 服务端收到请求 | 子进程收到请求 | 服务端收到请求 |
| 模型调用 `echo_marker` | 收到真实 `tools/call` | 收到真实 `tools/call` | 收到真实 `tools/call` |
| HTTP header 传递 | 每次请求均正确 | 不适用 | 每次请求均正确 |
| 首轮完成 | `end_turn` | `end_turn` | `end_turn` |
| `session/close` | 成功 | 成功 | 成功 |
| 关闭 stdin 后退出 | exit 0，无强制终止 | exit 0，无强制终止 | exit 0，无强制终止 |
| 新进程恢复原会话 | `session/load` 成功 | `session/load` 成功 | `session/resume` 成功 |
| 恢复后的记忆验证 | 精确返回 `RD_MEMORY_91` | 精确返回 `RD_MEMORY_91` | 精确返回 `RD_MEMORY_91` |
| 恢复后再次关闭 | close、EOF、exit 0 | close、EOF、exit 0 | close、EOF、exit 0 |

首轮提示要求只调用无副作用的 MCP `echo_marker`，返回固定字符串 `RAMBLEDESK_ACP_MCP_6F82`，并记住另一个代码。恢复后只询问此前的记忆代码，提示中不再次提供答案。三条路径均实际调用了工具并恢复了记忆。0.7.0 首轮还附加了记忆确认文字；这里验收的是工具可达性和上下文恢复，不要求模型首轮严格遵守输出格式。

官方 `session/load` 另行执行过一次，明确返回 `-32601` / `Method not found`，随后通过 `session/resume` 恢复，未创建替代会话。社区 0.8.0 的 load 重放了此前的消息和工具事件，包括相同的 `toolCallId`；官方 resume 没有重放历史。

## 能力与配置差异

| 项目 | 社区 0.7.0 / 0.8.0 | 官方 0.1.2-rc.1 |
|---|---|---|
| `loadSession` | true | 未声明 |
| `sessionCapabilities` | close、fork、list、resume | close、list、resume |
| `mcpCapabilities.http` | true | true |
| 图片 / embeddedContext | true / true | 本次部署 false / false |
| 模型选项 | 扁平 select，本次值为 `deepseek-v4-flash` | 带分组，本次值为序列化的 provider/model 二元组 |
| 推理选项 id | `reasoning` | `reasoning_effort` |
| 其他选项 | sandbox select、modes | 本次没有这些选项 |
| 消息更新 | 首轮与后续回复分多个 chunk | 每段已提交消息一次更新 |

这些是指定版本与指定部署的握手结果。图片、fork、list、配置修改本次没有独立执行验收，不能把声明等同于完整验证。模型选项值必须当作 opaque value 原样回传，不要拆解、重写或根据标签构造。

## 资源回收证据

| 运行 | 本次创建的 bridge PID | 结果 |
|---|---|---|
| 社区 0.7.0 初次 / 恢复 | 4524 / 12476 | 均在 stdin EOF 后 exit 0，随后查不到 PID |
| 社区 0.8.0 初次 / 恢复 | 30216 / 33616 | 均在 stdin EOF 后 exit 0，随后查不到 PID |
| 官方 dsh 初次 / 恢复 | 29224 / 11892 | 均在 stdin EOF 后 exit 0，未强制终止 |

社区 0.8.0 两次创建的 stdio MCP 子进程为 10372 / 16308。探针日志分别记录 `start → initialize → tools/list → stdin_end`；首次另有 `tools/call`。检查时两个 PID 均已不存在。此证据覆盖正常 close/EOF 路径；强制关闭、崩溃、孙进程与 PID 重用仍属于 supervisor 的专门验收范围。

## 仓库 Rust 客户端复核

在 `rambledesk-acp` smoke example 接入官方 Rust SDK 2.0.0 后，用隔离安装的社区 0.8.0 再执行一组真实测试：

1. Rust `initialize → session/new → session/prompt`：返回 `READY`、`EndTurn`，进程 exit 0。
2. 新 Rust 进程使用同一个 Agent Session ID `c720b72a-3c7f-4f6a-9016-26a83a3cc206`，按协商能力选择 `session/resume`。
3. 提示中不提供原答案，模型精确返回 `RD_RUST_MEMORY7`、`EndTurn`，进程 exit 0。
4. 退出后没有命令行指向本次探针目录的 Node 进程残留。

复核使用不含凭据的 `rust-smoke-0.8.0-close/launch.json`；凭据由外部 runner 继承到子进程环境。`rust-smoke.mjs`、两次 stdout/stderr 记录保留在临时探针目录。smoke 现会展示实际助手文本，并在 Agent 声明 close 能力时先完成 `session/close`，再关闭传输。

**持久化边界尚不能仅凭以上成功推断。** 补充检查社区 bridge 的多帧 Zstd 会话日志时，发现部分运行缺少末尾 turn/end；独立 Node 探针的显式 close 路径也观察到该现象。因此本次证明的是已收到的协议回复、相同 ID 恢复和记忆保持，不声称后端所有历史事件均已完整落盘。后续恢复验收需要单独覆盖最后一条助手回复和工具结果，客户端活动记录也应自行持久化。简单的一次 `zstdDecompressSync` 不能读取全部帧；临时 `read-session-log.cjs` 使用该包的帧扫描器逐帧检查。

## 本机复现

独立探针和原始脱敏报告保存在 `$env:TEMP\rambledesk-acp-probe-20260904`。这些临时文件不是项目运行依赖，也不替代仓库中的 Rust smoke example 和自动测试。

```powershell
$probeRoot = Join-Path $env:TEMP 'rambledesk-acp-probe-20260904'

# 独立安装，保留用户当前全局版本。
npm install --prefix "$probeRoot\bridge-0.8.0" --no-audit --no-fund --ignore-scripts deepseek-acp@0.8.0
npm install --prefix "$probeRoot\official-dsh" --no-audit --no-fund --ignore-scripts @deepseek-ai/dsh@0.1.2-rc.1

# 本机已有 0.7.0；探针从该包的依赖读取 YAML 解析器。
node "$probeRoot\probe.mjs"
node "$probeRoot\probe.mjs" "$probeRoot\bridge-0.8.0\node_modules\deepseek-acp" isolated-0.8.0-stdio stdio
node "$probeRoot\probe.mjs" "$probeRoot\official-dsh\node_modules\@deepseek-ai\dsh" official-0.1.2-rc.1-http http official
```

每次复现会创建新的测试会话并执行两个很短的模型回合。探针需要现有 `DEEPSEEK_API_KEY`，或从当前用户 `.dsh/.credentials.yaml` 中读取该引用；脚本本身不含凭据。

| 文件 | 用途 |
|---|---|
| `probe.mjs` | JSON-RPC stdio 客户端、临时 HTTP MCP 服务、恢复与退出检查 |
| `mcp-stdio.mjs` | 只提供 `echo_marker` 的 MCP 子进程，记录启动/请求/EOF |
| `installed-0.7.0/report.json` | 全局 0.7.0 的实际协议报告 |
| `isolated-0.8.0-stdio/report.json` | 隔离 0.8.0 的协议报告和 MCP 子进程存活检查 |
| `isolated-0.8.0-stdio/mcp-stdio.jsonl` | MCP 子进程请求与退出日志 |
| `official-0.1.2-rc.1-http/report.json` | 官方 dsh 的协议报告，含 load 拒绝与 resume 成功 |

最终探针 SHA-256：`probe.mjs` 为 `08e55f3d270b537bace9f62f1e4606e6e215d98f24408bdc08e75b855ca004c8`；`mcp-stdio.mjs` 为 `87d38442aa44eec6f5e21bc3348b93261f85ac7733a6220f15b538f970002135`。第一轮 0.7.0 运行时误把关闭 LSP 发现的值设成 `[]`，后端警告并回退内建发现；后续已修正为 `{}`，后两轮没有该警告。

## 真实托管反馈闭环

当时版本的 `crates/rambledesk-local-server/examples/managed_loop.rs` 组合真实 `SessionApplication`、`AcpSessionDriver`、`LocalManagedFeedbackProvider`、SQLite、恢复检查点和 outbox worker。2026-09-04 在社区 0.8.0 与官方 0.1.2-rc.1 上分别完成初次闭环，并在完整恢复/删除接线后各复核一次，均通过：

1. 两个独立临时项目同时创建各自的 Agent 会话，并各调用一次 scoped `request_feedback` 后结束 turn。
2. 探针通过工作台 application 入口保存并提交两个不同标记的反馈。这是模拟用户提交，用于验证应用与协议链路，不代替手工 UI 验收。
3. worker 自动续接原会话，两个 Agent 都调用 `get_feedback`，执行读取工具并返回各自反馈中的标记。初始提示不包含答案，两边均未输出另一会话的标记。
4. 两条持久化投递记录均为 `delivered`，随后 stop/start 仍恢复原来的两个 Agent Session ID。
5. 关闭整个 application、HTTP server 和 SQLite store，再从原数据库建立新的运行时与 scoped MCP provider。读取列表/会话不会隐式启动 Agent。
6. 显式并发恢复原来两个 Agent Session ID，通过新 scoped MCP 连接再次调用 `get_feedback`、读取原反馈并返回正确标记。新提示只提供 request ID，不提供答案；验收只检查这次新 turn 的工具和回复，避免把本地历史误当成恢复成功。
7. 直接删除会话 A，确认其 session、request 和 outbox 记录均已移除；会话 B 保持 Connected 与原 Agent Session ID。
8. 两次最终 harness 均正常 exit 0，未强制终止。按父子关系定期记录了社区后端 9 个、官方后端 13 个后代进程，退出后按 PID 与创建时间复核均无存活；初次闭环两次各记录 5 个后代，也均已退出。

社区后端的活动记录明确包含各自 `feedback.md` 的完整读取路径及 `Completed`；官方后端的协议标题仅显示 `read · Completed`，因此此处不将官方工具标题当作完整路径审计。官方回复同样精确包含对应反馈标记。

原始报告位于临时探针目录：

- `managed-community-0.8.0-2026-09-04T09-57-52-662Z/report.json`
- `managed-official-0.1.2-rc.1-2026-09-04T09-58-12-484Z/report.json`
- `managed-community-0.8.0-2026-09-04T10-20-00-677Z/report.json`：最终闭环、整运行时重启与删除。
- `managed-official-0.1.2-rc.1-2026-09-04T10-20-00-677Z/report.json`：同上。
- 每个目录的 `processes.json` 记录本次拥有的子进程、创建时间和退出复核；`launch.json` 不含凭据。

以下是历史执行参数，不代表当前 example 已重新通过真实模型验收。当前源码已接入统一反馈命令；重跑时还须按 [example 说明](../crates/rambledesk-local-server/examples/managed_loop.rs) 指定可执行命令并显式授权模型调用。历史探针使用：

```powershell
$env:RAMBLEDESK_MANAGED_PROBE_RUN = '1'
$env:RAMBLEDESK_MANAGED_PROBE_LAUNCH = 'C:\path\to\launch.json'
$env:RAMBLEDESK_MANAGED_PROBE_RUN_DIR = 'C:\path\to\new-probe-directory'
cargo run -p rambledesk-local-server --example managed_loop
```

启动 JSON 接受 `command`、`args`、`env`、`label`。凭据应放在继承的进程环境中，不能写进启动 JSON；JSON 中的环境变量会作为 Agent 配置持久化。示例要求新数据库路径，并为两个项目建立独立目录。一次执行包含两项目各三轮短模型调用：反馈请求、自动续接、整个运行时重开后的读取。每个等待阶段最多 180 秒；遇到权限请求时停止探针，不自动批准额外操作；成功或失败均执行 session/server/store 清理。

本机辅助 runner 是临时目录中的 `managed-loop.mjs`，沿用已有凭据的安全读取方式：`node managed-loop.mjs` 验证社区 bridge，`node managed-loop.mjs official` 验证官方 dsh。它只把凭据传入子进程环境，对输出脱敏，并复核本次后代进程退出。

## 历史发现与仍适用的约束

1. 恢复策略按能力区分 load 与 resume；失败时显示真实原因，不静默 new 一个空上下文。
2. 恢复过程区分历史重放与新回合输出，避免重复插入活动记录或把旧工具当作重新执行。
3. 动态模型/模式控件现已实现，继续原样使用后端提供的分组选项 id/value，并区分持久配置与运行会话实际使用值。
4. 启动程序版本和 `agentInfo` 分开记录；预设只提供默认值，实际能力来自协商和验证。
5. 管理器只回收自身创建的实例资源；正常 EOF 验收不能代替崩溃与进程树清理测试。

未在上述真实后端探针中执行：权限请求的用户交互、运行中取消、整个应用异常终止后的恢复，以及 Pi/Codex 的兼容验收。真实重启验证使用正常关闭；异常中断检查点、EOF 清理、延迟取消回调与新实例隔离由 `session_recovery_runtime` 的四项运行时测试覆盖。恢复原上下文与重新读取反馈成功仍不代表后端自身历史日志已全量 flush。
