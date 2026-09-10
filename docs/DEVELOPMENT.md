# RambleDesk 开发指南

本文维护开发入口、检查和交付方法。职责与依赖见[架构](ARCHITECTURE.md)，词汇见[术语表](TERMINOLOGY.md)，实际验收及未验项见[质量清单](quality/README.md)。

## 技术与代码入口

| 区域 | 技术 / 入口 |
| --- | --- |
| 桌面与共享工作台 | Tauri 2、Svelte 5、TypeScript、Vite；`apps/desktop` |
| 前端组件 | shadcn-svelte、Tailwind CSS；`apps/desktop/src/lib` |
| 后端 | Rust、Tokio；`crates/rambledesk-core` 的 application contract |
| 持久化 | SQLite、显式 [migrations](../crates/rambledesk-storage/migrations/) 与不可变反馈包 |
| Agent 与外部接入 | ACP SDK、`rambledesk-acp`、`rambledesk-feedback-client`；Generic MCP 的 `rmcp`；Pi/dsh 本地 JSON API |
| 官网 | Astro；`web/`，与工作台构建分开 |
| 工具链 | Cargo、pnpm；版本约束以根配置及各 package manifest 为准 |

ID 使用 UUIDv7，时间在边界使用 UTC / RFC 3339。默认日志只记录元数据，不记录正文、token 或附件内容。SQLite schema 以迁移为准，修改需考虑已有数据升级和中断恢复，不在文档维护第二份字段清单。

## 本地运行

```bash
pnpm install
pnpm dev:web
```

`dev:web` 启动前端开发入口，不会建立独立 Web Backend Runtime。开发桌面应用使用 `pnpm dev`；浏览器连接真实后端时使用 Desktop 的 Web Access，支持边界见[矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)。

隔离数据库、附件资料库与 Local Integration Server token：

```bash
RAMBLEDESK_DATABASE_FILE=/absolute/test/state/feedback.sqlite3 \
RAMBLEDESK_LIBRARY_DIR=/absolute/test/library \
RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE=/absolute/test/local-server.token \
RAMBLEDESK_LOCAL_SERVER_PORT=0 \
pnpm dev
```

这些变量不隔离所有桌面偏好、模型、应用 identifier 或 Web credential。不要将其当成完整产品验收沙箱；真实 HTTP/SQLite 与浏览器复验使用[隔离反馈夹具](quality/FEEDBACK_ACCEPTANCE.md)。通知、麦克风和屏幕录制权限必须由明确的人类操作触发，自动化测试不得主动弹出系统权限框。

`cargo run -p rambledesk-cli -- self-test` 使用临时数据库验证 MCP。CLI 也有 `serve` / `smoke` 开发诊断命令；`serve` 组装存储和 Local Integration Server，不提供完整 Desktop/Web/ACP 产品。应用内置的 `feedback request/get/recover/skip` 则调用已有托管运行时，不创建服务器或打开业务数据库。

## 前端边界与测试

共享工作台从 `App.svelte` / `BrowserWorkbenchRoot.svelte` 组合，经 `lib/application` 访问后端，经 `lib/capabilities` 访问当前设备。主要责任模块为 `agents`、`workbench`、`workspace`、`editor`、`speech`、`shell`、`settings`；`lib/generated` 是生成合同，只读。

[frontendBoundaries.test.ts](../apps/desktop/src/lib/architecture/frontendBoundaries.test.ts) 固化依赖方向：

- `lib/domain` 只依赖 `lib/generated` 与 `lib/feedback` 合同桶。
- 视图层 `lib/workspace` 不反向依赖 `lib/workbench`。
- 跨域 `ALLOWED_EDGES` 只减不增；新增边失败，消失的边须从清单删除。
- 组合根和 `dev/` 不能被 `lib/` 反向引用，显式列出的入口除外；共享 UI 不直接操作 Tauri。

纯逻辑测试留在 owner 模块。静态呈现可用 `svelte/server`；事件、焦点、异步保存与清理须在需要时用真实挂载的 jsdom 测试（`// @vitest-environment jsdom`），隔离 Tauri API。原生窗口、设备、权限和浏览器手势不能由 DOM 测试代替。

`?preview=fixtures` 在 `main.ts` 构造 [previewApplicationTransport](../apps/desktop/src/lib/preview/previewApplicationTransport.ts)，经 `createWorkbenchComposition({ previewTransport })` 注入；控制器仍只依赖 Application Transport。预览 workspace snapshot 通过 `createWorkspaceShellSession({ snapshots })` 注入，不写真实 `rambledesk.ui-state`。完整 HTTP/SQLite 夹具与内存预览的证据范围不同。

## 前后端合同

Rust DTO 是合同源；修改导出类型后运行：

```bash
pnpm contracts:generate
pnpm contracts:check
```

生成文件位于 `apps/desktop/src/lib/generated/`；不得手写第二套 Request 状态枚举或字段别名。Cargo 依赖由 [check-terminology.mjs](../scripts/check-terminology.mjs) 检查，前端边和文件规模也有门禁；修改实现时修正实际边界，不用放宽清单隐藏新增耦合。

## 验证与完成标准

按改动风险验证正常、重试、取消、断线、重启和资源释放。实现、协议、术语和 UI 文案应一致；不得留下未说明的别名或兼容分支，既有明确兼容合同不能为“零残留”口号而静默删除。

工作台与 Rust 的完整工程门禁：

```bash
pnpm format:check
pnpm clippy
pnpm test:rust
pnpm check
pnpm test
pnpm build:web
pnpm contracts:check
pnpm check:terminology
pnpm check:rust-size
pnpm check:frontend-size
pnpm test:pi
pnpm test:dsh
pnpm mcp:self-test
pnpm mcp:inspector-smoke
```

`test:rust` 包含独立 `target/desktop` 的桌面测试；`build:web` 是工作台构建。官网修改另运行 `pnpm -C web check` 与 `pnpm -C web build`。涉及打包、平台行为或真实输入时，在相应平台记录构建标识、步骤、实际结果和未验项；本机测试不等于远端 CI 或其他平台通过。

## 发布与更新说明

- 发布前在 [CHANGELOG.md](CHANGELOG.md) 顶部增加 `## vX.Y.Z` 条目。保持纯文本、英文在前中文摘要在后，更新弹窗不解析 Markdown。
- `release.yml` 的 checksums 阶段将条目写入 GitHub Release 正文与 `latest.json` 的 `notes`；缺少条目会警告并回退到通用说明。
- 手动生成：`node scripts/release-notes.mjs --tag vX.Y.Z`。
- 修正已发布说明时，同步更新 CHANGELOG、Release 正文和 updater metadata，使用 `scripts/release-notes.mjs` 与 `scripts/patch-updater-notes.mjs`，避免两处不一致。
- 版本、产物、平台验收和发布流程见[发布检查](RELEASE_CHECKLIST.md)。Git 合并、发布产物和完成全部产品验收分别判断。
