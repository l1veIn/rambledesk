# RambleDesk 架构基线

> 状态：v6 当前与目标边界，包含 ACP 托管会话。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。本文若与术语表冲突，以术语表为准。

本文同时记录已经存在的结构与后续平台扩展必须遵守的目标边界：

- **CURRENT** 表示仓库当前已经实现并可验证的事实；
- **TARGET** 表示已接受但尚未完成的演进边界，不得在产品文案中冒充已有能力。

Backend Runtime 是运行角色，不是新 crate 的名字。除非另有标记，package 章节描述 CURRENT；
TARGET 不预设新 crate、Web app 目录或 headless composition root。

ACP 托管会话的 CURRENT 见 [ADR 007](adr/007-acp-managed-sessions.md)：Backend Runtime 持有
Agent Session Management，ACP Client 与进程管理实现位于 core application contract 之外。
一个 RambleDesk Session 对应一个 Agent Session；首期独占 ACP Instance，不要求实例只有一个 OS
进程。持久会话、运行投影与反馈投递分别建模，Client view 不拥有其生命周期。
独立 `rambledesk-acp` 库提供稳定协议 v1 的 stdio 客户端、application driver 与 smoke example；采用官方
Rust SDK 2.0.0，SDK 主版本不等于 wire 协议版本。配置、交互、权限、托管反馈、停止、恢复和删除均已
接入 Desktop/Web 的统一 application 合同。支持版本和操作说明见 [ACP 托管会话](ACP_MANAGED_SESSIONS.md)。

## 运行时拓扑

### CURRENT

```text
┌────────────────────┐   MCP / Local JSON API   ┌──────────────────────────┐
│ Host Adapters      │ ───────────────────────→ │ Local Integration Server │
│ Generic MCP / Pi   │                          │ one /api + /mcp listener │
└────────────────────┘                          └────────────┬─────────────┘
                                                           │ application calls
                                                           ▼
┌────────────────────┐   Tauri Application      ┌──────────────────────────┐
│ Desktop Client     │ ─ Transport Impl. ─────→ │ Backend Runtime          │
└────────────────────┘                          │ one application Module   │
                                                │ core + storage + config  │
┌────────────────────┐   Web Access HTTP + WS   │                          │
│ Web Client         │ ─ Transport Impl. ─────→ │                          │
└────────────────────┘                          └──────────────────────────┘
                                                └──────────────────────────┘

┌────────────────────┐
│ Desktop Shell      │ ─── Native Capability Implementation
└────────────────────┘      (outside Application Transport)

Backend Runtime
  └─ core::SessionApplication
      ├─ storage: Session / AgentConfig / Activity / Delivery / Recovery / Deletion
      └─ AcpSessionDriver → owned stdio ACP Instance → Agent Backend
           └─ scoped HTTP MCP → Local Integration Server /mcp-managed → feedback application
```

`apps/desktop` 是 CURRENT composition root。每个 desktop 进程创建一份 Backend Runtime/
application facade，由 Tauri state、该进程的 Local Integration Server 与可选 Web Access Server
共同调用；这不表示跨进程全局单例。Local Integration Server 继续以独立 loopback listener 承载
`/api`、`/mcp` 与托管会话专用 `/mcp-managed`，不暴露 Web 静态资源、application routes 或 WebSocket。默认关闭的 Web Access
使用另一 listener、credential、auth domain、route set 与生命周期，固定绑定 `127.0.0.1:37643`。MCP SSE
属于 MCP transport，不是 Web Client 的事件流。

`/mcp-managed` 为每次运行绑定单会话凭据，复用相同 listener 与 Host/Origin policy；凭据不能访问 Generic
MCP，也不能跨会话或跨 MCP transport session 使用。停止/删除会话撤销绑定。托管请求在可信 application
入口注入本地会话归属，反馈终态与 outbox 入队原子提交；worker 只在原会话空闲后发送，结果不明需人工处理。

运行检查点与实时投影分开。重启不信任旧 connected 状态；未完成轮次通过持久检查点产生中断活动，显式恢复
使用原 remote id 的 resume/load。删除先持久化 intent，再停止资源并清理所属数据，失败可重试；文件清理与
发布共用锁，旧 publication plan 不得重新生成已删除的包。

### TARGET

```text
┌────────────────────┐                          ┌──────────────────────────┐
│ Host Adapters      │ → Local Integration ──→ │                          │
└────────────────────┘   Server                 │                          │
                                                │ Backend Runtime          │
┌────────────────────┐   Tauri Application      │ same application Module  │
│ Desktop Client     │ ─ Transport Impl. ─────→ │ and business facts       │
└────────────────────┘                          │                          │
                                                │                          │
┌────────────────────┐   Web Access HTTP + WS   │                          │
│ Web Client         │ ─ Transport Impl. ─────→ │                          │
└────────────────────┘                          └──────────────────────────┘

Desktop Shell ─── Native Capability Implementation ───┐
                                                      ├─ outside Application Transport
Web browser  ─── Browser Capability Implementation ───┘

Desktop / Browser / Mobile Platform Plugin
  └─ local speech or capture ──→ SpeechEvent / Attachment Candidate ──→ TipTap Ramble Core
```

Desktop Client 与 Web Client 复用同一 Workbench Client 和 Application Transport
Interface。Tauri 与 HTTP + WebSocket 是两个 Implementation，但调用同一 Backend Runtime
application Module；Web 路径不得形成第二套业务实现。

Local Integration Server 与 Web Access 必须复用 server Module 内同一套 security policy/primitives，
但拥有独立 listener handle、route set、credential、auth domain 和启停生命周期。单一安全策略
实现不等于共享 listener。Web Access 默认关闭；停止它不得停止 Backend Runtime 或 Local
Integration Server。

## Application Transport 与恢复合同

Application Transport Interface 暴露 typed command/query、变化订阅、ready barrier 和
capability manifest。Capability manifest 只报告能力是否可用；设备操作不通过 Transport 执行。

CURRENT Web Transport 遵循：

1. Backend Runtime 每次启动生成一个 opaque、进程生命周期内不变且跨启动唯一的
   `runtime_generation`；
2. WebSocket `ready` frame 携带该 generation。Client 只接受当前 connection epoch 的 ready，并
   原子替换 active generation、清空旧投影、取消或标记旧 in-flight HTTP request 为 stale；
3. HTTP snapshot/query 返回 Backend Runtime 当前事实的可丢弃投影，不成为第二事实源；每份
   snapshot 携带 generation 和相关 resource revision；
4. WebSocket 只传递 readiness 与轻量 invalidation，不承载 canonical Request/Draft/Package；
   invalidation 携带同一 generation 下可比较的 resource key/revision；
5. 客户端确认 ready 并建立 active generation 后，才允许发出会产生事件或修改状态的 command，
   避免首次订阅窗口丢事件。typed command envelope 携带 `expected_runtime_generation`，Web HTTP
   Implementation 映射为 `X-RambleDesk-Runtime-Generation`；Web session record 也绑定签发时的
   generation。服务端在任何领域副作用前原子比较 session、command 与当前 generation，任一不匹配
   都返回 HTTP `409` / typed `stale_generation`；Client 重新 bootstrap、连接并 refetch。resource
   revision/CAS 不能替代该 runtime generation 检查；
6. Client 只应用与 active ready generation 一致、且 revision 不低于已应用投影的 response；来自
   旧 socket、旧 connection epoch 或旧 generation 的 frame/response 一律丢弃；
7. fetch 期间若收到更高 revision 的 invalidation，当前 fetch 完成后必须再次 refetch；
8. WebSocket 断线后先重新建立并确认 ready，再 refetch 完整 snapshot；
9. 首期不实现 sequence replay、ring buffer 或 multiplex protocol。

CURRENT Tauri commands/events 与 Web Access HTTP + WebSocket 分别是 Desktop Client 和 Web
Client 的具体 Implementation。现有 MCP SSE 不能复用为上述 WebSocket
invalidation/readiness contract。

## Capability 边界

Native Capability 与 Browser Capability 都位于 Application Transport 之外。共享 Workbench
Client 只能根据 capability manifest 呈现可用操作：

- Native Capability 可以提供全局快捷键、系统截图、原生录音、tray、updater 和 native dialog；
- Browser Capability 受 secure context、浏览器权限、用户手势和当前设备限制；
- Browser file picker 选择客户端文件，不代表 Backend Runtime 所在机器的 working directory；
- 缺失或降级的 Browser Capability 必须明确不可用，不能伪装成 Native Capability 等价实现。

语音和截图等设备流程由当前客户端的 Platform Plugin 组合。Platform Plugin 是第一方、typed 的
Capability Module，不是 Host Adapter，也不隐含动态第三方插件加载器。它不经 Application Transport
代理设备操作，只向共享 TipTap Ramble Core 返回 `SpeechEvent` 或 `AttachmentCandidate`。

## Feedback Draft 所有权

每个 Workbench Client instance 最多只有一个可编辑 `RichFeedbackEditor`。当前 request 通过 Editor
transaction 写入；后台 Active Ramble 通过 TipTap JSON transformation、单一串行队列和
CAS 写入。数据库以版本化 `document_json` 为真源，保存时从同一 Document 同时生成
`body_markdown`。多个客户端并发编辑时，Backend Runtime 的 Draft revision/CAS 负责仲裁；
冲突必须显式反馈给人类，不得静默覆盖。详见
[ADR 004](adr/004-single-editor-structured-draft.md)。ADR 005 将 ADR 004 的“整个应用”作用域修订为
“每个 Workbench Client instance”；同一 Draft 可以在多个客户端打开，但只能通过 Backend Runtime
revision/CAS 协调，不能共享 Editor handle 或 client-local 文档状态。

禁止 per-request Editor、hidden Editor、session 持有 editor handle，以及自动 Tidy。

Action 使用带 `actionId` / `actionIndex` 的标准 Blockquote。ASR paragraph 使用稳定 `speechSegmentId` 与 `pending` / `cleaned` 状态。Tidy 只由当前 Editor 的人工按钮触发；Tidy 与 Cooking 配置彼此独立。全局快捷键只发出语义事件，不读取或持有 Editor。

## Package 边界

```text
rambledesk/
├── apps/
│   └── desktop/                  # Workbench UI + Tauri composition root
├── crates/
│   ├── rambledesk-core/          # application contract
│   ├── rambledesk-acp/           # ACP stdio driver + owned process resources
│   ├── rambledesk-storage/       # SQLite + feedback package publication
│   ├── rambledesk-local-server/  # CURRENT Local Integration Server
│   ├── rambledesk-mcp/           # Generic MCP Adapter (tool surface + installer engine)
│   ├── rambledesk-hosts/         # Host knowledge registry + profiles + continuation strategy
│   ├── rambledesk-speech/
│   └── rambledesk-cli/
├── packages/
│   ├── pi-rambledesk/            # Pi Native Adapter
│   └── dsh-rambledesk/           # dsh Native Adapter
└── docs/
```

### `rambledesk-core`

持有：

- 反馈请求 use cases：request/get/wait/cancel/list；
- 反馈草稿、附件和提交 use cases；
- 反馈包输出合同；
- 托管会话、启动配置、输入/权限/生命周期、活动和反馈投递 use cases；
- 稳定 DTO、错误码、状态机；
- Repository、PackagePublisher、Clock、IdGenerator 等 ports。

不得持有：

- HTTP、JSON、MCP、Pi package、Tauri command；
- Local Integration Server listener、token path、Host/Origin guard；
- 宿主安装逻辑、host profile、continuation strategy；
- 源码 checkout 模型或把外部反馈请求绑定到源码路径的依赖；托管会话合同携带执行目录 `cwd`。

### `rambledesk-acp`

持有官方 ACP SDK、stdio 通信、能力协商、权限回调映射，以及独占实例的启动与进程树清理。实现 core 的
Agent driver ports，不持有 SQLite、HTTP 路由或 Tauri UI。当前只宣告已实现的 Client capabilities，不承接
客户端文件/终端执行；有远端绑定时严格 resume/load，失败不回退为新会话。

### `rambledesk-storage`

持有：

- SQLite schema 与 migrations；
- core repository ports 的实现；
- request、draft、attachment metadata 持久化；
- 跨请求宿主会话关联；
- Agent 配置、托管会话、活动、投递、删除意图与运行检查点持久化；
- 不可变反馈包发布和恢复对账。

不得持有：

- 宿主协议；
- 适配器安装；
- 源码 checkout runtime 语义；
- UI 或 transport 细节。

### `rambledesk-local-server`（CURRENT）

持有：

- loopback HTTP listener；
- bearer token 生成、读取、默认路径；
- Host/Origin guard；
- `/api/feedback/request|get|wait|cancel`；
- `/mcp` 与受会话作用域约束的 `/mcp-managed` route mounting；
- Local Integration 与 Web Access 的独立 server handle、endpoint、Web session auth、静态资源与
  application/event routes；
- Web Access 固定 loopback、安全限制 snapshot 与 listener lifecycle。

Web Access 已复用该 crate/server Module 的 security policy/primitives；这不要求新 crate，也不允许它
复用 Local Integration Server 的 listener handle、credential、auth domain、route set 或启停生命周期。

不得持有：

- 领域规则；
- MCP tool schema；
- Pi package 代码；
- desktop UI 状态。

### `rambledesk-mcp`

持有 Generic MCP Adapter 完整方案（与 `packages/pi-rambledesk` 对等）：

- MCP tool schema、tool handler、instructions、structured result / error 格式化；
- MCP request 到 `rambledesk-core` application call 的映射；
- 客户端检测/安装执行引擎：按 `rambledesk-hosts` 声明的 `ConfigFormat` 分发，把
  RambleDesk 服务器条目写入各宿主的配置文件（JSON/TOML），含幂等与修复。

不得持有：

- HTTP listener、token path、local JSON API routes；
- host-specific continuation 实现；
- per-host 知识（可执行文件名、配置路径、配置格式声明）——这些属于 `rambledesk-hosts`。

### `rambledesk-hosts`

持有：

- 宿主知识注册表（单一真相源）：每宿主的 executable、marker 目录、配置文件路径、
  配置格式（`ConfigFormat`），以及默认适配器选择；
- Host Profile catalog（从知识注册表派生）；
- host label/icon；
- continuation 模式声明；
- 手动 continuation 提示 payload；
- 未来原生 continuation strategy 接口。

不得持有：

- MCP implementation、Pi package implementation；
- 适配器安装/写入执行逻辑（检测执行与格式引擎属于 `rambledesk-mcp`）；
- storage 或 desktop UI 状态。

### `packages/pi-rambledesk`

持有 Pi 原生适配器：

- Pi tools：`request_ramble_feedback`、`get_ramble_feedback`；
- request/get/wait/cancel 到本地 JSON API 的调用；
- Pi tool call 内等待终态；
- Pi package 安装说明和测试。

不得持有：

- MCP client 行为；
- desktop UI 状态；
- RambleDesk storage 逻辑。

### Desktop（CURRENT）

Desktop Shell 负责：

- 进程、窗口、tray、系统通知、文件选择、权限提示；
- 每进程装配一份 Backend Runtime、Local Integration Server 与 hosts；
- 暴露 Tauri commands；
- 桥接领域结果为前端事件。

Desktop Client 中的 Svelte Workbench Client 负责：

- 工作台投影；
- 人类反馈输入；
- 草稿、截图、录音、附件交互；
- 设置中的适配器安装 UX。

Workbench Client 不持有唯一业务事实。Tauri events 和系统通知都是提示，不是事实来源。CURRENT Web
Client 已复用该 UI，但不复用 Desktop Shell 或 Native Capability Implementation；具体支持面见
[WEB_ACCESS_SUPPORT_MATRIX.md](WEB_ACCESS_SUPPORT_MATRIX.md)。

### Workbench Client 生命周期

- 关闭 workspace Tab、关闭或刷新浏览器、Transport 断线，都只结束对应 Client 的 view / projection；
- 上述行为不得隐式 submit、cancel、archive Request，也不得停止后台 Active Ramble 或未来 Agent /
  Session Runtime；
- submit、cancel、archive 与停止 runtime 必须是可审计的显式 application command；
- Client 应持续 autosave canonical Draft 变更。关闭 workspace view 仍执行既有 save gate；浏览器
  `unload` 不可靠，因此不能把唯一一次保存或任何终态 mutation 放在 `unload` handler 中；
- 重连或重新打开 view 时，从 Backend Runtime 重新读取 Request、Draft revision 与运行状态。

### TipTap Ramble Core 与 Platform Plugin

Ramble 是以 TipTap Feedback Draft 为中心的编辑流程，不是录音 session。共享 Ramble Core 持有
SpeechEvent 到 TipTap transaction 的映射、stable segment identity、附件 node、autosave 与 CAS；
它不持有麦克风/屏幕权限、PCM、重采样、VAD、模型、WASM Worker 或系统截图 overlay。

- Desktop Speech Recognition Plugin 在本机组合 Native Audio Source、Rust sherpa-onnx Speech Engine
  与模型资料库；`rambledesk-speech` 的 source/engine seam 是其内部实现；
- Browser Speech Recognition Plugin 在浏览器所在设备组合 `getUserMedia`、AudioWorklet、流式重采样、
  dedicated Worker、sherpa-onnx WebAssembly 与 origin-local Model Store；
- Mobile Client 未来通过各自平台的原生音频 API 与 sherpa-onnx binding 实现同一插件合同；
- Capture Plugin 在当前设备取得图像并返回 Attachment Candidate；共享 Draft 流程负责验证、上传或
  持久化，成功后再插入 TipTap attachment node；
- 平台共享 SpeechEvent 与 Attachment Candidate 合同，不共享引擎进程、模型或权限；实时音频、
  recognition session 与设备权限不进入 Application Transport。

CURRENT Browser local ASR pilot 已自动化覆盖固定模型下载/hash/cache、Wasm/Worker/AudioWorklet 合同
与 recognizer creation；这不是 Chrome/Safari 真实麦克风兼容证明。麦克风授权、PCM 输入、稳定出字、
停止 flush 与长会话仍是人工未验项。Browser screen capture 尚未交付，不能从 `getDisplayMedia()` 的平台
API 推断为当前支持。

## 事实来源

| 数据 | 唯一事实来源 |
| --- | --- |
| 反馈请求状态 | SQLite |
| 草稿正文 | SQLite 中的版本化 `document_json`；`body_markdown` 为同文档投影 |
| 草稿附件 bytes | 应用 draft 目录，SQLite 存 metadata |
| 反馈包 | 不可变 package directory + manifest |
| 宿主身份 | adapter-provided `host_id`，服务端可按安装入口覆盖 |
| 宿主会话关联 | adapter-provided `host_session_id` |
| 上下文提示 | request `context_refs` / `source_hint` |
| Session tabs、顺序、active view、pane 尺寸 | 每个 Workbench Client 的 client-local workspace snapshot |
| 系统通知 | best-effort side effect |
| 局部转写 | 当前客户端 Platform Plugin 内存；稳定 SpeechEvent 通过 TipTap transaction 写入 Draft |

HTTP snapshot、Tauri query result 和 WebSocket invalidation 都是上述事实的投影或提示，不是
额外事实源。终态提交、取消等 application operation 必须幂等；CAS 冲突必须保留可见错误，
不得由任一 Transport Implementation 自动覆盖。

## 核心流程

### 通用 MCP 适配器

```text
MCP request_feedback
  → rambledesk-mcp 映射工具输入
  → rambledesk-core request_feedback
  → rambledesk-storage 持久化请求
  → Local Integration Server 返回 waiting 结果
  → 宿主智能体结束当前 turn
  → 人类在工作台提交/取消
  → 手动 continuation 提示
  → 宿主智能体调用 get_feedback(request_id)
```

通用 MCP 适配器不提供自动恢复原宿主上下文的产品保证。

### Pi 原生适配器

```text
Pi request_ramble_feedback
  → /api/feedback/request
  → Workbench receives persisted request
  → /api/feedback/wait blocks inside Pi tool call
  → Human completes/cancels in Workbench
  → wait returns terminal Feedback Package
  → Pi continues original task
```

Pi 不需要提交后的 continuation。

### 人类提交

```text
SubmitFeedback(request_id, expected_revision)
  → verify non-terminal state
  → render package into temp directory
  → flush + hash
  → atomic publish
  → persist terminal request state + package metadata
  → notify waiters / prepare continuation prompt
```

如果 package 已发布但数据库更新失败，启动恢复任务根据 manifest/request_id 对账；不得创建第二份 package。

## 状态模型

```text
waiting → in_progress → completed
   │           │
   └───────────┴──────→ cancelled
```

`completed` 和 `cancelled` 是终态。只有终态触发 continuation。

## 数据布局

应用数据：

```text
<local-data>/RambleDesk/
├── feedback.sqlite3
├── auth/
├── drafts/<request-id>/
├── feedback/<timestamp>-<request-id>/
├── models/
├── logs/
└── recovery/
```

反馈包默认写入应用数据目录。适配器若提供安全可写的路径 hint，未来可以作为导出或镜像目标，但核心协议不得要求源码 checkout 路径。

## 安全边界

### CURRENT：Local Integration Server

- 每个 desktop 进程当前只有一个 `/api` + `/mcp` listener，且只绑定 loopback。
- Local Integration Server 使用持久的 256-bit hex bearer token，并以 constant-time comparison
  校验所有 `/api` 与 `/mcp` 请求。
- token 文件在 Unix 上使用 `0600`；非 Unix 平台当前没有文档化的等价 ACL 保证，因此该凭据
  不得直接复用为 Web credential。
- Host 只接受 `127.0.0.1` 或 `localhost`；Origin 缺失时只作为本地非浏览器客户端调用放行，
  Origin 存在时必须 exact-match allowlist。
- 当前统一 request body limit 为 96 MiB。
- 当前 listener 不提供 Web 静态资源或 WebSocket。MCP SSE 不属于 Web event stream。
- Draft revision/CAS 已存在于 application/storage 路径。可复用的 application HTTP router、浏览器
  HTTP Application Transport Implementation 与 Tauri/HTTP conformance tests 已实现；Local
  Integration Server 仍不暴露这些 application routes。
- `host_id`、`host_session_id` 不是认证凭据；返回路径只保证同机、共享文件系统可见。

### CURRENT：loopback Web Access

- Web Access 默认关闭，使用独立 listener 且固定绑定 `127.0.0.1:37643`；它拥有与 Local
  Integration Server 分离的 route set、credential、auth domain 和 lifecycle。
- 静态资源、HTTP API 与 WebSocket 使用 same-origin 且不开放宽泛 CORS。两类 listener 必须复用
  同一套 security policy/primitives。所有请求严格校验
  Host；bootstrap、受保护 API 与 WebSocket handshake 还必须 exact-match Origin，以防 DNS
  rebinding；共享安全实现不表示共享 listener 或 credential。
- Desktop composition root 装配的 Web Access security Module 在人类显式启用或重新生成 Web
  credential 时创建并持久化独立的 256-bit durable token；Backend Runtime/core 不拥有 transport
  credential。durable token 不返回 UI；只有 Desktop 设置界面可以经专用原生 clipboard command
  复制它。
- durable token 进入 OS credential store（macOS Keychain、Windows Credential Manager、
  Linux Secret Service）；当前不使用 secret-file fallback，安全存储不可用时 Web Access fail closed。
  RambleDesk 不得把 token 复制到通用配置、SQLite、
  日志、诊断包、自己生成的 backup/export 或 Feedback Package；OS 管理的加密设备/账户备份属于
  平台安全边界，不宣称应用能够绝对排除。
- 浏览器以 `Authorization: Bearer <durable-web-token>` 调用 same-origin
  `POST /api/auth/session` 完成 bootstrap；成功后得到 scope 受限、idle TTL 30 分钟、absolute TTL
  12 小时的 session token。受保护 HTTP 请求或新的 WebSocket 认证会刷新 idle TTL，但不能延长
  absolute TTL；已连接 WebSocket 到期时主动关闭。停止 Web Access 必须撤销所有 session 并关闭
  socket；重新生成 durable token 必须同时撤销旧 token 与全部 session。
- session token 只存在 Web Access 进程内存与浏览器当前 JavaScript 内存；durable/session token
  都不得进入 `sessionStorage`、`localStorage`、IndexedDB、URL、日志或 Feedback Package。刷新或
  关闭页面后必须重新 bootstrap。服务端比较
  credential 时使用 constant-time comparison，并对认证 header 和错误上下文做日志脱敏。
- 浏览器 HTTP 请求使用 `Authorization: Bearer <session-token>`。
- 浏览器 WebSocket 通过 `Sec-WebSocket-Protocol` 同时提供稳定协议 `rambledesk-events` 与
  credential-bearing protocol `rambledesk-session.<base64url-no-pad-session-token>`，禁止 query
  token。服务端校验二者后只选择并回显 `rambledesk-events`，不得把 credential-bearing protocol
  回显给客户端、代理日志或诊断输出。
- 静态 SPA 只接受 exact Host；bootstrap、application API 与 WebSocket 还要求 exact Origin。
  history fallback 不覆盖 `/api/**`、`/assets/**`、扩展名路径、traversal 或 encoded separator；HTML
  `no-store`，build manifest 明确标记的 fingerprinted asset 使用 immutable cache。
- 当前每个 listener 每分钟最多接受 8 次 bootstrap 尝试，同时最多处理 16 个 application HTTP
  request 与 8 个 event socket；超过上限分别返回 `429` 或 `503`。JSON 使用 Axum 的有界 body，
  multipart 使用 core 的 20 MiB attachment 上限外加 64 KiB metadata allowance。
- session credential 以固定长度 SHA-256 hash 保存，并对最多 32 个 session 做完整 constant-time
  compare 扫描。认证是 request admission lease；已入场 mutation 即使随后 stop/expiry，也返回真实
  结果且绝不自动重放，后续 request 与 event socket 才被撤销。
- Web Client 只在 bootstrap 成功后创建 HTTP + WebSocket Transport；durable token 输入成功后即清空，
  session token 只留当前 JavaScript 内存。认证被撤销时 dirty Editor 不被卸载，重新认证后 refetch
  Backend Runtime 已保存 Draft 与 client-local tab snapshot。

### TARGET：Web Access 扩展

- 后续可按 command sensitivity、client/IP 与运维场景继续细分资源预算和审计；Application
  Transport 可达不等于拥有所有 Native Capability 或管理权限。
- Browser screen capture 延后；平台 API 存在不构成当前产品支持。
- LAN、TLS、autostart、可配置端口与 headless Backend Runtime 当前均为 unsupported / out of
  scope；若未来重新立项，LAN 至少必须使用 HTTPS/WSS 或受信任 TLS proxy，并重新审计 origin、
  credential delivery、设备暴露与文件访问边界。

以上目标决策记录于
[ADR 005](adr/005-shared-workbench-transport-capabilities.md)。
