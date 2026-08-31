# ACP Agent Client 实机验收

> 日期：2026-08-31
>
> 环境：macOS 26.3.1 / arm64 / Node.js 22.23.0
>
> Codeg 基线：[`769610c626f1fc4b18c11d3e289326acf097b99f`](https://github.com/xintaofei/codeg/tree/769610c626f1fc4b18c11d3e289326acf097b99f)
>
> 验收入口：Desktop 中被默认忽略的 `live_agent_install_connect_and_optional_ramble` 单 Agent 串行测试。

## 判定口径

本报告刻意分开三层结果：

1. **安装**：RambleDesk 能发现准确版本，或把固定版本安装进自己的 `v3/acp-clients` 目录。
2. **ACP 会话**：进程完成 `initialize` 与临时 `session/new`，并返回真实的模型、思考强度和访问模式。
3. **首个结构化 Ramble**：RambleDesk Launch 后，Agent 真的调用 Session Toolset 的 `request_feedback`，并产生持久化 Feedback Request。
4. **完整 Ramble Loop**：人类提交反馈后，Agent 真的依次调用 `get_feedback`、应用反馈正文，并创建下一条 `request_feedback`；下一条 Request 必须包含只存在于反馈正文中的验收 marker。

认证失败不是安装失败；ACP 会话成功、首个 Feedback Request 成功也都不等于完整 Ramble Loop 成功。

## 结果矩阵

| Agent | 安装 / 启动物 | ACP 会话 | 首个结构化 Feedback Request | 实机结论 |
|---|---|---|---|---|
| Claude Code `0.69.0` | 通过 | 通过 | **通过** | 返回 model、reasoning 与三档访问模式；产生 “What would you like to work on?” Feedback Request。 |
| Codex `1.7.0` | 受管安装通过 | 通过 | 超时 | 两组权限/思考强度组合均能 Launch，但 90 秒内没有调用 `request_feedback`。 |
| Gemini CLI `0.57.0` | 受管安装通过 | 到达账号边界 | 未执行 | Agent 返回当前账号无有效许可；已准确归类为认证/许可问题。 |
| OpenClaw `2026.7.1` | 受管安装通过 | 失败 | 不支持 | ACP 进程关闭输出；还需要已配置并运行的 Gateway。它拒绝 Session MCP，本版本明确仅提供连接诊断。 |
| OpenCode | 系统 `1.18.11` 启动通过 | 通过 | **通过** | Codeg 同样允许 PATH 版本优先；产生持久化 Feedback Request。固定二进制 `1.18.25` 的受管下载路径已实现但本轮未取代系统版本。 |
| Cline `3.0.60` | 受管安装通过 | 到达账号边界 | 未执行 | 修正其没有 `.bin/cline` 的安装特例后可启动；要求先 authenticate。 |
| Hermes `0.20.6` | 受管安装通过 | 通过 | 安全拒绝 | 固定版握手没有声明 HTTP MCP。虽然 Codeg 静态元数据写 `supports_mcp=true`，RambleDesk 不会把“能连接”冒充“能接收 Session Toolset”。 |
| CodeBuddy `2.141.0` | 受管安装通过 | 到达账号边界 | 未执行 | 要求登录。其 ACP 未暴露可变权限 selector，RambleDesk 只提供会触发授权请求的默认可写模式。 |
| Kimi Code `0.39.1` | 受管安装通过 | 通过后认证状态不稳定 | 超时 | 一次预检成功，但 Launch 未产生 Feedback；后续重试要求重新认证。 |
| Pi `0.0.33` | 受管安装通过 | 通过 | 不支持 | `pi-acp` 接收但不会把 `mcpServers` 转发给 Pi，因此只提供 ACP 连接，不开放结构化 Ramble。 |
| Grok `1.0.5` | 受管安装通过 | **通过** | 超时 | 返回 Grok 4.5/4.6；进程级 read-only/workspace-write/YOLO 映射已对齐 Codeg，但 Launch 后未调用 `request_feedback`。 |
| Cursor `2026.08.11-e8db854` | 受管整树安装通过 | 到达账号边界 | 未执行 | 完整保留 bundled Node/runtime 目录；要求 Cursor 登录。 |
| DeepSeek ACP `0.7.0` | 启动通过 | 通过 | **通过** | 返回 model、reasoning 与 sandbox 三档访问模式；无 `/ramble` 或文件型 skill 依赖，并完成已验证反馈正文的完整双循环。 |
| Qoder `1.1.33` | 受管安装通过 | 到达账号边界 | 未执行 | 要求 Qoder 登录或有效 token。 |
| Antigravity `1.0.0` | **受管整树安装通过** | **通过** | 超时 | 879 MB 主程序与 helper 均可执行；缺失时无损写入 `oauth-personal`，严格 ACP capability 形状修正后返回 11 个模型和两档访问模式。Launch 后未调用 `request_feedback`。 |

## 本轮关闭的问题

- 15 个内置 Agent 共用一份 release-owned Catalog；版本、包、Node 下限、平台 URL 与目录入口对齐 Codeg 固定提交。
- npm 安装固定使用官方 registry、包含 optional platform package，并验证 PATH 中 npm 命令的实际 package 版本。
- Cline、Cursor 与 Antigravity 的非标准目录树得到专门校验；Antigravity 的 helper 也会获得执行权限。
- Grok 的权限参数在 ACP 子命令前注入；CodeBuddy 的不可变默认权限不会再让 Launch 按钮永久禁用。
- Antigravity 缺失 `auth.type` 时采用 Codeg 的个人 OAuth 安全默认；已有认证配置保留不动，非严格 JSON 或异形 `auth` 块拒绝覆盖。
- `initialize.clientCapabilities.session.configOptions.boolean` 使用 ACP 要求的对象形状，而不是宽松实现曾接受的布尔值。
- 安装准备有三分钟上限，取消时会终止 npm 子进程；错误分为运行时、安装、认证、协议、平台与超时。
- Managed ACP 已提交/取消的 Feedback Request 会继续出现在中栏，并可从持久化 Feedback Package 恢复正文；这不是 Agent transcript 副本。
- Launch、Steering 与 Feedback Resume 统一注入 Ramble Loop Contract；普通 Agent 输出、Permission Request 与 Ask Question 都不能替代末尾的 `request_feedback`。Runtime 使用 Session Toolset 的可信 observation 与稳定 `work_id → request_id` 完成证据；缺失交接时只补发一次协议修复 Prompt，连续拒绝保持 pending，不自动忙循环。
- DeepSeek 完整双循环实测通过：首轮创建任务简报 Request；人类提交包含 `RAMBLE_LOOP_CONTENT_42` 的反馈；Agent 经 `get_feedback` 读取正文、只读检查 README，并在下一条 Feedback Request 中原样带回 marker 和两句摘要。验收同时覆盖了“人类在 Agent work 落证据前立即提交”的并发竞态。
- `get_feedback` 同时保留结构化 Delivery Envelope 和人类可读文本投影；正式反馈正文缺失时回退 uncooked 正文，文本层不复制附件 Base64 或泄漏本地路径。

## 尚未伪装成完成的边界

- 当前 Session Toolset 只提供 ACP HTTP MCP。只支持 stdio 转发或根本不转发 MCP 的 Agent，不开放结构化 Ramble。
- Codex、Kimi、Grok 与 Antigravity 的超时结论来自 Ramble Loop Contract 与运行时完成门落地前的 Launch 验收；仍需按新的完整双循环口径逐一重跑，当前不能写成全链路通过。
- Gemini、Cline、CodeBuddy、Cursor、Qoder 的下一层验收需要对应账号完成登录；这不是代码安装故障。
- OpenClaw 需要用户自己的 Gateway 配置，且当前不能承载 RambleDesk Session Toolset。
- 二进制上游 URL 没有提供可固定的 SHA-256；RambleDesk 支持有 checksum 时强制校验，但这一组 Codeg 内置 URL 仍继承上游供应链边界。
