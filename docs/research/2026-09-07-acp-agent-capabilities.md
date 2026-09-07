# ACP Agent 能力调研（2026-09-07）

本文覆盖 RambleDesk 当前 catalog 的全部 16 个入口，关注模式、YOLO/自动审批、模型选择、工具授权和结构化用户提问。结论来自本地 catalog、npm 发布元数据、对应版本源码及厂商文档；没有启动真实 Agent、发送模型提示、修改凭据或操作真实会话。

## 结论

1. **不能给所有 Agent 套同一组「默认 / Plan / YOLO」选项。** Claude 的 mode 是权限模式；Codex 将权限/沙箱预设与 collaboration mode 分开；Pi 的 legacy `modes` 是思考强度；OpenCode 的 mode 是主 Agent；Hermes 的 mode 主要控制文件编辑审批。[Claude 0.73.0 权限约定](https://github.com/agentclientprotocol/claude-agent-acp/blob/v0.73.0/docs/permission-extension.md)、[Codex 1.8.0 模式](https://github.com/agentclientprotocol/codex-acp/blob/v1.8.0/src/AgentMode.ts)、[Pi 0.0.33 模式映射](https://github.com/svkozak/pi-acp/blob/1bfcb394088ed879db8fd936b570bb626017f878/src/acp/agent.ts)、[OpenCode 1.18.26 模式来源](https://github.com/anomalyco/opencode/blob/v1.18.26/packages/opencode/src/acp/service.ts)、[Hermes 0.21.0 编辑审批模式](https://github.com/NousResearch/hermes-agent/blob/08b140d14e6c1d49f9b7ad02c9437fe940d54d65/acp_adapter/server.py)。
2. **工具授权与用户问题是两个交互类型。** Claude 0.73.0 的 `AskUserQuestion` 依赖客户端表单 elicitation；客户端不声明支持时，适配器在创建会话时禁用该工具。Codex 1.8.0 在不支持表单时将 user-input 请求返回空答案。Cursor 当前文档则要求响应 `cursor/ask_question`。只实现 `session/request_permission`，不能覆盖这些路径。[Claude](https://github.com/agentclientprotocol/claude-agent-acp/blob/v0.73.0/docs/permission-extension.md)、[Codex](https://github.com/agentclientprotocol/codex-acp/blob/v1.8.0/src/CodexElicitationHandler.ts)、[Cursor](https://cursor.com/docs/cli/acp)。
3. **YOLO 不等于所有交互都自动回答。** Claude 的 bypass 后仍进入权限回调的请求必须保留；Grok 的 Always-approve 仍受 deny/hook 约束，Plan 审查独立；Kimi 则明确有提问与授权的两条处理路径。不要在客户端看到「YOLO」字样后统一自动选择第一项。[Claude](https://github.com/agentclientprotocol/claude-agent-acp/blob/v0.73.0/docs/permission-extension.md)、[Grok](https://docs.x.ai/build/features/permissions)、[Kimi 0.40.1](https://github.com/MoonshotAI/kimi-code/blob/0d45dddc57510e6b1306dd12c0b0703c37b8c63a/packages/acp-server/src/interaction-bridge.ts)。
4. **官方 DSH 与社区 DeepSeek ACP 是不同的产品边界。** 官方 ACP 文档把它定义为 automation-only transport，不暴露模型选择、模式、elicitation 或工具展示；社区 `deepseek-acp@0.8.0` 明确补上这些编辑器能力。不能按同一个 `host_id=dsh` 推导能力。[官方 DSH ACP](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/acp/acp/README.md)、[社区适配器 0.8.0](https://github.com/xintaofei/deepseek-acp/blob/ae144bada70c07174a8772354984ddf1d2806084/README.md)。

## 范围与证据等级

本地入口来自 `crates/rambledesk-acp/src/agents/catalog.rs`。该 catalog 的所有条目均标为 `Unverified`；安装成功、ACP 握手成功、完整模型交互验证必须分开记录。先用 CodeGraph 查询定位 catalog，再读取未被 CodeGraph 输出的 catalog 文件。工作区根和 docs 下未发现额外落盘的 AGENTS.md；遵守任务中提供的 CodeGraph 指令。

| 标记 | 含义 |
| --- | --- |
| P | 已读取 catalog 指定版本的 tag，或 npm `gitHead` 对应源码；是该版本实现证据，仍不是本机端到端测试 |
| R | 已固定读取的上游 commit，其包版本与 catalog 一致；未建立发布二进制与 commit 的可复现对应关系 |
| D | 当前官方文档/上游文档证据；不能保证 catalog 旧版本完全一致 |
| U | 此次没有足够证据；表示未知，不表示不支持 |

网页可变，以下优先使用 tag/commit 永久链接。模型目录还可能随账户、服务端、地区、provider 配置或策略而改变，因此本文核对「如何选择模型」，不将当天可见的具体模型名单固化为产品能力。

## Catalog 版本与入口

| catalog ID | 展示名 | 固定版本 / 依赖 | ACP 启动方式 | 接入 |
| --- | --- | --- | --- | --- |
| `claude-acp` | Claude Code | `@agentclientprotocol/claude-agent-acp@0.73.0` | `claude-agent-acp` | SDK 适配器 |
| `codex-acp` | Codex CLI | `@agentclientprotocol/codex-acp@1.8.0` | `codex-acp` | app-server 适配器 |
| `gemini` | Gemini CLI | `@google/gemini-cli@0.57.0` | `gemini --acp` | 原生 |
| `openclaw-acp` | OpenClaw | `openclaw@2026.8.1` | `openclaw acp` | CLI 内置 Gateway 桥 |
| `cline` | Cline | `cline@3.0.61` | `cline --acp` | 原生 |
| `codebuddy` | CodeBuddy | `@tencent-ai/codebuddy-code@2.143.0` | `codebuddy --acp` | 原生 |
| `kimi` | Kimi Code | `@moonshot-ai/kimi-code@0.40.1` | `kimi acp` | 新 TypeScript Kimi Code |
| `pi-acp` | Pi | `pi-acp@0.0.33` + `@earendil-works/pi-coding-agent@0.83.0` | `pi-acp` → `pi --mode rpc` | 适配器 |
| `grok` | Grok | `@xai-official/grok@1.0.13` | `grok --no-auto-update agent stdio` | 原生 |
| `deepseek-acp` | DeepSeek ACP | `deepseek-acp@0.8.0` | `deepseek-acp` | 社区适配器 |
| `dsh` | DeepSeek Harness | `@deepseek-ai/dsh@0.1.2-rc.1` | `dsh --profile acp` | 官方 profile |
| `qoder` | Qoder | `@qoder-ai/qodercli@1.1.41` | `qoder --acp` | 原生 |
| `opencode` | OpenCode | `1.18.26`，手动安装 | `opencode acp` | 原生 |
| `cursor` | Cursor | `2026.08.31-4057e58`，手动安装 | `cursor-agent acp` | 原生 |
| `hermes` | Hermes Agent | `0.21.0`，手动安装 | `hermes acp` | 内置适配器 |
| `antigravity` | Google Antigravity | catalog `1.0.0`，并非上游 binary build ID | `agy_acp_server`（Linux 附 `--uid=`） | 手动二进制 |

注意：catalog 将 OpenClaw 归类为 Native，厂商文档描述其内部实现为 Gateway-backed bridge。这两个口径分别描述「是否另装适配器」和「运行时架构」，没有矛盾。Claude/Codex 的厂商 CLI 与 ACP 适配器也是不同版本，不能用厂商 CLI 的版本号替代 ACP 适配器版本。

## 全量能力矩阵

「授权」列指工具执行许可；「提问」列指结构化用户输入，不包括普通聊天文字中的问号。

| Agent / 证据 | 模式与自动审批 | 模型控制 | ACP 工具授权 | 结构化用户提问 |
| --- | --- | --- | --- | --- |
| **Claude Code / P** | `default`、`acceptEdits`、`plan`；`auto`、`bypassPermissions` 有条件出现。`dontAsk` 可由设置接受但不作为选择项公布 | 由会话提供模型/config options；provider override 与可见列表可配置；模型可能影响可用模式 | 标准 `session/request_permission`，精确 option ID；durable effect 由适配器执行 | **表单 elicitation**；未声明 `elicitation.form` 时禁用 `AskUserQuestion` |
| **Codex CLI / P** | `mode`：`read-only`（显示 Ask for approval）、`agent`（Approve for me）、`agent-full-access`。独立 `collaboration_mode`：`default/plan` | 动态模型、reasoning effort，支持 config options；不要把 effort 后缀当新模型种类 | 标准 permission；Full access 配置 `never` + `danger-full-access`，其余预设有沙箱/审批组合 | user-input → **表单 elicitation**；缺少支持时返回空答案；MCP form/URL 另有处理 |
| **Gemini CLI / P** | `default`、`auto_edit`、`yolo`；Plan 按配置启用 | 公布 `models`，提供 `setModel`；模型列表受 preview access/config 影响 | 标准 permission，存在一次/本会话/条件性永久授权选项 | **U**：此次已查的 ACP 核心源码不足以确认结构化 ask-user 的完整桥接路径；不能据 CLI 有 ask-user 工具就标支持 |
| **OpenClaw / P** | `session/set_mode` 及 Gateway 控制子集：思考、工具详细度、reasoning、usage、elevated；不是统一 Plan/YOLO | **不作为 ACP config option 暴露模型选择**；由 Gateway/Agent 配置控制 | 当前活动 prompt 的 Gateway exec approval 转发标准 permission | **U**：固定版本 ACP 文档未给出完整通用表单/提问协议保证 |
| **Cline / R** | `plan/act` 与独立布尔 `autoApproveTools`；两者不要合并 | provider、model、mode、auto-approve config options；兼容 legacy models/modes | 标准 permission；auto-approve 开启时绕过发给客户端的审批 | **U**：已查 ACP host capability 仅接 `requestToolApproval`；没取得独立提问回调完整桥接证据 |
| **CodeBuddy / D** | CLI 文档列 default/acceptEdits/auto/dontAsk/plan/bypassPermissions；runtime 还接受 fullAccess。实际 ACP picker 需协商 | `--model`；官方发布记录明确实现 ACP `unstable_setSessionModel` 并修复即时生效 | 官方 ACP/发布记录确认授权请求；高风险限制可能在 bypass 下继续存在 | 官方确认 AskUserQuestion 及 interruption 广播路径；**不可假定它等于标准 elicitation**，需实际协议契约 |
| **Kimi Code / P** | `default/plan/auto/yolo`；由 plan toggle + permission mode 映射 | config options 统一 model/thinking/mode；兼容 `session/set_model` | 标准 permission | 优先 **form elicitation**；RPC 失败或不支持时 fallback permission。fallback **只问第一题、降为单选** |
| **Pi / P** | legacy `modes` 是 `Thinking: <level>`；**不是权限模式**。没有验证可供客户端切换的权限预设 | model + thought_level config options；provider/model 动态列表 | **U/未见内置桥接**：在指定适配器核心未取得 permission gate；不要套用另一个同名 fork 的说明 | **U**：未取得指定适配器的原生表单或问题 callback 完整映射证据；Pi 扩展能力不等于此桥已接入 |
| **Grok / D** | 官方 CLI：Ask、条件性 Auto、Always-approve；Plan 独立。legacy yolo 与 permission_mode 有兼容关系 | CLI `--model`、自定义 provider/model 与 TUI `/model` 有官方文档；**ACP 动态 model picker 的 1.0.13 契约 U** | 官方权限系统已证；**本次未取得 1.0.13 的公开 wire-level permission 规范/源码** | **U**：未取得 1.0.13 的问答 RPC 与 capability 条件证据，不能推导为 Claude 的 AskUserQuestion 协议 |
| **DeepSeek ACP / P** | Plan；独立 sandbox/file-permission selector。默认 workspace-write；跨边界可请求单次提升 | 动态 model、reasoning、sandbox config options | 标准 permission，包含单次提升 | **form elicitation + 按钮 fallback**；是社区 adapter 明确补齐的能力 |
| **官方 DSH / D** | 官方文档明确 **不暴露 modes/config pickers** | 在 transport 的 provider/model 配置选初始模型，**不提供 ACP 会话 picker** | 一次性 permission 请求由客户端策略解决 | 官方文档明确 **不暴露 elicitation**，人类提问属于 Web host/client 模块 |
| **Qoder / D** | ACP 文档列 Default/Bypass；通用 CLI 另有 accept_edits/auto/dont_ask，`plan` 为工作状态兼容入口。实际 ACP 广告为准 | CLI 动态 model tier/frontier/BYOK 与 effort；**catalog 版本 ACP model picker U** | 官方文档明确 ACP `requestPermission`；组织策略、受信任目录影响模式 | **U**：未取得 ACP 结构化提问 wire contract |
| **OpenCode / P** | ACP mode 来自非 hidden、非 subagent 的 Agent 名称，默认常为 `build`；不是统一 YOLO 菜单 | model、effort、mode config options；provider/model/variant 映射 | `permission.asked` → ACP handler | **U**：指定版本 ACP event handler 已核对 permission 分支，未取得 `question.asked`/elicitation 完整桥接证据 |
| **Cursor / D** | ACP `agent/plan/ask`；agent 的 full tool access 不等于无需审批 | CLI 模型支持不自动证明当前 ACP 的动态 picker；**固定 build picker 契约 U** | 标准 `session/request_permission` | **专有阻塞 RPC `cursor/ask_question`**；计划审批另用 `cursor/create_plan`，必须答复 JSON-RPC |
| **Hermes / R** | `default/accept_edits/dont_ask` 映射文件编辑策略；`dont_ask` 在此意为自动批准部分编辑，**不能按其他 Agent 同名模式解释** | 已认证 provider/model 库，支持 legacy models 与 set_session_model；模型切换重建 session agent | 标准 permission；危险命令与文件编辑有各自策略；always 作用域需遵循 adapter | **U**：此次核对源码和 ACP 文档不足以确认完整的结构化提问 transport |
| **Antigravity / D+U** | **U**：当前官方 registry 只提供发行信息，不足以证明模式语义 | **U** | **U** | **U** |

### 矩阵的逐项来源

- **Claude / P**：[`v0.73.0` permission extension](https://github.com/agentclientprotocol/claude-agent-acp/blob/v0.73.0/docs/permission-extension.md)，尤其 AskUserQuestion、Permission modes、bypass callback 三段；[model configuration](https://github.com/agentclientprotocol/claude-agent-acp/blob/v0.73.0/docs/model-configuration.md)。该 tag 解析为 `ea7076c0bc324603e65d8c124b7573f158749969`。
- **Codex / P**：[`AgentMode.ts`](https://github.com/agentclientprotocol/codex-acp/blob/v1.8.0/src/AgentMode.ts)、[`CollaborationModeConfig.ts`](https://github.com/agentclientprotocol/codex-acp/blob/v1.8.0/src/CollaborationModeConfig.ts)、[`CodexElicitationHandler.ts`](https://github.com/agentclientprotocol/codex-acp/blob/v1.8.0/src/CodexElicitationHandler.ts)、[`ModelConfigOption.ts`](https://github.com/agentclientprotocol/codex-acp/blob/v1.8.0/src/ModelConfigOption.ts)。tag commit `87997e2627e8fa246a49de533c612f6196c4004e`。注意 ID `read-only` 的实际配置是 workspace-write，不可仅按 ID 翻译成严格只读。
- **Gemini / P**：[`v0.57.0 acpUtils.ts`](https://github.com/google-gemini/gemini-cli/blob/v0.57.0/packages/cli/src/acp/acpUtils.ts) 的 `buildAvailableModes`、`buildAvailableModels`、`toPermissionOptions`；[`acpSession.ts`](https://github.com/google-gemini/gemini-cli/blob/v0.57.0/packages/cli/src/acp/acpSession.ts) 的 `setMode`、`setModel` 与 `requestPermission`。tag commit `6b0ae9a6c37aa117cc8b070d8b41c5bb4fa6d253`。
- **OpenClaw / P**：[`v2026.8.1 docs/cli/acp.md`](https://github.com/openclaw/openclaw/blob/v2026.8.1/docs/cli/acp.md)，Supported features / Known limitations。文档明确区分本 catalog 的 `openclaw acp` 服务端与 `openclaw acp client` 调试客户端、ACPX 外部 harness。不能把 ACPX 的 `approve-all` 配置复制成此服务端的模式。tag commit `ea806575e6450e4d1efdfc72c19f04be982a1b9b`。
- **Cline / R**：固定 commit [`c21b172… acpAgent.ts`](https://github.com/cline/cline/blob/c21b17255b228e88a1518c18a73a473ee5876362/apps/cli/src/acp/acpAgent.ts)、[`permissions.ts`](https://github.com/cline/cline/blob/c21b17255b228e88a1518c18a73a473ee5876362/apps/cli/src/acp/permissions.ts)、[`auto-approve.ts`](https://github.com/cline/cline/blob/c21b17255b228e88a1518c18a73a473ee5876362/apps/cli/src/acp/auto-approve.ts)。该 commit `apps/cli/package.json` 为 3.0.61，但没有发布包 gitHead，因此不将源码相同版本等同于 npm 构建复现。
- **CodeBuddy / D**：[ACP 文档](https://www.codebuddy.ai/docs/cli/acp)、[CLI reference](https://www.codebuddy.ai/docs/cli/cli-reference)、[2.97.0 model switch 修复](https://www.codebuddy.ai/docs/cli/release-notes/v2.97.0)、[2.78.0 questionnaire interruption 路由](https://www.codebuddy.ai/docs/cli/release-notes/v2.78.0)。这些能证明厂商实现过对应功能，不能替代 2.143.0 具体 wire payload 验证。
- **Kimi / P**：npm 包指向 **MoonshotAI/kimi-code**，不是 MoonshotAI/kimi-cli。tag `@moonshot-ai/kimi-code@0.40.1` 解引用到 `0d45dddc57510e6b1306dd12c0b0703c37b8c63a`。[该版本 ACP 文档](https://github.com/MoonshotAI/kimi-code/blob/0d45dddc57510e6b1306dd12c0b0703c37b8c63a/docs/en/reference/kimi-acp.md)、[mode taxonomy](https://github.com/MoonshotAI/kimi-code/blob/0d45dddc57510e6b1306dd12c0b0703c37b8c63a/packages/acp-server/src/modes.ts)、[interaction bridge](https://github.com/MoonshotAI/kimi-code/blob/0d45dddc57510e6b1306dd12c0b0703c37b8c63a/packages/acp-server/src/interaction-bridge.ts)。旧 Python Kimi 的 AFK/print 文档不能直接用于这个 npm 包。
- **Pi / P**：npm 0.0.33 的 repository 是 **svkozak/pi-acp**，gitHead `1bfcb394088ed879db8fd936b570bb626017f878`。[README](https://github.com/svkozak/pi-acp/blob/1bfcb394088ed879db8fd936b570bb626017f878/README.md)、[`src/acp/agent.ts`](https://github.com/svkozak/pi-acp/blob/1bfcb394088ed879db8fd936b570bb626017f878/src/acp/agent.ts)。网上有 aadishv、victor-software-house 等同名/派生仓库，不能混引它们新增的 permission 或 elicitation 能力。
- **Grok / D**：[官方 ACP 启动说明](https://docs.x.ai/build/cli/headless-scripting)、[Permissions](https://docs.x.ai/build/features/permissions)、[Enterprise](https://docs.x.ai/build/enterprise)、[模型/provider 配置](https://docs.x.ai/build/overview)。npm 1.0.13 元数据有 gitHead `5e9a58528b76b6128ee610059d79aecaa71b9b8d`，没有公开 repository 字段；此次未取得可验证该 commit 的公开源码。未用社区帖作为 wire contract 证据。
- **DeepSeek ACP / P**：npm gitHead `ae144bada70c07174a8772354984ddf1d2806084`。[README 的官方/社区差异表](https://github.com/xintaofei/deepseek-acp/blob/ae144bada70c07174a8772354984ddf1d2806084/README.md)、[session config](https://github.com/xintaofei/deepseek-acp/blob/ae144bada70c07174a8772354984ddf1d2806084/src/protocol/session-config.ts)、[elicitation answerer](https://github.com/xintaofei/deepseek-acp/blob/ae144bada70c07174a8772354984ddf1d2806084/src/answerers/elicitation.ts)。
- **官方 DSH / D**：[官方 ACP README](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/acp/acp/README.md)、[官方 ACP app bundle](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/bundle/acp-app/README.md)。这两个 master 文档明确限定 automation-only。npm 0.1.2-rc.1 无 gitHead；本次未做发布包内嵌依赖版本的逐文件审计，因此单独标 D。
- **Qoder / D**：[ACP](https://docs.qoder.com/cli/acp)、[Permissions](https://docs.qoder.com/cli/permissions)、[模型与 effort](https://docs.qoder.com/cli/model)。通用 CLI 文档中的模式集合比 ACP 页列举的集合更大；不要据此擅自补充 ACP picker。使用国际版 Qoder 文档，未将 Qoder CN 的 token/命令名混入 catalog。
- **OpenCode / P**：[`v1.18.26 config-option.ts`](https://github.com/anomalyco/opencode/blob/v1.18.26/packages/opencode/src/acp/config-option.ts)、[`service.ts`](https://github.com/anomalyco/opencode/blob/v1.18.26/packages/opencode/src/acp/service.ts)、[`event.ts`](https://github.com/anomalyco/opencode/blob/v1.18.26/packages/opencode/src/acp/event.ts)、[`permission.ts`](https://github.com/anomalyco/opencode/blob/v1.18.26/packages/opencode/src/acp/permission.ts)。tag commit `774cc7c1914e4329eefde5a669f938b0cf566661`。源码搜索没找到完整 question 路径不足以证明全产品不支持问题，因此矩阵保留 U。
- **Cursor / D**：[当前官方 ACP 文档](https://cursor.com/docs/cli/acp)。文档给出 `cursor/ask_question` 的问题数组与选项 ID、多选标志，并明确它与 `cursor/create_plan` 是阻塞方法。未获取 catalog build 的公开源码。
- **Hermes / R**：固定 commit `08b140d14e6c1d49f9b7ad02c9437fe940d54d65` 的 [server.py](https://github.com/NousResearch/hermes-agent/blob/08b140d14e6c1d49f9b7ad02c9437fe940d54d65/acp_adapter/server.py)、[ACP guide](https://github.com/NousResearch/hermes-agent/blob/08b140d14e6c1d49f9b7ad02c9437fe940d54d65/website/docs/user-guide/features/acp.md)。此 commit pyproject 版本为 0.21.0；不是已验证的发行二进制哈希。
- **Antigravity / D**：官方 ACP registry commit `50f99cde7470920f24b8aec9c195f25281675ae9` 的 [agent.json](https://github.com/agentclientprotocol/registry/blob/50f99cde7470920f24b8aec9c195f25281675ae9/antigravity-acp/agent.json) 当前为 **1.1.1**，binary 指向 Google 下载域，Windows 命令为 `.exe`，macOS/Linux 为 `.par`。它只能证明发行和启动信息，不能证明四项会话能力。catalog 1.0.0 不能充当上游 build 证据。

## Claude 与 Grok：可以下结论的差异

| 比较项 | Claude adapter 0.73.0 | Grok CLI 1.0.13 |
| --- | --- | --- |
| 可审计资料 | 对应 tag 的公开 SDK 适配器源码和权限文档 | 官方 CLI 使用文档；本次没有对应公开 ACP 实现 |
| 权限模式语义 | SDK wire IDs；Auto/Bypass 条件可用 | 官方文档为 Ask/Auto/Always-approve，Plan 独立；是否全部映射为 ACP mode 未证 |
| 默认不弹问题的已知原因 | 客户端没有声明 form elicitation 会直接禁用 AskUserQuestion | 无充分证据证明同一机制；不能沿用 Claude 判断 |
| bypass 后仍收到 request | 适配器文档明确必须显示，不能由客户端代为批准 | deny/hook、Plan review 仍可约束；具体 ACP payload 尚需版本化记录 |

因此，如果 RambleDesk 上表现为「Grok 可以交互而 Claude 不会问」，优先核查 Claude 的 initialize 中是否声明 `elicitation.form`、客户端是否实现对应请求/响应，以及是否在创建会话之后才补能力。**这只是有源码依据的排查方向，尚不是本机根因认定。** Grok 的成功也只证明已走通的具体路径，不证明所有 Agent 共享该路径。

## 对 RambleDesk 的实现约束

1. **运行时协商优先。** 初始状态读取 `session/new/load/resume` 返回的 `configOptions`、兼容 `modes/models`；应用 `config_option_update`、`current_mode_update`。优先使用 category 来布置模型、思考和模式，保留原始 ID、name、description。没有广告时明确显示不可用或交由 Agent 配置，不合成模式/模型。
2. **三种等待分别建模。** 工具授权、结构化提问、计划确认可以共享排队与生命周期，但数据和用户决策必须分别保存。问题选项即使落在 permission 的 `allow_once` 上，也不是「允许运行工具」的自动审批依据。
3. **严格回传给定 ID。** `PermissionOption.kind` 只是粗粒度分类；实际许可、长期规则与会话作用域由 Agent 控制。不能按按钮文字猜 scope，也不能把不同 Agent 的同名 `dontAsk` 或 `auto` 映射为同一行为。
4. **仅声明已经实现的客户端能力。** `elicitation.form` 支持需要包含 schema 校验、单/多题、单/多选、输入值验证、decline/cancel、请求取消与会话关闭清理。Cursor 扩展单独适配；未知阻塞 RPC 应有明确错误与可诊断信息，不能静默丢弃。
5. **认证单独处理。** `initialize.authMethods` / `authenticate` 与工具授权不是一回事。账号权限、模型列表、模式可用性和项目 trust 都可能改变广告结果；复用厂商登录不等于已验证桥版本。
6. **静态 catalog 只记录发行和已核查证据。** 可增添 capability evidence 的 source/version/date 字段；不将本文 D/R/U 直接转成运行时 true/false。所有支持徽标应区分「上游声明」「当前连接广告」「本机交互实测」。

协议依据：[Session Config Options](https://agentclientprotocol.com/protocol/v1/session-config-options)、[ACP elicitation 设计及能力要求](https://github.com/agentclientprotocol/agent-client-protocol/blob/main/docs/rfds/elicitation.mdx)。Elicitation 文档所在路径为 RFD，客户端 SDK/协议版本仍须对齐；不应只看到设计文档便声明支持。

## Codeg 客户端参考：Grok 扩展（不是厂商协议保证）

另一条独立证据来自本机 `C:/Users/A/Desktop/codeg`，checkout commit 为 `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。这是 **Codeg 客户端已经实现的兼容逻辑**，可以作为 RambleDesk 的参考适配器和 fixture 来源；它不是 Grok 1.0.13 的厂商规范，也不证明未来版本保持兼容。因此上表 Grok 的上游 wire-level 项仍保留 U，同时补充这层客户端实现证据。

| 参考路径 | Codeg 可见的协议结构 | 对 RambleDesk 的意义 |
| --- | --- | --- |
| [question.rs:460](C:/Users/A/Desktop/codeg/src-tauri/src/acp/question.rs:460)、[connection.rs:5912](C:/Users/A/Desktop/codeg/src-tauri/src/acp/connection.rs:5912) | 原生阻塞 RPC `_x.ai/ask_user_question`；参数含 `sessionId`、`toolCallId`、`mode`，`questions` 元素含 `question`、`options[{label,description}]`、`multiSelect` | 方法名保留开头的下划线；可将其归一化为问题表单，保留完整原始请求关联 |
| [question.rs:682](C:/Users/A/Desktop/codeg/src-tauri/src/acp/question.rs:682) | 回答为 `{outcome:'accepted',answers:{[question text]:string|string[]},partial_answers:{}}`；跳过为 `{outcome:'skip_interview'}` | 回答键是问题原文；这与 Cursor 的 questionId/selectedOptionIds 不同，不能混用 |
| [plan_approval.rs:1](C:/Users/A/Desktop/codeg/src-tauri/src/acp/plan_approval.rs:1)、[plan_approval.rs:160](C:/Users/A/Desktop/codeg/src-tauri/src/acp/plan_approval.rs:160)、[connection.rs:5923](C:/Users/A/Desktop/codeg/src-tauri/src/acp/connection.rs:5923) | 原生阻塞 RPC `_x.ai/exit_plan_mode`；请求含 `sessionId`、`toolCallId`、`planContent:string|null`；响应 `outcome` 为 approved/abandoned/keep_planning，另含 feedback | 计划审批应保留这组原生结果，不简化为 permission allow/deny；feedback 的效果还需版本化验证 |
| [connection.rs:2901](C:/Users/A/Desktop/codeg/src-tauri/src/acp/connection.rs:2901)、[connection.rs:3037](C:/Users/A/Desktop/codeg/src-tauri/src/acp/connection.rs:3037)、[connection.rs:3199](C:/Users/A/Desktop/codeg/src-tauri/src/acp/connection.rs:3199) | 客户端有模型规格、模型信息合成及 set_model 路径 | 可以研究可用性补偿，但不能将 Codeg 合成选项误写为 Agent 原生广告 |

参考源码由本任务的 Codeg 对照子任务核对并提供行号，调研子任务也复核了原生方法名与响应注释。`question.rs:682` 的注释称 Codeg 作者曾用 **Grok 0.2.101** 真实运行确认 accepted 响应形状；这是参考项目对旧版本的验证声明，既不是本次重现实测，也不是 catalog **1.0.13** 的确认。它补全了「如何做兼容 fixture」的证据，不改变「本次未运行真实 Grok 会话」的限制。

另一个已核实的标准 schema 细节：Claude 0.73.0 的 [elicitation.ts:208](https://github.com/agentclientprotocol/claude-agent-acp/blob/v0.73.0/src/elicitation.ts#L208) 与 Kimi 0.40.1 的 [question.ts:142](https://github.com/MoonshotAI/kimi-code/blob/0d45dddc57510e6b1306dd12c0b0703c37b8c63a/packages/acp-server/src/question.ts#L142) 使用 `{type:'array',items:{anyOf:[{const:'值',title:'标签'}]}}`；**items 没有 type 字段**。前后端必须识别这种有标题的多选枚举，不能额外要求 `items.type === 'string'`。这应进入真实来源形状的回归 fixture。

## 最小验证矩阵与研究/实现边界

下面是调研提出的客户端验证矩阵。本任务的实现阶段另行执行了/正在执行本地 fixture 与前端测试；其实际通过项以实现阶段的命令输出为准，本文不重复认证测试结果。这些 fixture 不消耗模型、不修改真实会话：

- configOptions-only、legacy-only、两者同时存在、模型切换导致 mode/effort 列表变化。
- 同一 turn 多个并发 permission，未知 option ID，cancel 后迟到的回答，断线与关闭。
- form elicitation 单题、多题、多选、自填；accept/decline/cancel；无能力协商；URL 模式独立。
- Kimi/DeepSeek 的 permission fallback 问题；确保自动审批不会替用户选择答案。
- Cursor `cursor/ask_question` 与 `cursor/create_plan`，验证阻塞 RPC 一定结束。
- DSH 无 picker、Pi Thinking modes、Hermes 同名不同义模式，防止客户端按品牌或 ID 猜测语义。

真实 Agent 的模型驱动验证是另一层证据：在用户已授权的独立临时工作目录，按准确版本、账户和模型验证，保存脱敏 handshake、session response、互动 request/response 和版本证据。本调研没有执行这层实机验证；本地 fixture 通过也不能证明 16 个入口均已在 RambleDesk 完整可用。
