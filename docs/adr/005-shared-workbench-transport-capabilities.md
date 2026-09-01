# ADR 005：共享 Workbench Client 的 Transport、Capability 与 Web 安全边界

- 状态：Accepted
- 日期：2026-09-01
- 术语源：[TERMINOLOGY.md](../TERMINOLOGY.md)

## Context

本文使用的 Workbench Client、Backend Runtime、Application Transport、Capability、Local
Integration Server 与 Web Access 均以唯一术语源为准；这里只记录它们之间的决策和后果。

RambleDesk 最初只有 Desktop Client。Svelte 工作台通过 Tauri commands/events 调用由 desktop
composition root 装配的 application；同一进程还启动一个供 Generic MCP 与 Pi Host Adapter 使用的
Local Integration Server。现已增加共享该工作台与 Backend Runtime 的 loopback Web Client；实现
不得复制业务规则、把 Desktop 强制改走 HTTP，或把浏览器能力伪装成原生能力。

Transport、设备 Capability 和 listener 生命周期若没有独立边界，会产生以下风险：

- Tauri 与 HTTP 各自形成业务规则和事实状态；
- Web Access 的启停意外停止 Host Adapter 或 Backend Runtime；
- 持久的本地集成 token 暴露给浏览器、URL、日志或持久 Web storage；
- WebSocket 事件被误当成事实源，断线后无法可靠恢复；
- Browser Capability 被错误描述为 global shortcut、系统截图或服务器文件系统能力。

## Decision

### 1. 一个 Backend Runtime，多种 Workbench Client

Backend Runtime 是 Request、Feedback Draft、Package、配置以及未来 Session Runtime / Timeline
的唯一业务事实来源。Desktop Client 和 Web Client 复用同一 Workbench Client，并通过
同一个 Application Transport Interface 调用同一 application Module。

Backend Runtime 是运行角色，不要求新增同名 crate。当前每个 desktop 进程只装配一份
`FeedbackApplication` 给 Tauri state 与该进程的 Local Integration Server 使用；本决策不把它
提升为跨进程全局单例，也不决定 headless composition root。

### 2. Application Transport 是 Interface，Tauri 与 HTTP + WebSocket 是 Implementation

Application Transport Interface 提供 typed command/query、变化订阅、ready barrier 与
capability manifest。Tauri IPC 是 Desktop Implementation；Web Access 的 HTTP + WebSocket 是 Web
Implementation。两者只做 DTO、错误和传输映射，不实现领域规则。

Backend Runtime 每次启动生成一个 opaque、进程生命周期内不变且跨启动唯一的
`runtime_generation`。WebSocket `ready` frame 携带该 generation；Client 只接受当前 local
connection epoch 的 ready，并原子替换 active generation、清空旧投影、取消或标记旧 in-flight
HTTP request 为 stale。确认 ready 并建立 active generation 后，才可发出会产生事件或修改状态的
command。typed command envelope 携带 `expected_runtime_generation`，Web HTTP Implementation
映射为 `X-RambleDesk-Runtime-Generation`；Web session record 也绑定签发时的 generation。服务端
在任何领域副作用前原子比较 session、command 与当前 generation，任一不匹配都返回 HTTP `409` /
typed `stale_generation`。Client 随后重新 bootstrap、连接并 refetch；resource revision/CAS 不能
替代 runtime generation 检查。

HTTP snapshot/query 是 Backend Runtime 事实的可丢弃投影，每份 snapshot 携带 generation 与相关
resource revision。WebSocket 只承载 readiness 与 invalidation，invalidation 携带同一 generation
下可比较的 resource key/revision。Client 只应用与 active ready generation 一致、且 revision 不低于
已应用投影的 response；来自旧 socket、旧 connection epoch 或旧 generation 的 frame/response
一律丢弃。fetch 期间收到更高 revision invalidation 时，当前 fetch 完成后必须再次 refetch。断线
或 stale-generation 后重新建立订阅、确认 ready，再 refetch 完整 snapshot。

首期不实现 sequence replay、ring buffer 或 multiplex protocol。MCP SSE 继续属于 MCP
transport，不复用为 Web Client event stream。

### 3. Capability 位于 Transport 之外

Native Capability 与 Browser Capability 使用独立 Interface/Implementation。Application
Transport 的 capability manifest 只报告可用性，不执行设备操作。

- Desktop Shell 可以提供全局快捷键、系统截图、原生录音、tray、updater 与 native dialog；
- 浏览器只能提供其 secure context、权限、用户手势和当前设备允许的能力；
- browser file picker 不代表 Backend Runtime 所在机器的 working directory；
- Workbench Client 必须呈现 capability unavailable/degraded，不能伪造桌面等价性。

`Adapter / 适配器` 继续只表示完整 host-facing integration。Application Transport 与 Capability
均称为 Interface/Implementation，不称为 Adapter。

### 4. 服务端事实与 client-local 状态分离

Request、Feedback Draft、Package 和 application configuration 属于 Backend Runtime。view
descriptor、tab 顺序、active view 与 pane 尺寸属于每个 Workbench Client 的 client-local
workspace snapshot；snapshot 不缓存 canonical Draft 正文。

每个 Workbench Client instance 最多挂载一个可编辑 `RichFeedbackEditor`，继续禁止 per-request 与 hidden
Editor。多个客户端并发写 Draft 时使用 Backend Runtime revision/CAS，冲突必须显式显示而不是
last-write-wins。submit/cancel 等终态 operation 必须幂等。

关闭 workspace Tab、关闭或刷新浏览器、Transport 断线都只结束 Client view/projection；这些
动作不得隐式 submit、cancel、archive Request，也不得停止后台 Active Ramble 或未来 Agent /
Session Runtime。终态与 runtime lifecycle 只能由可审计的显式 application command 改变。
Client 应持续 autosave；关闭 workspace view 继续使用 save gate，但不得依赖不可靠的 browser
`unload` 完成唯一一次保存或终态 mutation。重连/重开后从 Backend Runtime refetch 事实。

### 5. Platform Plugin 在输入设备本地处理媒体

本节由 [ADR 006](006-edge-media-plugins-and-tiptap-ramble-core.md) 修订。Audio Source 与 Speech
Engine 可以在单个平台插件内部保持分离，但它们不构成跨客户端网络 seam。Desktop、Browser 与
未来 Mobile Client 各自在输入所在设备完成采集、重采样、VAD、识别和模型管理；Application
Transport 不传输实时音频、recognition session 或设备权限。

Speech Recognition Plugin 只向共享 TipTap Ramble Core 投影统一 SpeechEvent；Capture Plugin
只返回 Attachment Candidate。平台共享事件和候选合同，不共享同一个引擎进程、模型或 acquisition
UX。

### 6. Local Integration Server 与 Web Access 独立

Local Integration Server 服务 Host Adapter；Web Access 服务浏览器。它们必须复用 server Module
内同一套 security policy/primitives，但拥有独立 listener handle、route set、credential、auth
domain 和启停 lifecycle。关闭 Web Access 不得停止 Backend Runtime 或 Local Integration Server。

Web Access 默认关闭，第一阶段只绑定 `127.0.0.1`。本决策不指定新 crate、Web app 目录或
headless composition root。

### 7. Web Access 使用 bootstrap 后的短期 session credential

Web Access 使用与 Local Integration Server 不同的 credential：

1. Desktop composition root 装配的 Web Access security Module 在人类显式启用或重新生成
   credential 时创建并持久化独立的 256-bit durable Web token；Backend Runtime/core 不拥有
   transport credential；durable token 不返回 UI，只有 Desktop 设置界面可经专用原生 clipboard
   command 复制它；
2. durable token 优先存入 OS credential store（macOS Keychain、Windows Credential Manager、
   可用时的 Linux Secret Service），并在平台支持时使用 device-local / non-sync 属性。仅允许回退
   到通用配置和 RambleDesk backup/export roots 之外的专用 secret file：Unix mode `0600`，
   Windows 使用仅当前用户可读的 DACL，并在平台支持时设置 backup exclusion；当前实现不使用
   secret-file fallback，无法使用 OS credential store 时 Web Access 必须 fail closed。RambleDesk
   不得把 token 复制到通用配置、
   SQLite、日志、诊断包、自己生成的 backup/export 或 Feedback Package；OS 管理的加密设备/账户
   备份属于平台安全边界，不宣称应用能够绝对排除；
3. 浏览器以 `Authorization: Bearer <durable-web-token>` 调用 same-origin
   `POST /api/auth/session`；成功后签发 scope 受限、idle TTL 30 分钟、absolute TTL 12 小时的
   session token；受保护 HTTP 请求或新的 WebSocket 认证可以刷新 idle TTL，但不能延长 absolute
   TTL，已连接 WebSocket 到期时主动关闭；
4. session token 只存在 Web Access 进程内存与浏览器当前 JavaScript 内存；页面刷新或关闭后必须
   重新 bootstrap；
5. durable/session token 禁止进入 `sessionStorage`、`localStorage`、IndexedDB、URL、日志或
   Feedback Package；
6. 停止 Web Access 必须撤销全部 session；重新生成 durable token 必须撤销旧 durable token 与
   全部 session；
7. HTTP 使用 `Authorization: Bearer <session-token>`，服务端使用 constant-time comparison 并
   对认证 header 与错误上下文做日志脱敏；
8. WebSocket 通过 `Sec-WebSocket-Protocol` 同时 offer `rambledesk-events` 与
   `rambledesk-session.<base64url-no-pad-session-token>`，禁止 query token；
9. 服务端校验两个 protocol 后只选择并回显 `rambledesk-events`，不得回显 credential-bearing
   protocol，也不得把它写入代理日志或诊断输出。

静态资源、HTTP API 与 WebSocket 使用 same-origin，不开放宽泛 CORS。所有请求严格校验 Host；
bootstrap、受保护 API 与 WebSocket handshake 还必须 exact-match Origin，以防 DNS rebinding。
Web routes 分别设置 body、upload、rate 与 concurrent-connection 上限。敏感 command 单独分类、
授权与审计。

## Current vs Target

### Current

- `apps/desktop` 是 composition root；每个 desktop 进程装配一份 `FeedbackApplication`。
- Desktop Client 使用 Tauri commands/events；Desktop Shell 提供 Native Capability。
- Local Integration Server 的 loopback listener 承载 `/api` 与 `/mcp`，不提供 Web
  static/application/WS route。
- Local Integration Server 使用持久 256-bit hex bearer token，并 constant-time compare。
- token 文件在 Unix 为 `0600`；非 Unix 当前没有等价 ACL 保证。
- Host 只接受 `127.0.0.1` 或 `localhost`；Origin 缺失供本地非浏览器客户端，存在时 exact
  allowlist；统一 body limit 为 96 MiB。
- 默认关闭的独立 Web Access Server 只绑定 `127.0.0.1`，提供共享 SPA、application HTTP 与
  readiness/invalidation WebSocket；它与 Local Integration Server 分离 listener、credential、auth
  domain、route set 与 lifecycle。
- Web Client 复用 Workbench Client，以 OS credential store 中的独立 durable token bootstrap
  短期、仅内存 session；当前支持文字 Draft、附件 file picker、提交、宿主会话操作及安全反馈投影下载。
- Web Access 当前限制为每分钟 8 次 bootstrap、16 个并发 application HTTP request、8 个 event
  socket；JSON 与 attachment upload 分别使用有界 body。session hash 对有界 session 集合完整
  constant-time compare；request authorization 是 admission lease，已入场 mutation 不会在提交后被
  revoke 改写成 401。
- Draft CAS、Tauri/HTTP application parity、ready/refetch 与 session revoke/re-auth 已实现。

### Target

- LAN/TLS Web Access 与更完整的 credential 管理 UI。
- Browser Speech Recognition Plugin、Capture Plugin 与更完整的 capability manifest。
- Native/Browser Capability 继续位于 Application Transport 外，通过 manifest 呈现差异。

## Rejected

- **让 Desktop Client 也通过 HTTP：** 增加本地 listener 依赖和序列化成本，不能替代 Tauri
  对 desktop lifecycle 的直接组合。
- **为 Web Client 建第二套后端或领域实现：** 会产生事实、状态机和安全规则漂移。
- **把 Capability 调用并入 Application Transport：** 会让共享 UI 把浏览器权限误认为后端或
  desktop 能力。
- **让 Web Access 复用 Local Integration Server 的 listener/credential/auth domain：** 无法
  独立关闭 Web，也会把持久本地集成 credential 暴露给浏览器。
- **WebSocket 发送完整 canonical state 或依赖 replay：** 增加协议复杂度，并让事件流变成脆弱的
  第二事实源。
- **在 URL query 传 token：** credential 会泄漏到历史、日志、代理和错误报告。
- **把 Browser Capability 描述为 Native Capability 等价实现：** 与浏览器权限和设备边界不符。

## Deferred

- LAN Web Access；启用前必须采用 HTTPS/WSS 或受信任 TLS proxy，并重新安全审计；
- headless Backend Runtime / composition root；
- headless 或独立 Web deployment packaging；
- 完整 credential rotation/revocation 管理 UI；
- sequence replay、ring buffer、multiplex protocol；
- 浏览器本地 sherpa-onnx WASM 的生产模型矩阵与性能优化；
- ACP、Router 或全局 client state framework。

## Consequences

正向：

- Desktop/Web 共享 UI 与业务合同，不产生第二 Backend Runtime 实现；
- listener、credential、auth domain 和 capability 权限均可独立审计；
- WebSocket 断线后可从 Backend Runtime snapshot 恢复；
- 浏览器功能可以诚实降级，不污染原生实现。

代价：

- 需要维护 Tauri/HTTP parity、ready/refetch、Web credential bootstrap/session lifecycle 与静态
  资源安全合同；
- 多客户端会暴露真实 CAS 冲突，UI 必须提供恢复而非静默重试；
- Desktop 与 Web 需要分别验收设备权限和安全策略。

## 与 ADR 001–004 的兼容性

- **ADR 001：** 保持 `core`、storage、transport 与 composition root 分离；Backend Runtime 是运行
  角色，不创建隐含新 crate，也不把领域规则或 transport credential 放入 core。两个 listener
  必须复用同一套 security policy/primitives，延续“本地安全策略只有一处实现”，但分离 credential、
  auth domain 与 lifecycle。
- **ADR 002：** 保留 `cpal` / speech crate 作为 Desktop Speech Recognition Plugin 的内部实现；
  Browser 通过自己的本地 Plugin 接入，不改写现有 SpeechEvent / Draft contract。
- **ADR 003：** Ramble 仍是统一 TipTap 编辑流程，但 Web Client 必须按 capability manifest 显示缺失或
  降级的全局快捷键、系统截图和剪贴板能力。
- **ADR 004：** 本 ADR 修订其“整个应用最多一个 Editor”的所有权作用域为“每个 Workbench Client
  instance 最多一个 Editor”。canonical Draft 仍在 Backend Runtime/SQLite；跨客户端并发只由
  revision/CAS 仲裁，不共享 Editor handle、不引入 hidden Editor。
