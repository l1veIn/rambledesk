# ACP 后端探针

本指南对应仓库中的 [`managed_loop.rs`](../crates/rambledesk-local-server/examples/managed_loop.rs)：使用当前托管反馈命令、私有 IPC 和 application 服务验证双项目闭环。它会启动真实 Agent、调用模型并执行工具，可能产生模型费用；执行前应明确授权。当前实测结果与失败阶段见[质量清单](quality/README.md)，本指南本身不是通过报告。

## 准备与执行

使用已经安装并完成认证的 ACP 入口，不接管用户正在运行的 Agent。将非敏感启动信息保存到独立的 `launch.json`：

```json
{
  "command": "deepseek-acp",
  "args": [],
  "label": "Installed ACP bridge"
}
```

`command` 必填；`args` 默认空数组，`label` 仅是报告标签。可选 `env` 会作为启动配置保存到探针数据库，**不要把凭据放入 JSON**；使用 Agent 已有认证或继承的环境变量。命令是一个可执行名称或路径，参数独立传入，不是 shell 命令。其他 bridge 应使用其真实 ACP 启动参数，不能仅替换标签就宣称兼容。

运行目录必须是新的绝对路径，不能含已有 `database.sqlite`。探针自行创建 `project-a`、`project-b`、数据库、反馈库和 `report.json`；此入口的 JSON 不接受项目 `cwd` 配置。不要指向用户数据或真实项目目录。

macOS / Linux，在仓库根目录执行，替换下面两个绝对路径：

```sh
RAMBLEDESK_MANAGED_PROBE_RUN=1 \
RAMBLEDESK_MANAGED_PROBE_LAUNCH=/absolute/path/launch.json \
RAMBLEDESK_MANAGED_PROBE_RUN_DIR=/absolute/path/new-probe-run \
cargo run -p rambledesk-local-server --example managed_loop
```

Windows PowerShell：

```powershell
$env:RAMBLEDESK_MANAGED_PROBE_RUN = "1"
$env:RAMBLEDESK_MANAGED_PROBE_LAUNCH = "C:/temp/launch.json"
$env:RAMBLEDESK_MANAGED_PROBE_RUN_DIR = "C:/temp/new-probe-run"
cargo run -p rambledesk-local-server --example managed_loop
```

未设置 `RAMBLEDESK_MANAGED_PROBE_RUN=1` 时，只打印说明并退出，不启动 Agent；此时的 exit 0 **不表示验证通过**。探针启动独立 loopback listener，使用随机端口与凭据；示例可执行文件本身分派共享 `feedback` 命令，不需要另装反馈 CLI。

## 实际覆盖的链路

1. 为两个隔离项目创建 ACP 会话，要求均已连接且远端 ID 不同。
2. 分别发送真实模型任务，要求执行内置反馈命令、使用指定 request ID，并结束当前轮次；检查请求归属正确。
3. 通过 `FeedbackApplication` 保存并提交两个不同 marker 的草稿。**此处程序化模拟人类提交，没有操作 Workbench UI。**
4. 等待投递 worker 续接原会话，要求 `delivered`、原 ID 不变，回复含自身 marker 且不含另一个项目的 marker。
5. 分别停止再恢复 Agent，检查仍是原 ID；随后关闭并重新组装 runtime、server 与数据库连接。恢复持久状态不得隐式启动 Agent。
6. 显式恢复两个原会话并发送新的模型任务，要求重新执行反馈读取工具；检查新轮次有工具事件、返回原 marker、无跨会话内容。
7. 删除第一个会话，检查其会话、请求和投递记录消失，另一个仍保持连接与原 ID；最后关闭自有资源。

正常流程每个项目包含请求、反馈续接、重开后读取等模型轮次；这不是固定计费调用上限。等待阶段有 180 秒超时，遇到权限、问答或计划等待即失败，不自动批准；`uncertain` 投递也会失败，不自动重发。

## 怎样判读结果

成功需要进程正常退出，且该次 `report.json` 的 `success`、`cleanup_complete`、`reopened_cleanup_complete` 为 true，并包含两会话 marker、原 ID 恢复及删除隔离结果。stdout 的阶段消息用于定位进度，单独出现 `connected` 或 `continued` 不代表整个探针通过。

报告包含本地/远端 session ID、request ID、投递状态、marker 观察和工具标题等。失败通常保存 `error` 与已完成部分；启动配置解析等早期失败可能没有报告，不能将报告缺失解释为成功。报告中的工具事件只证明观察到调用活动，不保证所有第三方工具行为都被完整审计。

每次记录源码 commit、操作系统、bridge 的实际发行版本与协议自报版本、模型配置、执行时间、退出状态和报告位置；不要把两种版本号当作必然相同。证据去除凭据和用户内容，更新到[质量清单](quality/README.md)，原始输出放本地 `.local-artifacts/`。探针保留自己的运行目录供复核；确认不再需要后再清理。

## 证明边界

通过仅证明指定构建、Agent 版本与环境中的上述链路。它不等于正式安装/升级、Windows 命名管道及真实设备、Desktop/Web 编辑和媒体操作、所有权限选择、强制崩溃/进程树回收或所有模型的兼容验收。正常 runtime 关闭与恢复不冒充操作系统崩溃恢复。

只需检查协议连接时，使用 [ACP crate 的 smoke 示例](../crates/rambledesk-acp/README.md#probes-and-evidence)。未发送 prompt 的 smoke 仍启动 Agent 并创建或恢复会话，但不验证模型回复；Node fixture 的协议与生命周期测试也不替代真实模型闭环。使用行为与投递合同见 [ACP 托管会话](ACP_MANAGED_SESSIONS.md)。

## 历史快照

2026-09-04 的 Windows 测试曾验证社区 `deepseek-acp@0.8.0` 与官方 dsh `0.1.2-rc.1` 的 **MCP 托管路径**、正常重启恢复与删除隔离。原报告保留在[固定 Git 快照](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/ACP_BACKEND_PROBE.md)，不能据此判定当前命令/IPC 路径或其他版本已通过；来源与复用边界见 [CODEG_PORTS.md](CODEG_PORTS.md)。
