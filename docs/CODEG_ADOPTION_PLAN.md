# Codeg 深度移植：Agent 管理与 Chat

> 状态：执行中。用户于 2026-09-04 明确扩大 `codex/acp-managed-sessions` 范围并授权实施。
> 基线：RambleDesk `ccbc083`；Codeg `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`。
> 前一阶段及修正见 [ACP 提交地图](ACP_COMMIT_MAP.md)，术语以 [TERMINOLOGY.md](TERMINOLOGY.md) 为准。

## 结果目标

普通用户选择智能体、安装所需程序、完成认证、选择项目后即可开始工作。RambleDesk 管理 ACP 启动细节，
Chat 展示流式文本、思考、结构化工具内容和权限；输入器支持稳定的多行编辑、草稿和上下文引用。
模型、模式等选项按后端实际提供的能力展示。每个支持入口都需要完成本项目的反馈闭环验收。

## 采用方式和边界

- 提取 Codeg 适用的 Rust 与无框架 TypeScript 实现和回归案例，记录来源文件、版本、修改；React 视图移植为 Svelte。
- Agent 目录定义产品元数据和分发方式；本机安装记录描述版本和路径；AgentConfig 保存可复用启动选择。
  安装、认证、ACP 连接、反馈能力分别显示，不把一个绿点视为全部就绪。
- 安装器管理 RambleDesk 自有目录，优先复用已安装程序；安装/更新由用户按钮触发，取消或失败不破坏旧版本。
  命令、参数和环境变量保留为高级配置。不同后端的认证使用各自提供的入口，不统一伪造登录成功。
- 聊天内容保存为带类型的内容块，保留工具输入/输出/状态及变更详情；旧文本活动继续可读。
  实时与持久历史保持同一轮次和消息身份，更新不重复追加；关闭视图不停止 Agent。
- Backend Runtime 继续拥有会话身份、实例、权限、持久反馈、投递、恢复和删除。Codeg 的临时 live feedback
  不替代 Request/Package/Delivery，恢复失败不静默创建新 Agent 会话。
- stdio MCP 作为 HTTP 注入之外的接入候选；不同后端需要实际验证，不从 Codeg 目录或 ACP 握手推断兼容。
- 此次扩展包含 Agent 管理与 Chat，不引入 Codeg 的多 Agent 编排、远程部署、全量外部历史导入或 checkout 管理。

## 小提交地图

| 切片 | 独立行为目标 | 验收条件 |
| --- | --- | --- |
| A0 范围与来源 | 更新术语、范围、移植记录及许可证 | 文档一致；后续实现按本表记进度 |
| A1 Agent 目录与安装服务 | 可列出内置 Agent、检测依赖，安装到自有目录并生成启动配置 | fixture 验证安装/取消/失败/版本切换；旧程序不被覆盖 |
| A2 Agent 管理界面 | 列表、详情、安装进度、登录引导与高级配置；新建直接选智能体 | Desktop/Web 同合同，失败给出可执行下一步 |
| C1 结构化会话内容 | 保存并合并文本、思考、工具内容与状态，兼容旧记录 | 迁移、流式 patch、同 ID 更新、轮次与会话隔离、重载一致 |
| C2 Chat 时间线 | 消息/轮次分组、流式 Markdown、思考折叠、工具卡片、历史加载与滚动 | 连续输出、结束、切换、取消、恢复后无丢失或重复 |
| C3 输入编辑器 | 移植 Codeg Tiptap 编辑规则、序列化、快捷键、引用与草稿 | IME、多行、粘贴、Markdown 字面量、草稿和发送清空边界 |
| C4 动态配置与上下文 | 根据 ACP 能力展示模型/模式等选项；受支持的结构化输入 | 选项变化与失败可见；重连刷新；不能选择后端未提供的能力 |
| F1 反馈接入扩展 | 验证需要 stdio 的后端，保持托管归属和自动续接 | 原会话创建请求→提交→读结果→继续，长空闲及停止清理 |
| V1 完整验收 | 实际安装体验、原生/Desktop事件、Web UI、真实后端支持矩阵 | 精确记录版本、实际经过的路径、未验证限制；每个修改有对应回归 |

依赖：A1→A2；C1→C2；C3 可并行开发后集成；C4/F1 与已有生命周期合同接通。
每个切片仍可拆分为更小的完整提交，不集中提交大量实现。前一阶段“未纳入安装器/动态配置/完整 Chat”的
描述只表示首期边界，不再阻止此轮用户授权的扩展。

## 来源记录

源仓库位于 `C:/Users/A/Desktop/codeg`，不作为构建依赖。主要入口：

- `src-tauri/src/acp/{registry,preflight,binary_cache}.rs`、`src-tauri/src/commands/acp.rs`
- `src-tauri/src/acp/{session_state,event_stream,connection}.rs`
- `src/lib/{tool-call-lifecycle,tool-call-normalization}.ts`、`src/lib/adapters/ai-elements-adapter.ts`
- `src/stores/conversation-runtime-store.ts`、`src/components/message/*`、`src/components/ai-elements/*`
- `src/components/chat/composer/*`、`src/components/chat/session-config-selector.tsx`

具体移植文件随提交记录到 [CODEG_PORTS.md](CODEG_PORTS.md)，许可证随代码分发。

## 执行记录

- A0：已建立本地图，正式纳入安装管理、结构化 Chat 与动态配置；保留旧会话与反馈合同。

- C1：`02105b1` 保存结构化消息与工具补丁，已验证旧 schema 迁移、流式顺序、大小限制和原 ACP runtime。
- C3：输入器已接入真实发送/取消与会话草稿，基于现有 Tiptap；48 个编辑器测试及 58 个集成相关测试通过。
- A1：已实现 Agent 目录和安装服务；7 个 fixture 覆盖五条 npm 路线、系统版本检测、失败回滚、取消/超时/future drop 及后代进程清理。尚未将 fixture 视为真实网络安装或全部后端验证。
- A2：设置已接入 Agent 目录、检测、后台安装/取消、认证表单及高级配置。安装任务跨页面保留，Desktop/Web 共用应用接口；模型/模式控件及最终实际 UI 验收继续在 C4/V1。
- C4a：动态配置后端与 Desktop/Web 命令已接通，模型/模式来自协商结果，写入等待 Agent 确认；typed prompt 与配置选择 UI 分别继续。
- C2：结构化时间线已接入 Workspace，保留权限/恢复/反馈界面；33 个有针对性的测试、Svelte 检查及隔离浏览器工具差异卡片通过。
- F1a：stdio companion 已完成真实子进程验证，复用私有 MCP 权限、三工具 schema 和持久请求；不创建额外 continuation。ACP 自动选择及 Pi 原生扩展接线继续。
- C4b：输入区配置控件完成，模型/模式/boolean 来自 Agent 选项并以确认值呈现；8 个专项用例通过，隔离浏览器验证选中后先等待确认。
- C4c：结构化输入后端已接入共用 turn 生命周期，支持协商的图片、内嵌文本与资源链接。真实 stdio、取消、历史预览限额及 continuation 回归通过；Web 独立 5MiB body 上限与 generation 门禁通过。
