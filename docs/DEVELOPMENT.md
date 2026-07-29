# RambleDesk 开发基线与实施计划

> 状态：M0 complete · Ready to start M1
> 日期：2026-07-29

## 1. 已冻结的开发方向

| 领域 | 决定 |
|------|------|
| 桌面框架 | Tauri 2 |
| 前端 | Svelte 5 + TypeScript + Vite |
| 核心逻辑 | Rust；前端不直接操作数据库或发布反馈包 |
| 异步运行时 | Tokio |
| MCP | 官方 Rust SDK `rmcp` 3.0.0，Streamable HTTP |
| 持久化 | SQLite，Rust 侧访问并执行显式 migrations |
| 序列化/schema | Serde + Schemars |
| 日志 | tracing；默认只记录元数据 |
| ID | UUIDv7 |
| 时间 | UTC，边界使用 RFC 3339 |
| 包管理 | pnpm；Rust 使用 Cargo |
| 测试 | Rust 单元/集成测试 + Svelte 静态检查/组件测试 + Playwright 桌面关键路径 |

选择 Svelte 5 是工程默认，不是产品合同。它与已验证的 Kotone Tauri workspace
保持一致，便于复用脚手架、事件桥接和桌面构建经验。UI 必须保持薄层，未来替换
前端不得影响领域模型和 MCP 合同。

## 2. 工程边界

```text
rambledesk/
├── Cargo.toml                   # 虚拟 Cargo workspace
├── Cargo.lock
├── rust-toolchain.toml
├── package.json                 # 只保留根级转发脚本
├── pnpm-workspace.yaml          # apps/*；单一 pnpm lock
├── apps/
│   └── desktop/
│       ├── package.json
│       ├── src/                 # Svelte UI
│       │   ├── features/
│       │   │   ├── inbox/
│       │   │   ├── request-workspace/
│       │   │   ├── history/
│       │   │   └── settings/
│       │   ├── components/
│       │   └── lib/
│       └── src-tauri/           # rambledesk-desktop：Tauri 薄壳与 composition root
├── crates/
│   ├── rambledesk-core/         # 领域模型、ports、use cases、状态机
│   ├── rambledesk-storage/      # SQLite、draft、Feedback Package 发布
│   ├── rambledesk-mcp/          # MCP schema、server、auth 与执行模式适配
│   ├── rambledesk-speech/       # M3：音频采集、STT 适配与模型管理
│   └── rambledesk-cli/          # 无 Tauri 的开发/自动化消费者
├── docs/
├── scripts/
└── tests/
    ├── fixtures/
    ├── protocol/
    └── e2e/
```

Cargo members：

```toml
[workspace]
resolver = "2"
members = ["apps/desktop/src-tauri", "crates/*"]
```

根级依赖只在 `[workspace.dependencies]` 声明版本，成员通过
`dependency.workspace = true` 引用。前端依赖使用单一根级
`pnpm-lock.yaml`。

### 2.1 Crate 职责

#### `rambledesk-core`

- 唯一领域模型和状态迁移；
- application use cases；
- Repository、PackagePublisher、Notifier、Clock 等 ports；
- 不依赖 Tauri、MCP SDK、SQLx、cpal 或任何 STT SDK；
- 不直接读取环境变量或全局路径。

#### `rambledesk-storage`

- SQLite repositories 和 migrations；
- Draft 文件管理；
- Feedback Package 临时写入、哈希、fsync 与原子发布；
- 实现 core 定义的持久化 ports；
- 不认识 MCP 或 Tauri。

#### `rambledesk-mcp`

- 本文档定义的工具 schema；
- Streamable HTTP server、认证和 Host/Origin 防护；
- MCP Tasks / polling 到 core use cases 的适配；
- 不持有独立业务状态，不直接写数据库或反馈文件。

#### `rambledesk-speech`

- M3 才进入默认构建；
- cpal 音频采集、重采样、有背压的 PCM 通道；
- STT engine trait 的实现、模型清单和下载器；
- 允许以 Cargo feature 隔离 sherpa-onnx 等重依赖；
- 不能承担 Draft、Request 或提交语义。

#### `rambledesk-cli`

- 无 GUI 启动 MCP 的开发入口；
- 数据库/Feedback Package 诊断；
- WAV fixture 直灌与语音回归；
- 证明 core、storage、mcp 和 speech 不依赖 Tauri。

#### `rambledesk-desktop`

- Tauri 生命周期、窗口、托盘、系统通知和权限提示；
- 装配各 crate 的具体实现；
- Tauri commands/events；
- 不实现领域规则。

### 2.2 依赖方向

```text
rambledesk-storage ───┐
rambledesk-mcp ───────┼──→ rambledesk-core
rambledesk-speech ────┘

rambledesk-cli ───────→ core + storage + mcp (+ speech)
rambledesk-desktop ───→ core + storage + mcp + speech + Tauri
```

适配 crate 之间默认不得互相依赖。跨适配编排由 `core` 的 ports/use cases 和
最终 composition root 完成。

### 2.3 拆 crate 判据

满足以下至少一项才新增 crate：

1. 有 Tauri 壳之外的独立消费者；
2. 隔离明显的原生/重依赖；
3. 有独立发布、测试或变更节奏。

普通页面、单个 provider 或一组 DTO 不因“看起来整齐”而单独拆包。

### 2.4 前后端合同

Rust 是领域 DTO 的事实来源。M0 选择并锁定一种 TypeScript 生成方式，生成文件
提交到 `apps/desktop/src/lib/generated/`，CI 检查无漂移。前端不得手写第二套
Request status 枚举。

### 2.5 与 Kotone 的关系

RambleDesk 借鉴 Kotone 已验证的 workspace 结构和 ports/adapters 依赖方向，但
不是 Kotone 的子项目，也不对其 crate 建立 sibling path dependency。具体迁移
边界见 [KOTONE_REUSE.md](KOTONE_REUSE.md)。

## 3. 数据库基线

首个 migration 至少包含：

### `projects`

- `id`
- `name`
- `root_path`
- `root_path_canonical`
- `created_at`
- `updated_at`

### `agent_sessions`

- `id`
- `project_id`
- `agent`
- `external_session_id`
- `ended_at`
- `created_at`
- 唯一键：`project_id, agent, external_session_id`

### `feedback_requests`

- `id`
- `session_id`
- `what_happened`
- `status`
- `revision`
- `input_hash`
- `created_at`
- `started_at`
- `completed_at`
- `cancelled_at`
- `cancel_reason`

### `request_actions`

- `request_id`
- `action_id`
- `position`
- `instruction`
- 唯一键：`request_id, action_id`

### `drafts`

- `request_id`
- `body_markdown`
- `revision`
- `updated_at`

### `attachments`

- `id`
- `request_id`
- `draft_path`
- `published_path`
- `media_type`
- `sha256`
- `position`
- `created_at`

### `invocation_attempts`

- `id`
- `request_id`
- `transport_request_id`
- `execution_mode`
- `status`
- `opened_at`
- `closed_at`
- `error_code`

### `completion_notifications`

- `id`
- `session_id`
- `summary`
- `input_hash`
- `created_at`
- 唯一键：`session_id, input_hash`

### `feedback_results`

- `request_id`
- `package_uri`
- `directory_path`
- `markdown_path`
- `manifest_path`
- `manifest_sha256`
- `published_at`
- 唯一键：`request_id`

### `outbox_events`

- `id`
- `event_type`
- `aggregate_id`
- `payload_json`
- `created_at`
- `delivered_at`
- `attempt_count`
- `last_error_code`

数据库约束必须阻止非法终态回退；application transaction 负责跨表不变量。

## 4. Tauri 与进程模型

- 桌面进程是唯一写入者；
- 启动顺序：打开数据库并迁移 → 恢复未结束请求 → 启动 MCP → 初始化 UI/托盘；
- MCP 和 Tauri commands 调用同一 application services；
- Rust 通过 Tauri events 通知 UI 状态变化，UI 重新查询事实状态；
- 事件只用于唤醒，不承载唯一数据；
- 正常退出停止接受新 MCP 调用，等待短事务结束后关闭；
- 非正常退出后依靠 SQLite 和 draft 文件恢复，不执行 `waiting → interrupted`。

## 5. 里程碑

### M0：技术与协议尖峰

状态：**2026-07-29 已验收**。目标是消除无法开工的兼容风险，不做产品 UI。

交付：

- Tauri 空壳可启动；
- Rust 进程内 `/mcp` loopback 服务；
- bearer token、Host/Origin 验证；
- 用 MCP Inspector 调用一个只读 health tool；
- 分别记录 Codex 和 Claude Code 的：
  - 可连接 transport；
  - 协议版本；
  - 自定义 header 配置；
  - Tasks 支持；
  - 普通工具调用超时/取消行为；
- 锁定 `rmcp`、Tauri 和 Rust toolchain 版本；
- 将兼容结果写入 `docs/COMPATIBILITY.md`。

实测结果见 [COMPATIBILITY.md](COMPATIBILITY.md)。首发采用 polling：
`request_feedback` 快速返回 durable request，Agent 使用 `get_feedback` 查询。
Tasks 保留为双方显式声明支持后的增强路径。

验收门：

- 至少 MCP Inspector 和一个目标 Agent 能稳定调用；
- 不依赖公网服务；
- 未授权请求被拒绝；
- 已选定 Tasks 或 polling 的首发路径。

### M1：纯文本纵向闭环

目标：第一次真实完成“Agent 请求 → 人提交 → Agent 取回”。

交付：

- SQLite migrations；
- `request_feedback`、`get_feedback`、`cancel_feedback`；
- Inbox 和单请求工作区；
- 文本 Draft 自动保存；
- 提交发布 `feedback.md + manifest.json`；
- 系统通知；
- 重启恢复；
- 协议与状态机自动化测试。

验收门：

1. Agent 创建请求；
2. 请求在 UI 出现；
3. Operator 输入文本并提交；
4. Agent 取得完成结果和有效文件路径；
5. 在以上任意非事务中间点强制退出，重启后不丢请求或草稿；
6. 相同 `request_id` 重试不产生重复记录或目录。

### M2：截图、历史与可用性

交付：

- 粘贴、拖放和文件选择添加图片；
- 附件哈希、排序、删除和相对路径引用；
- Session/历史页面；
- `list_feedback_requests`；
- 托盘和待处理角标；
- 设置页复制 MCP 配置。

系统级跨应用截图可在 M2 后半段加入；先确保导入已有截图可靠。

### M3：语音 Ramble

交付前必须先写语音 ADR，明确：

- 录音采集方案和跨平台权限；
- 本地或云端转写 provider；
- 原始音频保留策略；
- 隐私提示；
- 失败时不影响文字和截图提交。

首版语音必须是可选输入方式，而不是提交依赖。

### M4：发行准备

- macOS / Windows / Linux 安装验证；
- 签名、自动更新与崩溃日志策略；
- 无障碍和键盘路径；
- 数据导出/删除；
- 协议兼容回归矩阵；
- README 安装和 Agent 配置文档。

## 6. M0 完成清单

- [x] 以 `apps/ + crates/` 初始化 Cargo/pnpm workspace；
- [x] 初始化 `apps/desktop` 的 Tauri 2 + Svelte 5/TypeScript/Vite 薄壳；
- [x] 建立 core/storage/mcp/cli 边界和单向依赖；
- [x] 接入官方 Rust MCP SDK，由 CLI 和桌面壳共同装配 `/mcp`；
- [x] 实现 256-bit 本地令牌、权限受限文件、Host/Origin 校验；
- [x] 编写并执行 MCP Inspector smoke test；
- [x] 验证 Claude Code，记录 Codex 本机安装阻塞；
- [x] 锁定 SDK、toolchain、TypeScript DTO 生成和 polling 首发模式；
- [ ] M1 再引入 SQLite、领域状态机和反馈业务工具。

不要在 M0 中实现语音、截图、完整导航或视觉系统。

## 7. Definition of Ready

一个开发工作项只有满足以下条件才可进入实现：

- 对应宪章原则明确；
- 输入、输出和错误已定义；
- 状态迁移及幂等性已定义；
- 数据所有者和写入边界明确；
- 有可自动验证的验收条件；
- 不依赖尚未验证的客户端行为。

## 8. Definition of Done

- 实现和文档一致；
- 正常、重试、取消、断线和重启路径有测试；
- 不记录敏感正文或 token；
- 格式化、lint、类型检查和测试通过；
- 没有把临时兼容逻辑放进领域层；
- 用户可观察失败原因，并有恢复动作。

## 9. 当前判断

M0 已通过自动化与真实客户端验收，项目具备开始 M1 的工程条件。M1 应从
core 状态机、SQLite migration 和 polling 合同开始，不提前引入语音、截图或
Tasks 专用业务分支。
