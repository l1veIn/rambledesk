# ADR 001：采用 apps/ + crates/ 产品 monorepo

- 状态：Accepted
- 日期：2026-07-29
- 参考：Kotone 的 workspace 拆分与实际运行结果

## 上下文

RambleDesk 同时包含桌面 UI、本地 MCP server、持久化、反馈包发布、未来语音
引擎和自动化测试入口。把这些全部放进 Tauri 的 `src-tauri` 会让：

- 领域模型依赖桌面框架；
- MCP 无法脱离 GUI 做兼容测试；
- SQLite、MCP SDK、cpal 和 STT 原生库互相拖慢编译；
- 无法用 CLI 对恢复、协议和 WAV fixture 做无人值守测试；
- 将来增加第二个 app 时再次搬迁。

Kotone 已经从单 `src-tauri` crate 迁移到 `apps/desktop + crates/*`，验证了
Tauri canonical 目录、Cargo workspace 和 pnpm workspace 可以共存。

## 决策

采用：

```text
apps/desktop/src-tauri
crates/rambledesk-core
crates/rambledesk-storage
crates/rambledesk-mcp
crates/rambledesk-speech
crates/rambledesk-cli
```

根目录是 workspace，不是某一个应用的项目根。

依赖必须指向 core；Tauri 壳与 CLI 是 composition roots。业务状态不得存放在
MCP adapter、Tauri command handler 或前端 store 中。

## Crate 拆分判据

只有满足独立消费者、重依赖隔离或独立变更节奏之一才拆 crate。

speech 在 M3 前可以只有占位 manifest 或暂不加入实现，但名称和依赖位置现在固定，
避免语音重新进入桌面壳。

## 被否决方案

### 单一 Tauri crate

初始文件少，但把产品边界和框架边界合并；MCP Inspector、headless server 和
协议集成测试都必须拉起 GUI。

### 所有能力各拆一个 crate

会产生过多 DTO crate/provider crate 和依赖噪音。首版 storage 同时承担 SQLite、
draft 与 package publisher；有独立复用或编译压力后再拆。

### 直接依赖 `../kotone/crates/*`

本机 sibling path 不可发布、不可复现，并把两个产品的版本和领域模型耦合。允许
审计后迁移代码，不允许建立这种依赖。

## 后果

正向：

- core 可快速编译和跨平台测试；
- MCP 可由 CLI 与桌面共同托管；
- 语音重依赖不污染 M0–M2；
- Tauri 保持薄壳；
- 根命令统一，只有一个 Cargo.lock 和 pnpm lock。

代价：

- 初始 manifest 较多；
- 需要显式 composition root；
- DTO 需生成到 TypeScript，CI 要检查漂移；
- 集成测试要放在能看到具体适配实现的 crate 或顶层 tests 中。
