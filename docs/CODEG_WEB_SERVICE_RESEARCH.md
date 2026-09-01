# Codeg Web 服务与 RambleDesk 前后端分离调研

> 状态：调研完成，待架构评审
> 日期：2026-09-01
> Codeg 基线：`main` 提交 [`46636d7`](https://github.com/xintaofei/codeg/tree/46636d758ebd66ae4b29787b4206491c4a1fee03)，版本 `0.29.0`
> 范围：Web 服务生命周期、双端前端、Transport seam、HTTP/WebSocket/SSE、认证、静态资源、跨客户端同步，以及快捷键、截图、录音与 ASR 的平台边界。
> 约束：Codeg 事实只引用 Codeg 官方仓库与官方文档；浏览器/Tauri 能力只引用其官方文档或 MDN。对 RambleDesk 的建议是基于这些事实与当前 [架构基线](ARCHITECTURE.md)、[术语表](TERMINOLOGY.md) 作出的推论。

## 1. 结论摘要

Codeg 最值得 RambleDesk 借鉴的不是某个 Web 设置页面，而是下面这条主轴：

```text
一套静态前端
     │
统一 BackendTransport
  ┌──┴──────────────┐
Tauri IPC      HTTP + WebSocket
  │                  │
  └──── 同一业务核心与运行态 ────┘
```

前端只构建一次，Tauri 和浏览器使用不同 Transport；Rust 业务核心与状态不复制。Codeg 的环境检测只有一层很薄的 `window.__TAURI_INTERNALS__` 判断，UI 通过统一的 `call/subscribe` 合同访问后端。[官方架构说明](https://docs.codeg.app/reference/architecture)、[Transport 合同](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/types.ts#L92-L153)、[运行环境检测](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/detect.ts#L1-L8)

RambleDesk 可以采用同一个方向，但不应把 Codeg 当前已经累积的全部复杂度一起搬过来。建议分为两个大版本：

1. **版本 A：先完成 UI 与运行时边界重构。** 建立统一业务客户端、Tauri/Web Transport 和独立的本机能力 Adapter；现有桌面体验保持不变。
2. **版本 B：再提供 Desktop 可选启动的 Web 服务。** 同一静态 UI 通过 HTTP + WebSocket 访问同一 `rambledesk-core`、SQLite 和草稿真源；第一版以 loopback、安全的文本与附件旅程为主。

Codeg 默认监听 `0.0.0.0`，允许从 LAN 地址访问；这适合其产品目标，却不应成为 RambleDesk 的无条件默认值。尤其浏览器麦克风和主动屏幕捕获要求安全上下文，局域网裸 `http://192.168.x.x` 无法支撑完整录音/截图能力。[Codeg Web Service 文档](https://docs.codeg.app/reference/settings/web-service)、[MDN `getUserMedia`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)、[MDN `getDisplayMedia`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getDisplayMedia)

## 2. Codeg 的运行时与构建边界

### 2.1 一个核心，多个入口

Codeg 的 Rust workspace 以一个共享 `codeg_lib` 为核心；Desktop、`codeg-server` 和 `codeg-mcp` 是不同入口，Tauri 能力由 feature 控制，而不是复制业务实现。[Cargo feature 与 binary 定义](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/Cargo.toml#L11-L53)

前端是 Next 静态导出：同一构建产物既作为 Tauri 的 `frontendDist`，也被打进桌面资源目录 `web/`，并可由独立服务托管。[Next 静态导出配置](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/next.config.ts#L28-L35)、[Tauri 前端与 bundle 资源配置](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/tauri.conf.json#L6-L32)

这意味着“Web 版”不是第二个前端项目。差异集中在访问后端的 Transport 和本机能力边界，页面、领域状态和绝大多数交互组件仍然共享。

### 2.2 Transport seam

Codeg 的 Transport 合同承担两个职责：

- `call(command, args, options)`：命令/查询；
- `subscribe(event, handler)`：事件订阅；

另外暴露 `isDesktop`、等待连接就绪、重连回调、per-session event stream 和销毁等生命周期能力。[Transport 类型定义](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/types.ts#L92-L153)

两个主要实现如下：

| 实现 | 调用 | 订阅 | 证据 |
| --- | --- | --- | --- |
| Tauri | `invoke(command, args)` | Tauri event `listen/unlisten` | [TauriTransport](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/tauri-transport.ts#L9-L68) |
| Web | `POST /api/{command}` | `/ws/events` | [WebTransport HTTP 调用与订阅](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/web-transport.ts#L140-L227) |

Transport 选择发生在组合层：检测到 Tauri 就创建 TauriTransport，否则以当前 origin 创建 WebTransport。[Transport 选择](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/index.ts#L10-L75)

值得注意的是，Codeg 还有第三种 RemoteDesktopTransport：Tauri 窗口连接远程 Codeg server，并经 Rust proxy 处理 mixed-content 与远程 WS 生命周期。[RemoteDesktopTransport](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/remote-desktop-transport.ts#L73-L119) 这不属于 RambleDesk 第一阶段的必要条件。

### 2.3 Platform seam 不等于 Transport seam

Codeg 另有 `platform.ts`，封装打开 URL/路径、文件选择、窗口操作等运行时差异；浏览器文件选择与 Tauri 原生 dialog 使用不同实现。[平台封装](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/platform.ts#L16-L199)

这个区分很重要：

- **BackendTransport** 回答“UI 如何访问同一个后端”；
- **LocalCapability Adapter** 回答“当前这台客户端设备能做什么”。

若把两者混为 `isDesktop()` 分支，截图、录音、快捷键、文件路径等差异最终会散落在组件中。

## 3. Desktop Web 服务生命周期

### 3.1 配置合同

Codeg 保存三个 Web Service 配置项：

- `web_service_port`，默认 `3080`；
- `web_service_token`；
- `web_service_auto_start`，默认关闭。

配置保存在 SQLite `AppMetadata`。Token 的选择顺序是：启动时显式非空值、已持久化值、随机 UUID 去连字符；端口与 token 在 bind 成功后通过同一事务保存，避免只落下一半配置。[常量、token 与 port 解析](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L27-L222)、[配置读写](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L231-L295)

### 3.2 启动、停止与状态

Codeg 的启动流程有明确状态机：

1. atomic compare-exchange 防止并发启动；
2. 默认绑定 `0.0.0.0`；
3. socket bind 成功后才保存 token/port；
4. 将桌面进程已有的数据库、连接管理器、事件总线等共享句柄装入 HTTP Router 的 `AppState`；
5. spawn Axum server task；
6. 保存 shutdown handle、task handle、实际 port/token/host，并返回可访问地址。

共享启动路径见 [bind、持久化与运行状态](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L520-L629)，桌面特化装配见 [Tauri Web server 组装](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L700-L903)。

停止时先通知 WebSocket，随后 graceful shutdown；最多等待 2 秒，超时则 abort。只有 socket 确认释放后才清除 running 状态，避免 UI 已显示停止而端口仍被占用。[停止与状态查询](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L631-L694)

Desktop setup 会读取 `auto_start`，启用时自动启动 Web 服务；失败记录日志并发桌面通知。应用退出也会停止服务。[自动启动](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/lib.rs#L737-L756)、[退出清理](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/lib.rs#L1481-L1493)

### 3.3 设置页行为与监听范围

Codeg 的 Desktop 设置页提供：

- 端口、访问 token、自动启动；
- 运行/停止状态和 Start/Stop；
- 运行时锁定端口与 token；
- token 显示、复制、重新生成；
- loopback/LAN 地址选择、复制、二维码和浏览器打开；
- 停止状态下的端口占用探测。

配置修改在 500 ms debounce 后保存。[设置加载与保存](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/settings/web-service-settings.tsx#L280-L406)、[启停与设置 UI](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/settings/web-service-settings.tsx#L408-L599)

最容易误读的一点是：地址选择器只改变显示、复制与打开的目标，不改变监听地址。服务默认绑定所有网卡 `0.0.0.0`，因此 loopback 和每个 LAN IP 都可访问。[官方 Web Service 设置说明](https://docs.codeg.app/reference/settings/web-service)、[地址枚举与默认 host](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L415-L565)

Web 端会隐藏 Web Service 设置项，也不能停止承载自己的独立 `codeg-server`，否则调用会主动杀死自己的连接。[浏览器设置过滤](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/settings/settings-shell.tsx#L190-L193)、[外部管理服务的启停限制](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/handlers/web_server.rs#L49-L81)

### 3.4 独立服务不是 Desktop 内嵌服务的前置条件

Codeg 另有 `codeg-server`，从 `CODEG_PORT`、`CODEG_HOST`、`CODEG_TOKEN`、`CODEG_STATIC_DIR` 等环境变量读取配置，用于 headless/always-on 部署。[独立服务配置与启动](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/bin/codeg_server.rs#L144-L219)、[bind 与状态发布](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/bin/codeg_server.rs#L536-L585)、[官方部署说明](https://docs.codeg.app/getting-started/deployment)

RambleDesk 第一阶段不必因此同步提供独立 server binary、Docker、supervisor、自更新和远端部署；Desktop 内嵌、可启停的 Web 服务可以先独立交付。

## 4. HTTP、WebSocket、SSE 与静态资源

### 4.1 HTTP 命令面

WebTransport 将命令映射为 `POST /api/{command}`，JSON 编解码，默认 60 秒超时；`401` 会进入 unauthorized 状态。[WebTransport HTTP 调用](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/web-transport.ts#L140-L188)

Rust Router 手工挂载大量 command handler，未实现的命令返回 `501 not_implemented`。[Router 命令面与 501](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/router.rs#L1540-L1692) 这揭示了 Codeg 方案的一项实际成本：前端 Transport 很薄，但若把每个 Tauri command 一比一镜像到 HTTP，后端 parity 和安全审计面会快速膨胀。

### 4.2 静态资源托管

Codeg 的静态目录解析顺序会区分 Desktop 生产资源、开发 `out/`、`CODEG_STATIC_DIR` 与独立服务的 `./web`。[静态目录解析](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/mod.rs#L316-L388)

Router 使用 `ServeDir`，支持根 `index.html` fallback，并把无扩展名路由映射到 Next 静态导出的 `.html` 文件；API 和 WebSocket 与静态 UI 由同一 origin 提供。[静态路由与 fallback](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/router.rs#L1619-L1669)

同源托管对 RambleDesk 很合适：浏览器 UI、HTTP API 和 WS 可以共用 origin，第一阶段无需开放任意 CORS。

### 4.3 认证

Codeg 的认证规则是：

- HTTP：`Authorization: Bearer <token>`；
- WebSocket：浏览器无法自定义握手 Authorization header，因此用第二个 `Sec-WebSocket-Protocol` 传递 `codeg-token.<base64url-no-pad>`，同时协商 `codeg-events`；
- 服务端空 token 直接 fail closed；
- 浏览器登录页先调用 `/api/health` 验证，再把 token 存入 `localStorage.codeg_token`。

证据：[Rust token middleware](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/auth.rs#L9-L45)、[前端 WS protocol 编码](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/ws-auth.ts#L1-L22)、[Web 登录流程](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/app/login/page.tsx#L19-L53)

绝大多数 API 和 `/ws/events` 都在 token middleware 后；语言读取、一次性下载 ticket、Office preview capability 路径等少量路由例外公开。[公开/保护路由边界](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/router.rs#L1553-L1617)

Codeg 当前还配置了 `allow_origin/methods/headers Any`。[CORS 配置](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/router.rs#L27-L30) 官方文档明确说明 token 应视为密码，暴露到不可信网络时应使用 HTTPS/VPN/tunnel。[隐私与安全说明](https://docs.codeg.app/reference/privacy) RambleDesk 不应把 `0.0.0.0 + CORS Any + localStorage 长期 bearer token` 当作第一版默认安全模型。

### 4.4 WebSocket：全局广播与 per-session attach

Codeg 的同一个 `/ws/events` 承载两套语义：

1. 全局 `{channel, payload}` 广播，用于 folders、tabs、settings、tasks 等跨窗口/跨浏览器更新；
2. ACP per-session attach 协议，客户端发送 `attach/detach/ping`，服务端返回 `snapshot/replay/event/detached/pong`。

[WebSocket 双协议循环](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/ws.rs#L42-L250)、[Attach wire contract](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/ws_attach.rs#L1-L105)

全局事件会同时发给 Tauri webview 与浏览器 WS，因此两个 UI 观察同一后端状态。Tabs 同步还带 `version` 和 `origin`，客户端丢弃旧版本并忽略自己的 echo；保存端使用 expected version 做 CAS，冲突时返回服务器真值。[跨客户端事件桥](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/event_bridge.rs#L149-L223)、[Tabs version/origin](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/event_bridge.rs#L285-L306)、[Tabs CAS](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/commands/conversations.rs#L183-L207)

### 4.5 Snapshot、replay 与 reconnect

Codeg 的 per-session 可靠性协议已经相当完整：

- 首次 attach 返回完整 snapshot；
- 重连携带 `since_seq`；
- 小缺口从 ring buffer replay；
- 缺口超过 32 个事件或历史已被淘汰时重新给 snapshot；
- 一个 WS multiplex 多个 session subscription；
- 慢消费者 lag 后由服务端 detach，客户端再 attach；
- attach 的订阅建立与 snapshot/replay 判断在同一读锁下完成，避免 fetch-then-subscribe 丢事件。

[Snapshot/replay 决策](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/ws_attach.rs#L26-L185)、[客户端 attach 与 re-attach](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/web-event-stream.ts#L75-L191)

连接层还有另一组恢复措施：

- 服务端在 broadcaster receiver 已建立后发送 `__ready__`；
- WebTransport 的 `subscribe()` 等待 ready，避免 HTTP 已触发事件而 WS 还没有 receiver；
- 断线以 1、2、4……32 秒指数退避并持续重试；
- 先访问带认证的 `/api/health`，区分服务不可达与 token 失效；
- 普通全局广播本身不保证离线 replay，因此 reconnect callback 需要重新获取快照。

[Ready 与订阅等待](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/web-transport.ts#L194-L223)、[WS 连接与重连](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/web-transport.ts#L341-L527)、[无 receiver 时全局广播被丢弃](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/event_bridge.rs#L28-L70)

RambleDesk 第一版不需要一步实现这套完整协议。当前 Feedback Draft 已有版本化真源和 CAS，可以先用：

```text
HTTP snapshot = 真值
WS invalidate(id, revision) = 提示
重连 = refetch request list + 当前 request/draft
写冲突 = 使用现有 revision/CAS 返回服务器真值
```

只有出现高频流事件、长时间断线追赶或多客户端确实需要无损续接时，才引入 sequence、ring buffer、replay 和 lag-detach。

### 4.6 SSE 不是 Codeg 的主业务通道

Codeg 主业务实时通道是 WebSocket。SSE 只用于代理 `officecli watch` 的文档预览刷新：iframe 的 `EventSource('/events')` 经 capability-gated reverse proxy 透传。[Office Watch SSE proxy](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/handlers/office_watch_proxy.rs#L1-L31)、[SSE 转发](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/handlers/office_watch_proxy.rs#L278-L298)

因此不要把 RambleDesk 现有 MCP SSE compatibility 与 Web UI 状态同步混为一条通道。第一版普通请求走 HTTP、交互提示走一个 WS 即可，不必同时为 UI 引入 SSE。

## 5. 本机能力必须按“客户端设备”建模

浏览器访问 Desktop 的 Web 服务时，前端所在设备与服务所在设备可能不是同一台机器。平台边界不能只写成 `desktop/web`，还必须回答“能力发生在哪台设备”。

| 能力 | Tauri 客户端 | 浏览器客户端 | 统一产物/语义 |
| --- | --- | --- | --- |
| 页面内快捷键 | DOM keydown | DOM keydown | `WorkspaceShortcut` |
| 系统全局快捷键 | Tauri global shortcut | 不支持 | `SystemGlobalShortcut` capability |
| 原生截图/区域 overlay | Desktop host 原生能力 | 不等价；首版粘贴/上传 | `ImageAttachment` |
| 主动屏幕分享 | 可保留现有原生实现 | `getDisplayMedia()`，捕获浏览器所在设备 | `ImageAttachment` 或后续 screen-share session |
| 麦克风录音 | 当前 Tauri/Rust 音频采集 | `getUserMedia` + AudioWorklet，采集浏览器所在设备 | 当前客户端的 Speech Recognition Plugin |
| ASR | Rust sherpa-onnx，在 Desktop 本地运行 | sherpa-onnx WebAssembly，在 dedicated Worker 本地运行 | `SpeechEvent` 与统一错误状态 |
| 本地文件 | 可传 native path | 必须先上传 bytes/blob | `Attachment` |

### 5.1 快捷键

Codeg 的快捷键是页面级 DOM keydown，配置保存在 localStorage；它没有展示可供 RambleDesk 照搬的 OS 全局快捷键实现。[工作区 keydown](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/layout/workspace-chrome-controller.tsx#L76-L125)、[事件注册](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/layout/workspace-chrome-controller.tsx#L243-L260)、[快捷键 localStorage](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/keyboard-shortcuts.ts#L412-L438)

Tauri 的 OS 全局快捷键属于 Desktop plugin 与权限能力。[Tauri Global Shortcut 官方文档](https://v2.tauri.app/plugin/global-shortcut/) RambleDesk 应将 `WorkspaceShortcut` 与 `SystemGlobalShortcut` 拆成两个接口；Web UI 对后者明确显示“仅桌面支持”，不能把页面 keydown 当成全局快捷键降级实现。

### 5.2 截图

当前 Codeg main 可见的是粘贴/上传已有图片，而非系统区域截图：它从 Clipboard/DataTransfer 提取图片，在 Web/remote 环境先上传服务器，再引用服务器端文件。[Clipboard 图片处理](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/clipboard-images.ts#L1-L69)、[Web 图片上传](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/chat/composer/use-composer-attachments.ts#L576-L588)、[粘贴入口](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/components/chat/composer/use-composer-attachments.ts#L920-L983)

浏览器主动捕获屏幕只能使用 `getDisplayMedia()`：必须处于安全上下文，需要瞬时用户手势，每次都由用户选择并授权，权限不能持久复用，也不能等价复刻 Desktop 的全局区域 overlay。[MDN `getDisplayMedia`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getDisplayMedia)

因此 RambleDesk 建议：

- Tauri 保留现有全局热键、native capture、overlay/滚动截图/钉图；
- Web 首版支持粘贴与文件上传；
- 后续将 `getDisplayMedia()` 作为独立 capability，不宣称与 Desktop 截图等价；
- 两端最终都产出同一种 `ImageAttachment`，草稿逻辑不感知采集来源。

### 5.3 录音与 ASR

在固定 Codeg 快照中未发现 `getUserMedia`、`MediaRecorder`、SpeechRecognition 或录音/ASR pipeline；Codeg 不能作为这部分实现的直接参考。

RambleDesk 更合理的拆法是：

```text
Speech Recognition Plugin（客户端平台能力）
  ├─ Desktop：native capture + Rust sherpa-onnx
  └─ Web：getUserMedia + AudioWorklet + Worker + sherpa-onnx WASM
                 │
                 ▼
       SpeechEvent + timing/error
                 │
                 ▼
           TipTap Ramble Core
```

浏览器麦克风访问要求安全上下文和用户授权；loopback 可作为可信上下文，但局域网裸 HTTP 通常不行。Browser Speech Recognition Plugin 应读取实际采样率、用 AudioWorklet 取得 PCM，并在 dedicated Worker 中完成本地重采样、VAD 与 sherpa-onnx WASM 推理。[MDN `getUserMedia`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)、[MDN AudioWorklet](https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API/Using_AudioWorklet)

所以“让手机通过 LAN 地址访问并录音”会把 HTTPS/pairing 从部署优化提升为功能前提。Web 第一版先验证 loopback Browser 本地 ASR；LAN 与 Mobile Client 分别在拥有可信 HTTPS 和本地插件实现后开放，不以服务端音频上传作为 fallback。

## 6. 建议的 RambleDesk 目标边界

### 6.1 运行时拓扑

```text
┌───────────────────────────── UI（同一份静态构建） ─────────────────────────────┐
│                                                                                │
│   Tauri WebView                                         Browser                │
│       │                                                     │                  │
│ TauriWorkbenchTransport                           WebWorkbenchTransport         │
│ invoke + Tauri events                              HTTP + WebSocket            │
│       │                                                     │                  │
└───────┼─────────────────────────────────────────────────────┼──────────────────┘
        │                                                     │
        └──────────────────┬──────────────────────────────────┘
                           ▼
                  rambledesk-core application
                           │
                  rambledesk-storage / SQLite

Platform Capability / Plugin（由客户端持有）
  ├─ Shortcut Capability
  ├─ Capture Plugin
  ├─ Speech Recognition Plugin
  ├─ Attachment Ingress Capability
  └─ Window Integration Capability
```

这保持 [ARCHITECTURE.md](ARCHITECTURE.md) 的原则：`rambledesk-core` 不持有 HTTP/Tauri，Desktop 继续作为 composition root；新增 Web router 只是 application contract 的第二个 transport adapter。

### 6.2 不要直接扩宽现有 local server

当前术语中的 Local Server 是认证 loopback 集成边界，服务 Generic MCP 与 JSON API。新的 Web Service 面向人类浏览器、托管静态 UI、具有可选启停与不同的认证/Origin/可用性要求。两者可以复用 `rambledesk-core`、storage 和部分监听基础设施，但不应默认成为同一个安全 profile。

建议明确两份配置与生命周期：

| 边界 | Local Server | Web Service |
| --- | --- | --- |
| 使用者 | Generic MCP host / Pi adapter | 人类浏览器 |
| 默认可见性 | loopback，随 Desktop 启动 | 默认关闭或 loopback，可由 Desktop 启停 |
| token | 集成 token | 独立 Web access token/session |
| 路由 | MCP + integration JSON API | 静态 UI + Web allowlist API + WS |
| 状态 | Desktop 内部集成基础设施 | 设置页可见：stopped/starting/running/stopping/error |
| LAN | 不改变 | 后续显式 opt-in，并伴随 HTTPS 风险提示 |

特别不要通过把现有 listener 从 `127.0.0.1` 改成 `0.0.0.0` 来“完成 Web 版”。这会无意扩大 MCP/集成 API 的暴露范围，并把两套认证语义绑死。

### 6.3 建议的前端合同

第一阶段可以定义比 Codeg command-string 更收敛的业务合同：

```ts
interface WorkbenchClient {
  requests: RequestQueries & RequestMutations;
  drafts: DraftQueries & DraftMutations;
  attachments: AttachmentCommands;
  subscribe(listener: WorkbenchInvalidationListener): Unsubscribe;
  capabilities(): RuntimeCapabilities;
}
```

`WorkbenchClient` 的 Tauri 与 Web 实现可以共享 DTO/error code，但 endpoint 应按浏览器核心旅程显式 allowlist；不要把所有 Tauri command 自动暴露到 HTTP。

`RuntimeCapabilities` 至少应描述：

- `workspaceShortcut`；
- `systemGlobalShortcut`；
- `nativeScreenshot`；
- `browserDisplayCapture`；
- `audioCapture`；
- `directLocalPath`；
- `notifications`。

组件依据 capability 决定入口是否可用；Transport 内部不判断截图或录音能力。

## 7. 两个版本的建议拆分

### 7.1 版本 A：一套 UI，显式平台边界

目标是证明 Desktop 在不回归的情况下已经能通过抽象边界运行，而不是立即发布 Web 地址。

建议 PR 顺序：

1. **定义 Workbench DTO、错误与 BackendTransport 合同。** 为现有 Tauri commands 建立类型化 client，不改变行为。
2. **收口 UI 中的 Tauri 直接依赖。** 请求、草稿、附件、设置通过 TauriWorkbenchTransport；保留少量 composition root。
3. **建立 Platform Capability / Plugin Implementation。** 快捷键、截图、录音、文件与窗口能力从业务 Module 移出，并加入 capability matrix。
4. **让共享 UI 可在普通浏览器构建。** 不要求此时拥有完整后端；保证 SSR/static build 不直接访问 Tauri globals。
5. **静态构建与 Desktop bundle 合并。** 同一前端产物供 WebView 与后续 Web server 使用。

版本 A 的验收重点：现有 Desktop 全局热键、截图、录音/ASR、Feedback Draft 恢复与提交行为不变；普通浏览器 build 不因 Tauri import 崩溃；平台分支集中在 adapter/composition 层。

### 7.2 版本 B：Desktop 可选 Web 服务

建议 PR 顺序：

1. **Web Service lifecycle 与设置。** 独立 port/token/auto-start，默认 loopback；Desktop-only Start/Stop/status/address UI；退出时可靠停服。
2. **同源静态资源与最小 HTTP API。** 只开放请求列表、请求详情、草稿、附件与提交等核心旅程；handler 调用同一 application service。
3. **WebTransport 与浏览器登录。** Bearer HTTP；WS 使用 subprotocol token 或换成登录后短期 HttpOnly session，必须有明确威胁模型。
4. **轻量跨客户端同步。** WS invalidation + reconnect refetch；沿用 draft revision/CAS，不先做 replay ring buffer。
5. **浏览器附件入口。** 图片粘贴、拖放、文件上传；服务端统一生成 attachment metadata。
6. **浏览器本地音频采集与 ASR。** 先在 loopback secure context 中验证 AudioWorklet、Worker、sherpa-onnx WASM 与模型缓存；不新增音频上传或服务端识别协议。
7. **LAN opt-in 与配对体验。** 明确监听范围、地址/二维码、失败诊断、HTTPS/tunnel 指引，再承诺跨设备访问。

## 8. 第一阶段不应直接照搬 Codeg 的复杂度

1. **不做第三种 RemoteDesktopTransport。** 第一阶段只需要本机 Tauri IPC 与浏览器 HTTP/WS。
2. **不同时产品化独立 headless server、Docker、supervisor、自更新。** 先让 Desktop 可选启停 Web 服务。
3. **不镜像全部 Tauri command。** 以浏览器用户旅程形成最小 endpoint allowlist，避免 1600 行 Router 式 parity 负担。
4. **不先实现 snapshot/replay/seq/ring-buffer/lag-detach 完整协议。** HTTP 真值 + WS invalidation + reconnect refetch 足以起步。
5. **不复制 `0.0.0.0 + CORS Any + localStorage 长期 token` 默认组合。** 默认 loopback、同源、独立 Web credential；LAN 必须显式开启。
6. **不为 UI 再引入 SSE。** Codeg 的 SSE 是 Office preview 特例，不是双端 UI Transport 的组成部分。
7. **不承诺 Web 与 Tauri 本机能力完全对等。** 系统全局快捷键、区域截图 overlay、钉图等应明确 Desktop-only。
8. **不让服务端替远程浏览器捕获“客户端屏幕/麦克风”。** 服务端本机能力作用于 Desktop host，浏览器 API 作用于浏览器设备；设备所有权必须进入合同。
9. **不在可信 HTTPS 方案前承诺 LAN 实时录音。** 裸 LAN HTTP 不满足浏览器媒体 API 的安全上下文要求。
10. **不在组件中扩散 `isDesktop()`。** Transport 和 capability 应分别封装，页面只消费业务 client 与 capability。

## 9. 决策清单

进入实现前需要明确以下架构决策：

- Web Service 与 Local Server 是否使用两个 listener；本报告建议“是”。
- Web Service v1 默认关闭还是自动 loopback；本报告建议“默认关闭、用户启动后 loopback”。
- LAN 是否进入版本 B 首发范围；本报告建议“可以作为实验性 opt-in，但不与录音/主动截屏同批承诺”。
- HTTP API 是 command parity 还是业务资源/用例 allowlist；本报告建议“allowlist”。
- WS 第一版传 delta 还是 invalidation；本报告建议“invalidation + revision”。
- 浏览器凭证是长期 bearer token 还是短期 session；需单独安全 ADR。若先用 token，至少与 MCP token 分离。
- Web ASR 在浏览器本地使用 streaming 还是 VAD + offline；先以 X-ASR streaming 做 feasibility gate，再按真实设备结果决定后续模型。
- 静态前端是否保持单一构建产物；本报告建议“是”，避免 Desktop/Web 页面分叉。

## 10. 一手资料索引

### Codeg 官方资料

- [Architecture](https://docs.codeg.app/reference/architecture)
- [Web Service settings](https://docs.codeg.app/reference/settings/web-service)
- [Deployment](https://docs.codeg.app/getting-started/deployment)
- [Configuration](https://docs.codeg.app/getting-started/configuration)
- [Privacy & Security](https://docs.codeg.app/reference/privacy)
- [固定研究快照 `46636d7`](https://github.com/xintaofei/codeg/tree/46636d758ebd66ae4b29787b4206491c4a1fee03)

### 平台官方资料

- [Tauri Global Shortcut plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [MDN `MediaDevices.getDisplayMedia()`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getDisplayMedia)
- [MDN `MediaDevices.getUserMedia()`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)
- [MDN AudioWorklet](https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API/Using_AudioWorklet)
