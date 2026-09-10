# RambleDesk 架构

本文描述当前实现的运行边界和所有权；词汇以[术语表](TERMINOLOGY.md)为准。用户旅程见[产品文档](PRODUCT.md)，构建方法见[开发指南](DEVELOPMENT.md)，平台支持与待验项见[质量清单](quality/README.md)。历史 ADR 记录决策，不能把其中的目标状态直接当成当前支持承诺。

## 运行时拓扑

```text
外部 Host Adapters ── MCP / Local JSON ── Local Integration Server ─┐
Desktop Client ────── Tauri Application Transport ─────────────────┤
Web Client ────────── Web Access HTTP + WebSocket ─────────────────┤
                                                                  ▼
                         Backend Runtime / ApplicationCommandFacade
                         core + storage + config + session controllers
                           └─ AcpSessionDriver → owned ACP Instance → Agent Backend
                                                └─ feedback command → private IPC relay
                                                     → /agent-feedback/* → application

Desktop / Browser Capability → 当前客户端 Platform Plugin
                             → SpeechEvent / Attachment Candidate → TipTap Ramble Core
```

`apps/desktop` 是完整产品的 composition root。每个 desktop 进程创建一份 Backend Runtime/application facade，由 Tauri state、该进程的 Local Integration Server 和可选 Web Access 共用；不是跨进程全局单例。

Local Integration Server 的独立 loopback listener 承载 `/api`、`/mcp`、托管会话专用 `/agent-feedback/*`，并保留旧 `/mcp-managed` 兼容入口。它不提供 Web 静态资源、Workbench application routes 或 WebSocket；MCP SSE 不是 Web Client 事件流。

Web Access 默认关闭，使用另一 listener、credential、auth domain、route set 和启停生命周期；默认 `127.0.0.1:37643`。它复用同一 application module，停止它不停止 Backend Runtime 或 Local Integration Server。设置支持下次启动使用的新端口及可选随 Desktop 启动，均不改变 loopback 边界。

CLI 的 `serve` 仍可组装 SQLite 与 Local Integration Server，供开发诊断；它不是完整的 headless Web/Agent 产品。应用内置 `feedback` 命令只是已有托管运行时的客户端，不启动 UI、后端或业务数据库。

## 模块与依赖

| 位置 | 当前职责与边界 |
| --- | --- |
| `crates/rambledesk-core` | Feedback、workspace、Agent Session、Activity、Delivery、Recovery、Deletion 的 DTO、application use cases 与 ports。不依赖 HTTP/JSON/MCP/ACP SDK、Tauri、SQLite、宿主安装或文件路径推断。 |
| `crates/rambledesk-storage` | SQLite、migrations、草稿与附件元数据、会话事实、发布对账及所属文件清理；依赖 core，不持有宿主协议。 |
| `crates/rambledesk-acp` | 官方 ACP SDK、stdio driver、能力协商、权限回调、实例与进程树生命周期。依赖 core 和 feedback-client；不持有 SQLite、HTTP 路由或 Tauri UI，也不承接客户端文件/终端执行能力。 |
| `crates/rambledesk-feedback-client` | 桌面/CLI 共用的反馈命令、输入验证、本地 IPC relay/client 和 scoped HTTP 客户端；依赖 core。relay 由 ACP 实例拥有，不拥有产品会话或推理。 |
| `crates/rambledesk-local-server` | Local Integration 与独立 Web Access 的 listener、路由、认证、安全策略、静态资源与事件服务；依赖 core、mcp，不实现领域规则。 |
| `crates/rambledesk-mcp` | Generic MCP schema、handlers、instructions、结果映射及客户端检测/安装执行引擎；依赖 core、hosts，不持有 listener 或 JSON API。 |
| `crates/rambledesk-hosts` | Host Profile、程序/配置知识、标签、默认适配器、continuation strategy；依赖 core，不执行安装或宿主协议。 |
| `crates/rambledesk-speech` | Desktop 本地语音实现与资源；不成为跨客户端音频服务。 |
| `packages/pi-rambledesk` / `packages/dsh-rambledesk` | 外部宿主原生反馈适配器，通过本地 JSON API 等待和恢复；不属于 ACP Client 或 Cargo 领域层。 |
| `apps/desktop/src-tauri` | 装配上述模块、Tauri commands/events、窗口、托盘、更新、权限及原生能力。 |
| `apps/desktop/src` | 共享 Workbench Client、Tauri/HTTP Application Transport、设备能力与客户端状态；共享 UI 不直接依赖 Tauri 细节。 |
| `crates/rambledesk-cli` | `serve` / `smoke` / `self-test` 诊断及共用 feedback 命令分派。 |
| `web/` | 独立官网，不是 Web Access 的业务客户端。 |

Cargo 依赖的精确清单由 [check-terminology.mjs](../scripts/check-terminology.mjs) 固化；前端组合根、模块依赖和预览注入约束见[开发指南](DEVELOPMENT.md#前端边界与测试)。跨模块编排留在组合根，不能让 core 反向依赖实现库。

## 业务事实与客户端投影

| 事实 | 所有者 |
| --- | --- |
| Request、结构化 Draft、revision、附件元数据 | Backend Runtime / SQLite |
| 不可变反馈包 | 发布目录与 SQLite 结果记录共同对账 |
| Session、AgentConfig、Activity、Delivery、Recovery、Deletion intent | Backend Runtime；持久事实与实时连接投影分开 |
| 当前连接、执行状态、上下文 `used/size` | 当前运行实例；不把历史 connected 或累计 token 当作当前事实 |
| 打开的 view、顺序、active view、pane 尺寸 | 每个客户端的 workspace snapshot，不保存第二份 Draft 正文 |
| phone/tablet/desktop mode、抽屉和 rail 布局 | 客户端 shell layout owner；布局不创造后端业务状态 |
| Editor selection、未保存编辑、当前设备录音与预览资源 | 当前 Workbench Client 的对应 owner |

事件只提示重新查询。通知、UI store、浏览器缓存和路径 hint 都不是额外业务事实源。响应式抽屉、焦点与安全区约束见[响应式工作台](WEB_ACCESS_SUPPORT_MATRIX.md)；物理手机能力与 CSS 视口适配分别验收。

## Application Transport 与恢复合同

Interface 暴露 typed command/query、变化订阅、ready barrier 与 capability manifest。Tauri IPC 和 Web HTTP + WebSocket 调用同一 application facade；manifest 只报告能力可用性，不执行设备操作。

Web Transport 保留以下顺序与失效合同：

1. 每次 Backend Runtime 启动生成唯一、进程内稳定的 `runtime_generation`。WebSocket ready 带 generation；客户端只接受当前 connection epoch 的 ready，替换 active generation 并使旧投影和请求失效。
2. HTTP snapshot/query 是带 generation/resource revision 的可丢弃投影；WebSocket 只传 readiness 与轻量 invalidation，不传 canonical Request/Draft/Package。
3. 修改状态的 command 等待当前 ready；envelope 的 `expected_runtime_generation` 映射到 `X-RambleDesk-Runtime-Generation`。服务端在领域副作用之前比较 command、session 与当前 generation，不匹配返回 HTTP `409` / typed `stale_generation`。Draft CAS 不能替代此检查。
4. 旧 socket、epoch、generation 的响应一律丢弃；同代 response revision 不能倒退。fetch 期间收到更高 revision 的 invalidation，完成后仍须再次 refetch。
5. 断线、撤销或连接失败结束对应等待，不能把保存永远挂在一个已失效的 ready Promise 上。恢复时重新认证/连接，确认 ready 后 refetch；不静默重放结果不明的 mutation。
6. 认证 gate 不卸载 dirty Editor。重新认证后保留本地编辑并按 revision/CAS 保存；冲突显示失败并保留原文，不自动合并、覆盖或丢弃。没有 sequence replay、ring buffer 或 multiplex 协议。

## Feedback Draft 与输入所有权

每个 Workbench Client instance 最多一个可编辑 `RichFeedbackEditor`；不建立 per-request/hidden Editor，也不让 session 持有 Editor handle。SQLite 中版本化 TipTap `document_json` 是保存真源；`body_markdown` 是同一次保存从该文档生成的投影。历史 Markdown 迁移、导出和展示不能成为第二套正文状态。见 [ADR 004](adr/004-single-editor-structured-draft.md)与修订其客户端作用域的 [ADR 005](adr/005-shared-workbench-transport-capabilities.md)。

当前请求通过 Editor transaction 修改，后台 Active Ramble 对固定 request owner 使用 JSON transformation、串行队列与 CAS。Draft controller 合并并发保存等待，等待中的调用共享成功/失败结果；成功后排空保存期间的新编辑，失败不自动重复提交。发布前等待最新 snapshot/revision，旧请求结果不能污染新请求。

多个客户端可以编辑同一 Draft，但只能由 Backend Runtime revision/CAS 仲裁。跨客户端 refetch、自动保存回执和导航不得静默清空未保存编辑；冲突保留正文和可见错误。接受当前保存的投影也不能无故重置编辑器 Undo 历史。

设备输入位于 Application Transport 之外。Platform Plugin 组合本地能力、权限与资源，只返回 `SpeechEvent` 或 `AttachmentCandidate`；实时 PCM、识别 session、模型与权限不进入后端 Transport。浏览器选文件是读取客户端文件，不是选择服务器 `cwd`；不可用能力明确禁用。Browser ASR 当前是本地 pilot，浏览器屏幕采集尚未交付，具体设备支持见[矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)。

Action 用带 `actionId` / `actionIndex` 的标准 Blockquote；语音段有稳定 `speechSegmentId` 和 `pending` / `cleaned` 状态。当前 Editor 支持手动 Tidy 与数量阈值 Auto Tidy；待确认语音也可启用逐段自动整理，默认关闭并仍等待人类确认。两条路径都保留编辑/整理/写入占用、owner 与原文校验，已整理语音写入时传递 `cleaned` metadata；不整理后台文档。Tidy 与 Cooking 独立配置，规则见[术语表](TERMINOLOGY.md#tidy-与-cooking)。快捷键只发送语义事件，不持有 Editor。

附件候选先由当前目标 owner 验证和持久化，成功后才成为 Draft 引用。过期目标、发布后的写入及丢失响应不能借本地乐观状态伪装成功；预览资源由实际使用者持有，在最后一个使用者结束后释放，不能以面板切换代替整个生命周期。

## 会话与客户端生命周期

托管路径中一个 RambleDesk Session 对应一个 Agent Session，每个会话独占一个 ACP Instance；实例可包含 bridge 和子进程。配置、权限、输入、反馈、停止、恢复和删除都使用共享 application 合同。协议握手不证明模型或 Agent 执行工具可用，支持版本见 [ACP 指南](ACP_MANAGED_SESSIONS.md)。

新会话先创建内部持久 `prepared` 记录，准备不发 prompt；首条真实用户消息被接受并落盘时转为 `active`。导航不展示 prepared；准备失败可用原 ID 重试，关闭/变更/重启回收准备资源，不能通过草稿 discard 删除 active 会话。前端草稿 Tab 原位转为正式 Tab，只恢复用户输入，不恢复准备凭据或身份。

正式会话的 Ramble / Agent 是平级视图；Ramble 预览按可信 `managed_session_id` 查询状态，不为状态卡加载全部 Agent 历史。Agent 时间线按 turn 展示，折叠过程延迟挂载；上下文占用只来自当前实例真实 usage 更新。

关闭正式 Tab、刷新/关闭浏览器或 Transport 断线不隐式 submit、cancel、archive Request，也不停止 Backend Runtime 中的 Agent Session。浏览器关闭会结束该客户端的录音、Editor 和设备资源；它不能保证本地 Active Ramble 跨页面存活。未保存或保存中的 Draft 使用浏览器标准 `beforeunload` 确认，人类选择离开后按浏览器行为结束；不在 unload 尝试保证异步保存。桌面关闭窗口/托盘合同与浏览器离页处理分开。

重启不信任旧 connected 状态；未完成轮次由检查点记为中断。存在 `remote_session_id` 时恢复必须 resume/load 原会话，失败不静默新建。删除先持久化 intent，再停止并清理所属资源，失败可重试；文件清理与发布共用锁，旧 publication plan 不能复活已删除数据。

## 发布与托管 continuation

```text
Submit(request_id, expected_revision)
  → 检查非终态与 revision
  → 写入临时包、flush 与 hash
  → 原子发布
  → 持久化终态与包元数据；托管提交同时入 outbox
  → 通知 waiters / 对应 continuation
```

包已发布但数据库更新失败时，启动恢复按 manifest/request_id 对账，不生成第二份包。提交、批准与取消都必须幂等；批准最终总结可以完成请求而不发布包。外部协议的状态与包合同见 [PROTOCOL](PROTOCOL.md)。

生产 ACP 使用应用内置 `feedback request/get/recover/skip`。每次运行绑定会话专用 credential，停止/删除时撤销；它不能访问 Generic MCP 或其他会话。命令子进程只持有私有 IPC 能力，HTTP credential 留在运行时 relay 内存。入口注入可信会话归属，不注入 MCP server 或 Pi 扩展，不修改用户全局 Skills。

每条用户/续接 prompt 前置工作流上下文，但用户 Activity 不保存注入说明。Agent 工具和 bridge 环境传播仍须实测。托管提交/批准与 outbox 入队原子提交，worker 仅在原会话可接收输入时续接；取消不创建新投递。旧取消投递保留历史并停止派发。delivered 不表示任务完成，uncertain 不自动重试，只由人类显式处理。

## 数据布局

默认业务数据与可迁移资料库分开：

```text
<local-data>/RambleDesk/
├── state/feedback.sqlite3
├── settings.json
└── library/                    # 可由资料库设置或 RAMBLEDESK_LIBRARY_DIR 改变
    ├── drafts/<request-id>/
    └── feedback/<timestamp>-<request-id>/
        ├── feedback.md
        ├── uncooked.md
        ├── manifest.json
        └── attachments/
```

其他模型/缓存由各能力模块管理。数据库、资料库与 Local Integration token 的开发覆盖变量见[开发指南](DEVELOPMENT.md#本地运行)。Web credential 使用独立的 Tauri `app_local_data_dir()`，按应用 identifier 隔离，不属于可迁移 library、通用配置或业务 SQLite。

## 安全边界

### Local Integration Server

只绑定 loopback，使用持久 256-bit hex bearer、constant-time 比较和 Host/Origin guard。Host 只允许 `127.0.0.1` / `localhost`；无 Origin 只按非浏览器本机调用放行，有 Origin 必须 exact-match allowlist。通用 body limit 为 96 MiB。Unix token 文件 `0600`，非 Unix 不宣称等价 ACL；不能把它直接复用为 Web credential。关联字段和 context hint 均不承担认证。

### Web Access

- 独立 listener 使用 same-origin 静态资源、HTTP 与 WebSocket，不开放宽泛 CORS。两类 listener 共用安全 policy/primitives，但不共享 credential 或生命周期。静态请求严格检查 Host，bootstrap、受保护 API 和 WebSocket 还检查 exact Origin。
- 独立 256-bit durable token 由 Desktop 的 Web security module 持有；不返回通用 UI，仅设置页的原生 clipboard command 可复制。macOS/Linux 使用 `app_local_data_dir()/auth/web-access.token`；Windows 保留 Credential Manager。
- Unix `auth` 目录 `0700`、文件 `0600`，原子创建/轮换并拒绝 symlink、非普通文件、错误归属和损坏内容。自动加载失败不生成替代 token；显式 Refresh token / Confirm 可修复损坏的自有普通文件，不绕过路径与归属检查。
- 旧 macOS Keychain / Linux Secret Service 条目不读取、不自动迁移、不删除。升级首次启用创建新 token，需重新复制认证；后续同一运行时有效 cookie 仍可恢复。文件权限不等于加密，也不宣称排除 OS 备份。
- durable token 经 same-origin `POST /api/auth/session` bootstrap 换取受限 session。响应写入 `HttpOnly; SameSite=Strict; Path=/api/auth/session` cookie，刷新/新标签页/浏览器重启可凭有效 cookie 恢复；HTTP 使用 session bearer。
- 内存 session 的 idle/absolute TTL 为 30 分钟/12 小时；浏览器 session 为 30 天/180 天。HTTP 请求或新 WebSocket 认证刷新 idle，不延长 absolute。停止服务、Runtime 重启或轮换 token 撤销 session；stop/rotate 关闭已有 socket。
- session token 仅在服务端、当前 JavaScript 内存及上述 cookie；token 不进入 URL、sessionStorage、localStorage、IndexedDB、SQLite、日志、诊断或应用备份/导出/反馈包。bootstrap 成功清空 durable token 输入。
- WebSocket 同时提供 `rambledesk-events` 与 `rambledesk-session.<base64url-no-pad-session-token>` subprotocol；只回显前者，不用 query token，不记录 credential-bearing protocol。
- HTML `no-store`，仅 build manifest 标记的 fingerprinted asset 使用 immutable cache。SPA fallback 不覆盖 `/api/**`、`/assets/**`、扩展名路径、traversal 或 encoded separator。
- 每 listener 每分钟最多 8 次 bootstrap、同时 16 个 application HTTP request / 8 个 socket、最多 32 个 session。超限返回 `429` 或 `503`；multipart 使用 20 MiB 附件上限加 64 KiB metadata allowance，其余 JSON 同样有界。
- 服务端保存 session credential 的固定长度 SHA-256 hash，完整 constant-time 扫描。认证是 request admission lease：已入场 mutation 在随后 stop/expiry 时仍返回真实结果，不自动重放；撤销限制后续请求和 socket。

LAN、TLS、独立 Web deployment/headless Backend Runtime 和浏览器屏幕采集均不在当前支持面。未来扩展须重新定义并验证设备暴露、认证与文件边界，不能从现有浏览器 API 或 CLI 推导支持。存储选择、自动化和真实平台操作的证据分别见 [Web Access 支持矩阵](WEB_ACCESS_SUPPORT_MATRIX.md)、[ADR 005](adr/005-shared-workbench-transport-capabilities.md)及[质量清单](quality/README.md)。
