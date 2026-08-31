# Codeg ACP Agent Client 调研

> 调研日期：2026-08-31
>
> Codeg 源码基线：[`769610c626f1fc4b18c11d3e289326acf097b99f`](https://github.com/xintaofei/codeg/tree/769610c626f1fc4b18c11d3e289326acf097b99f)
>
> 范围：Codeg 内置 ACP Agent 的身份、安装、启动、平台、认证与会话能力。
>
> 注意：本文区分“Codeg 当前实现”“Agent 官方承诺”和“仍需 RambleDesk 实机验证”。

## 结论

Codeg 当前源码内置 15 个 Agent。RambleDesk 可以复用这份目录，但不应该把它们实现为 15 套静态表单：

1. 安装包、启动命令、最低运行时和认证方式属于 Agent Catalog 的静态元数据。
2. Model、Reasoning、访问模式和恢复能力应以每次 ACP `initialize`、`session/new` 返回的 capability、mode 和 `configOptions` 为准。
3. 恢复顺序建议统一为：

   `session/resume`（若声明）→ `session/load`（若声明）→ 明确提示无法原生恢复。

   不应把“新建会话并补提示词”伪装成原会话恢复。
4. Codeg 的版本固定值比文档更新，初版应先镜像 Codeg 已验证的 pin，不应直接追 ACP Registry 的最新版。
5. OpenClaw、Pi、DeepSeek Harness 和 Hermes 存在重要的传输或供应链特例，不能仅靠通用 npm 配置覆盖。

Codeg 官方说明其内置 15 个 Agent，并通过统一 ACP 工作区管理它们；其源码中的实际目录和 pin 位于 [`registry.rs`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/acp/registry.rs)，稳定存储 ID 与标签位于 [`agent.rs`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src-tauri/src/models/agent.rs)。[Codeg Supported Agents](https://docs.codeg.app/guide/supported-agents)

## Agent Catalog

| RambleDesk 建议 ID | Codeg 标签 / Registry ID | Codeg 固定版本与安装物 | ACP 启动命令 | 平台与前置条件 |
|---|---|---|---|---|
| `claude_code` | Claude Code / `claude-acp` | `@agentclientprotocol/claude-agent-acp@0.69.0` | `claude-agent-acp` | Node ≥22 |
| `codex` | Codex CLI / `codex-acp` | `@agentclientprotocol/codex-acp@1.7.0` | `codex-acp` | Node ≥20 |
| `gemini` | Gemini CLI / `gemini` | `@google/gemini-cli@0.57.0` | `gemini --acp --skip-trust` | Node ≥20 |
| `open_claw` | OpenClaw / `openclaw-acp` | `openclaw@2026.7.1` | `openclaw acp` | Node ≥22.22.3 |
| `open_code` | OpenCode / `opencode` | 官方二进制 `1.18.25` | `opencode acp` | macOS/Linux/Windows，x64/arm64 |
| `cline` | Cline / `cline` | `cline@3.0.60` | `cline --acp` | Codeg 要求 Node ≥22；Cline 官方最低要求可能更低 |
| `hermes` | Hermes Agent / `hermes` | 社区 npm 包装器 `hermes-agent@0.20.6` | `hermes acp` | Node ≥20；包装器内部安装官方 Hermes Python 3.11 环境 |
| `code_buddy` | CodeBuddy / `codebuddy-code` | `@tencent-ai/codebuddy-code@2.141.0` | `codebuddy --acp` | Node ≥22 |
| `kimi_code` | Kimi Code / `kimi-code` | `@moonshot-ai/kimi-code@0.39.1` | `kimi acp` | Node ≥22.19 |
| `pi` | Pi / `pi-acp` | `pi-acp@0.0.33` | `PI_ACP_ENABLE_EMBEDDED_CONTEXT=true pi-acp` | Node ≥22；另需 `@earendil-works/pi-coding-agent` 的 `pi` 在 PATH |
| `grok` | Grok / `grok-build` | `@xai-official/grok@1.0.5` | `grok --no-auto-update [--permission-mode …] agent stdio` | Node ≥20；npm 安装必须包含 optional platform package |
| `cursor` | Cursor / `cursor` | 官方整树二进制 `2026.08.11-e8db854` | `cursor-agent acp` | macOS/Linux/Windows，x64/arm64；必须保留完整目录树 |
| `deepseek` | DeepSeek Harness / `deepseek-acp` | 社区桥接器 `deepseek-acp@0.7.0` | `deepseek-acp` | Node ≥22 |
| `qoder` | Qoder / `qoder-cli` | `@qoder-ai/qodercli@1.1.33` | `qoder --acp` | macOS/Linux/Windows；官方暂不支持 Windows arm64 |
| `antigravity` | Google Antigravity / `antigravity-acp` | Google 二进制树 `1.0.0` | `agy_acp_server.par`；Linux 追加 `--uid=` | macOS arm64、Linux x64/arm64、Windows x64/arm64；无 Intel Mac |

通用发现策略应优先检查运行命令是否已存在于 PATH，再检查受管安装；Claude 和 Codex 需要查找的是 ACP adapter 命令，而不是用户可能已经安装的 `claude` 或 `codex`。两套 adapter 分别复用 `~/.claude` 和 `~/.codex` 的现有认证状态。[Codeg Working with Agents](https://docs.codeg.app/guide/agents), [Claude ACP adapter](https://github.com/agentclientprotocol/claude-agent-acp), [Codex ACP adapter](https://github.com/agentclientprotocol/codex-acp)

## 认证与 ACP 能力

表中：

- “动态”表示应读取 ACP 握手或 `configOptions`，不能硬编码。
- “Codeg 验证”表示 Codeg 源码或其注释记录了实测，但不一定是 Agent 官方兼容承诺。
- “待验”表示公开官方资料不足，必须用固定版本做集成测试。

| Agent | 认证前置 | Model | Reasoning | 访问/授权 | MCP | 原生恢复 |
|---|---|---:|---:|---|---:|---|
| Claude Code | 复用 Claude 登录，或 Anthropic/API endpoint | 动态 | 动态 | `default`、`acceptEdits`、`plan`、`auto`、`bypassPermissions`；Permission Request | 是 | `load` + `resume` |
| Codex | ChatGPT 登录，或 `CODEX_API_KEY` / `OPENAI_API_KEY` / gateway | 动态 | 动态 | approval 与 sandbox selector；Permission/Elicitation | 是 | `load` + `resume` |
| Gemini CLI | Google 登录、Gemini API key 或 Vertex 配置 | 动态 | 独立 effort 待验 | `default`、`auto_edit`、`yolo`、`plan` | 是 | CLI/ACP 声明可恢复；固定版本需实测 |
| OpenClaw | 已运行并配置 Gateway；远程连接另需 token/password | Gateway 决定 | 待验 | 支持 permission relay | **否**：Codeg 必须向 `session/new` 发送空 `mcpServers` | 有 session mapping；标准 `load/resume` 待验 |
| OpenCode | `opencode auth login` 或 provider 配置 | 动态 | 动态 effort | Agent permissions 与 mode | 是 | `load` + `resume` |
| Cline | `cline auth`、客户端登录或 `CLINE_API_KEY` | 动态 | CLI 支持 thinking，ACP 固定版需读 handshake | Plan/Act、逐项授权、Auto-approve | 是 | `load`/resume |
| Hermes | `hermes acp --setup` 配置 provider/model | 动态 | 待验 | Permission Request | Codeg 元数据为是；RambleDesk 实测 `0.20.6` 未声明 HTTP MCP | Codeg 验证 list/resume/fork |
| CodeBuddy | `codebuddy`/官方登录或产品支持的 key 配置 | 动态/待验 | 待验 | 官方 ACP 支持权限请求 | 是 | 待固定版本实测 |
| Kimi Code | `kimi login` 或 Kimi provider 配置 | 动态 | 动态 | manual/yolo/auto/plan | 是 | `load` + `resume` |
| Pi | 在 Pi 中单独配置 provider；可用 `pi-acp --terminal-login` | 动态 | 通过 mode 映射 thinking | **无内建 sandbox/permission 系统** | **否**：adapter 接收但不会转交 MCP | `session/load`，依赖 session-map |
| Grok | `grok login` 或 `XAI_API_KEY` | 动态，但 1.0.5 使用 xAI 私有 `_meta` | 动态，使用 xAI 私有 `_meta` | launch-level permission mode + Permission Request | 是 | `resume`，Codeg 1.0.5 实测 |
| Cursor | `cursor-agent login`、`CURSOR_API_KEY` 或 auth token | 动态 | 独立 effort 未确认 | Agent/Plan/Ask + Permission Request | 是 | `load` |
| DeepSeek Harness | `deepseek-acp --setup` 或 `DEEPSEEK_API_KEY` | 动态 | 动态 | sandbox/file-permission selector | 是 | `load` + `resume` + `fork` |
| Qoder | `qoder login` 或 `QODER_PERSONAL_ACCESS_TOKEN` | 动态 | 动态 | 官方至少承诺 Default/Bypass；Codeg 握手还见到 plan/acceptEdits | 是 | Codeg 验证 `load` + `resume` + `fork` |
| Antigravity | `$GEMINI_HOME/antigravity-acp/settings.json` 必须有 `auth.type`，随后 browser OAuth | 动态 | 未确认独立 effort | `default`、`auto_edit`、`yolo` | 是 | Codeg 验证 `load` + `resume` |

主要官方依据：

- Claude adapter 明确支持 tool permission、客户端 MCP、图片和终端。[Claude ACP adapter](https://github.com/agentclientprotocol/claude-agent-acp)
- Codex adapter明确支持模型、reasoning effort、approval、sandbox、MCP、Permission Request 和 `session/load` 恢复。[Codex ACP adapter](https://github.com/agentclientprotocol/codex-acp)
- Gemini CLI 官方参数包括 `--acp`、`--model`、approval mode 和 session resume。[Gemini CLI configuration](https://github.com/google-gemini/gemini-cli/blob/main/docs/reference/configuration.md)
- OpenClaw ACP 是 Gateway 桥接层并维护 session mapping。[OpenClaw ACP](https://docs.openclaw.ai/cli/acp)
- OpenCode 官方命令为 `opencode acp`，并承诺 ACP 下保留工具、MCP、Agent 和权限系统。[OpenCode ACP](https://dev.opencode.ai/docs/acp/)
- Cline 官方明确承诺 model/provider selector、Plan/Act、权限提示、auto-approve 和 session resume。[Cline ACP](https://docs.cline.bot/usage/acp)
- Hermes 官方 ACP 支持 session creation、permission、fork、cancel 和 auth；Codeg 的 npm 安装渠道则是经审计的社区包装器，不是 Nous Research 官方 npm 分发。[Hermes Programmatic Integration](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/developer-guide/programmatic-integration.md)
- CodeBuddy 官方 ACP 文档确认 `codebuddy --acp`、工具委托和 Permission Request。[CodeBuddy IDE integration](https://www.codebuddy.ai/docs/cli/ide-integrations), [CodeBuddy ACP](https://www.codebuddy.ai/docs/cli/acp)
- Kimi 官方 CLI 支持 `kimi acp`、model、session resume 与 permission mode。[Kimi command reference](https://github.com/MoonshotAI/kimi-code/blob/main/docs/en/reference/kimi-command.md)
- `pi-acp` 明确说明它是 MVP 社区 adapter，支持 session mapping、model/thinking selector，但不支持 ACP filesystem/terminal delegation，也不把 MCP 转发给 Pi。[pi-acp](https://github.com/svkozak/pi-acp)
- Grok 官方命令为 `grok agent stdio`，支持本地认证或 `XAI_API_KEY`；Codeg 对 model/reasoning 还需兼容 xAI 私有 `_meta`。[Grok Headless & ACP](https://docs.x.ai/build/cli/headless-scripting)
- Cursor 官方支持 `cursor-agent acp`、session load 和 Permission Request。[Cursor ACP](https://cursor.com/docs/cli/acp)
- Codeg 使用的是社区 [`deepseek-acp`](https://github.com/xintaofei/deepseek-acp)，而不是 DeepSeek 官方自动化型 ACP transport。官方 DSH transport 不提供完整编辑器体验、历史恢复或 MCP，因此二者不可互换。[DeepSeek Harness official ACP README](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/acp/acp/README.md)
- Qoder 官方确认 `qoder --acp`、Default/Bypass 权限模式、MCP、图片和客户端文件/终端能力；CLI 另外支持 model、reasoning 与 session resume。[Qoder ACP](https://docs.qoder.com/cli/acp), [Qoder CLI reference](https://docs.qoder.com/cli/cli-reference)
- Antigravity 的发行物、平台与官方图标来自公共 ACP Registry；认证和详细 selector 结论目前主要来自 Codeg 对固定二进制的握手验证。[ACP Registry](https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json)

## Logo 对照

Codeg 没有统一远程加载所有图标，而是在 [`agent-icon.tsx`](https://github.com/xintaofei/codeg/blob/769610c626f1fc4b18c11d3e289326acf097b99f/src/components/agent-icon.tsx) 内维护经过 UI 适配的 SVG：

- 彩色：Claude Code、Codex、Gemini CLI、OpenClaw、Kimi Code、Pi、DeepSeek Harness。
- 单色 `currentColor`：OpenCode、Cline、Hermes、CodeBuddy、Grok、Cursor、Qoder、Google Antigravity。

可直接使用 ACP Registry 官方图标的明确来源：

- [Cursor](https://cdn.agentclientprotocol.com/registry/v1/latest/cursor.svg)
- [Google Antigravity](https://cdn.agentclientprotocol.com/registry/v1/latest/antigravity-acp.svg)
- [Qoder](https://cdn.agentclientprotocol.com/registry/v1/latest/qoder.svg)
- [Claude](https://cdn.agentclientprotocol.com/registry/v1/latest/claude-acp.svg)
- [Codex](https://cdn.agentclientprotocol.com/registry/v1/latest/codex-acp.svg)
- [Gemini](https://cdn.agentclientprotocol.com/registry/v1/latest/gemini.svg)
- [Cline](https://cdn.agentclientprotocol.com/registry/v1/latest/cline.svg)
- [OpenCode](https://cdn.agentclientprotocol.com/registry/v1/latest/opencode.svg)
- [Pi](https://cdn.agentclientprotocol.com/registry/v1/latest/pi-acp.svg)
- [Grok](https://cdn.agentclientprotocol.com/registry/v1/latest/grok-build.svg)
- [CodeBuddy](https://cdn.agentclientprotocol.com/registry/v1/latest/codebuddy-code.svg)

对其余图标建议复用 Codeg 已适配的 SVG path，不应猜测非稳定 favicon URL。

## 已发现的版本与文档漂移

截至调研日：

| 项目 | Codeg 源码固定值 | 当前 ACP Registry / 文档 |
|---|---:|---:|
| Claude ACP | `0.69.0` | Registry `0.70.0` |
| CodeBuddy | `2.141.0` | Registry `2.142.0` |
| Grok | `1.0.5` | Registry `1.0.13` |
| Kimi Code | `0.39.1` | Codeg 网站仍写 `0.36.1` |
| Kimi Registry identity | `kimi-code` | Registry 当前主要条目是 `kimi` `1.49.0` |
| Qoder Registry identity | `qoder-cli` | Registry 当前主要条目是 `qoder` `0.2.14` |

Kimi `0.37–0.38` 曾破坏 ACP stdio MCP，Codeg 在 `0.39.x` 修复后才更新。初版应复制源码中的固定版本，并把升级视为需要重新跑完整验收的产品变更。

## RambleDesk 实现建议

Agent Catalog 至少需要以下字段：

```text
id
display_name
registry_id
logo
distribution:
  type: npm | binary-tree | binary-file
  package
  pinned_version
  command
  args
  environment
  node_minimum
  platform_artifacts
discovery:
  commands
  shared_config_directory
auth_strategy
quirks
```

以下内容不应静态承诺：

```text
available_models
reasoning_options
access_modes
can_load
can_resume
can_fork
supports_images
```

它们应由实际 ACP 连接产生 capability snapshot。Launch Ramble 弹窗只显示当前连接真实提供的选项；不提供的 selector 应隐藏，而不是显示一个无法生效的通用值。

## 安装与连接验收

每个固定版本至少应执行：

1. 检查平台和 Node/runtime，验证错误原因可直接展示给用户。
2. 检查系统已有命令；没有时走一键受管安装。
3. 启动 ACP 进程并完成 `initialize`，保存 capability snapshot。
4. 验证认证状态；“安装成功”与“已经登录”必须分开。
5. 调用 `session/new`，记录返回的 model、mode、`configOptions`。
6. 发送首轮 prompt，要求 Agent 发起 `request_feedback`。
7. 若 Agent 支持，分别触发 Permission Request 与 Ask Question。
8. 杀掉 ACP 进程后重新连接，按 `resume → load` 顺序验证原 session。
9. 检查失败状态能区分：缺运行时、未安装、未认证、协议不兼容、平台不支持、session 不存在。
10. 将真实安装版本、`agentInfo`、capability snapshot 和测试结果写入测试报告。

本轮结果见 [`ACP_AGENT_CLIENT_ACCEPTANCE.md`](./ACP_AGENT_CLIENT_ACCEPTANCE.md)。其中只有 Claude Code、OpenCode 与 DeepSeek 已完整产生结构化 Feedback Request；其余 Agent 的认证、Toolset 或首轮工具调用边界均单独记录，没有用“已连接”替代“全链路通过”。

没有对应账号或 API key 时，只能声称完成安装与握手验证，不能声称完成真实 Ramble E2E。
