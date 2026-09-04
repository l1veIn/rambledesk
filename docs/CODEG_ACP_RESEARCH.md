# Codeg：ACP 接入与 Agent 设置参考

> 后续状态：本文件保留固定版本的调研结论；RambleDesk 当前已实现的功能与后端支持以
> [ACP 托管会话指南](ACP_MANAGED_SESSIONS.md)、[实机探针](ACP_BACKEND_PROBE.md) 和
> [源码借鉴地图的采用记录](CODEG_ACP_REFERENCE_MAP.md) 为准。

> 日期：2026-09-04；状态：源码与官方文档调研，未运行 Codeg 或真实 Agent 验证。
> 来源：用户指定的 [xintaofei/codeg](https://github.com/xintaofei/codeg)。
> 源码快照：[`3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`](https://github.com/xintaofei/codeg/tree/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1)。
> 本文记录来源观察与设计取舍；RambleDesk 术语仍以 [TERMINOLOGY.md](TERMINOLOGY.md) 为准。

后续已 clone 本地参考库，并补充会话、权限、进程、反馈投递和重连的
[源码借鉴地图](CODEG_ACP_REFERENCE_MAP.md)，包含可移植单元、与本项目不同的语义及对应回归案例。

已有 [Codeg Web Service 调研](CODEG_WEB_SERVICE_RESEARCH.md) 使用另一源码快照并聚焦应用传输；本文补充
ACP 启动方式与设置页，不把两次调研当作同一版本的兼容性证据。官网文档是访问当日内容，源码链接固定版本。

## 接入方式：后端家族不等于 ACP 入口

Codeg 的注册表把展示元数据、MCP 接入声明与 distribution 分开，启动描述支持 Npx、Binary、Uvx 三种
形式。当前内置入口包含以下分组；这些是 Codeg 的选择，不是 RambleDesk 已支持的名单。

| 接入分组 | Codeg 中的后端 |
| --- | --- |
| 独立 ACP bridge，npm 分发 | Claude Code、Codex、Pi、DeepSeek Harness |
| 带 ACP / stdio 子命令或标志的入口，npm 分发 | Gemini、OpenClaw、Cline、Hermes、CodeBuddy、Kimi Code、Grok、Qoder |
| Binary 分发的 ACP 入口 | OpenCode、Cursor、Google Antigravity |

其中 Codex 使用 `@agentclientprotocol/codex-acp@1.8.0`，Pi 使用 `pi-acp@0.0.33`，DeepSeek Harness
使用 `deepseek-acp@0.8.0`。不能用“安装了后端 CLI”推断相应 bridge 已就绪；也不能从 distribution
形式推断实际进程数量。以上观察来自固定版本的
[ACP 注册表](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/registry.rs)。

**dsh 待实测**：Codeg 选择社区 bridge，前期讨论的是官方 `dsh --profile acp`。注册表注释还记录了
其选择理由，但注释不是当前官方入口的验证结果。步骤 2 应分别记录两条路径的版本、初始化能力、流式输出、
权限、`mcpServers` 的实际工具调用及恢复支持，然后选定首个闭环后端。此次调研不宣称任一路径已通过。

**Pi 待实测**：同一注册表记录 `pi-acp` 仍依赖实际 Pi 可执行程序，并注明 MCP 配置转发存在例外。
因此元数据布尔值与握手成功都不能替代真实反馈工具验证；“能聊天”与“能完成 RambleDesk 反馈闭环”应分别记录。

## 设置页：列表、详情与检查结果

从组件结构可确认：左侧为可排序的 Agent 列表，展示启用状态和检查摘要；右侧为所选 Agent 的详情，包含
分发类型/bridge 标识、启用开关、检查结果和后端专属配置。页首提供添加自定义 Agent，错误与诊断可定位到
具体 Agent。安装、版本与运行依赖检查也在同一设置流程中。这里是源码层面的 UI 分析，尚未做视觉或交互验收。
来源：[Agent 设置组件](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src/components/settings/acp-agent-settings.tsx#L7520)。

自定义 Agent 支持 registry 与手工输入两条路径，手工输入描述 distribution；它是可复用的 Agent 定义，
不是新建会话表单。来源：[自定义 Agent 文档](https://docs.codeg.app/guide/custom-agents) 与
[添加对话框](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src/components/settings/add-custom-agent-dialog.tsx)。

持久设置单独保存后端类型、registry id、enabled、排序、安装版本、环境覆盖与 provider 关联；连接对象另有
id、status、会话状态、输入互斥与 child pid。这支持“可复用配置与当前运行资源分开”的设计判断，不要求照搬字段。
来源：[agent_setting](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/db/entities/agent_setting.rs) 与
[AgentConnection](https://github.com/xintaofei/codeg/blob/3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1/src-tauri/src/acp/connection.rs#L1077)。

## RambleDesk 的采用范围（设计决策）

| 参考点 | 首期取舍 |
| --- | --- |
| Agent 列表 + 详情 | 设置页独立为步骤 9a；展示配置名称、后端、启用与检查状态，右侧编辑所选配置。 |
| 启动配置 | 使用已安装可执行程序、结构化参数与环境配置；后端家族、反馈适配器、启动配置保持不同身份。 |
| 检查状态 | 分别表达配置已保存、程序可启动、ACP 已连接及反馈工具已验证；错误显示对应阶段，不用一个“已安装”推断所有能力。 |
| 后端专属选项 | 根据已验证的启动约定与协商能力显示；不把一种后端的认证、模型或权限选项应用到所有后端。 |
| 会话创建 | 独立在步骤 9b；选择已保存的 Agent 配置与工作目录，一会话独占一个实例。 |
| 安装与 registry | 首期不实现通用安装器、自动更新、registry 导入及任意分发 JSON；后续按实际需要扩展。 |
| 术语 | 用户入口称“Agent 配置”；RambleDesk 自身承担 ACP Client 角色，外部程序是 ACP Server 或 Bridge。 |

Codeg 的完整设置系统可作为后续兼容性研究入口，首期不复制其全部后端特例，也不将其历史导入、空会话过滤、
权限默认值或自动安装策略当作 RambleDesk 的默认行为。

## 后续实现前的验证清单

1. 固定首个真实后端及 bridge 版本，记录进程树与关闭后的资源状态。
2. 在两个不同工作目录各开一个会话，验证消息、权限和反馈归属互不串用。
3. 实际调用反馈工具并完成提交后续接；单独记录 MCP 注入、原生反馈工具与恢复能力。
4. 在 UI 步骤开始前重新对照 Codeg 的页面与本项目组件，做 RambleDesk 自己的视觉与交互验收。
