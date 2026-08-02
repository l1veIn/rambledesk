# ADR 001：采用 apps/ + crates/ 产品 monorepo

- 状态：Accepted
- 日期：2026-07-29
- 修订：2026-08-02

## 上下文

RambleDesk 同时包含桌面工作台、本地服务、持久化、适配器、反馈包发布、语音能力
和 headless 验证入口。把这些能力全部放进 Tauri crate 会把 application contract、
transport、SQLite 和窗口生命周期耦合在一起。

仓库需要满足：

- core 可以脱离 GUI 和 transport 测试；
- 本地服务可以由 desktop 与 CLI 装配；
- 通用 MCP 适配器使用 MCP 作为 transport；
- Pi Native Adapter 可以独立发布和测试；
- SQLite 与反馈包发布不依赖 UI；
- desktop 保持 composition root，而不是业务事实来源。

## 决策

采用：

```text
apps/desktop/src-tauri
crates/rambledesk-core
crates/rambledesk-storage
crates/rambledesk-local-server
crates/rambledesk-mcp
crates/rambledesk-hosts
crates/rambledesk-speech
crates/rambledesk-cli
packages/pi-rambledesk
```

职责：

- `rambledesk-core`：application contract、状态机、DTO 和 ports；
- `rambledesk-storage`：SQLite、draft/attachment metadata、宿主会话关联、反馈包发布；
- `rambledesk-local-server`：loopback listener、auth、guards、JSON API 和 route mounting；
- `rambledesk-mcp`：Generic MCP Adapter tool schema、handler、instructions 和结果映射；
- `rambledesk-hosts`：Host Profiles 与 continuation strategy contract；
- `pi-rambledesk`：Pi Native Adapter；
- desktop 与 CLI：composition roots。

根目录是 workspace。RambleDesk 不建立源码 checkout 的产品对象，也不要求调用方
提供源码目录。

## 依赖规则

- adapter、storage 和 local server 依赖 core contract；
- local server 可以装配 MCP adapter；
- MCP adapter 不依赖 local server；
- desktop 可以装配所有桌面所需 crate；
- UI 不直接访问 SQLite；
- transport 与 Tauri commands 不实现领域规则；
- package 间不得用 DTO alias 维持旧字段兼容。

## 被否决方案

### 单一 Tauri crate

会让协议、持久化和自动化测试必须拉起 GUI，也会把业务生命周期绑定到窗口生命周期。

### 把本地 listener 放在通用 MCP 适配器

会把 listener、auth 和 JSON API 误归为 MCP 能力，阻止其他原生适配器复用本地服务。

### 在 core 中持有宿主与桌面逻辑

会让 application contract 依赖安装方式、Host Profile、continuation 和窗口事件。

### 过度拆分 DTO crate

会增加 manifest、版本和依赖噪音。只有具备独立消费者、重依赖隔离或独立发布节奏
时才拆 package。

## 后果

正向：

- application contract、transport、持久化和 UI 边界明确；
- Generic MCP Adapter 与 Pi Native Adapter 可独立演进；
- 本地安全策略只有一处实现；
- desktop 可保持薄装配层；
- headless 测试无需启动窗口。

代价：

- composition root 需要显式装配更多 crate；
- DTO 生成和合同漂移需要 CI 门禁；
- 跨 package 集成测试需要放在能看到具体实现的 crate 中；
- 目录和依赖规则需要持续通过架构与术语审计。
