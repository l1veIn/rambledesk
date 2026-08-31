# RambleDesk UI Workspace 与 Web 架构重构评估

> 日期：2026-09-01
> 状态：提案，供主分支分阶段实施
> 范围：先完成 Workspace UI，再完成前后端分离与 Web Client；本文不包含 ACP 实现

## 结论

这两个版本方向成立，而且应该拆成多批小 PR 推进。

建议的总路线是：

1. **Workspace UI 版本**：先把左栏改成平铺 Session，再建立右侧多 Tab Workspace；保留已经打磨好的请求列表、反馈正文、附件、语音、截图和提交链路。
2. **Client / Server 版本**：把现有 `rambledesk-local-server` 加深为 Web Server，让 Desktop Client 和 Web Client 通过同一套 ApplicationTransport Interface 访问同一个 Rust Backend Runtime；Desktop 使用 Tauri IPC Implementation，Web 使用 HTTP / WebSocket Implementation。
3. **ACP 版本**：Agent 进程、Session runtime、Permission / Ask / Feedback、Timeline 都落在 Server 一侧，再由 Desktop 与 Web 共用同一条事件流。

推荐基线是 **16 个 PR、约 40 个可独立审查的 commit**：Workspace UI 6 个 PR，Client / Server 与 Web 10 个 PR。实际合并时允许微调，但不应把两个版本压成一两个巨型 PR。

不建议现在引入新的前端框架。现有 Svelte、Vite、Bits UI、PaneForge 已足以完成 Workspace Shell。SvelteKit 会额外引入一套 JavaScript server runtime，与 Rust Backend Runtime / Web Server 职责重叠；Dockview 一类 docking framework 只有在真正需要分屏、拖拽停靠、跨窗口布局时才值得引入。

## 一、当前基础并不差

这次不是从零拆前后端。主分支已经具备三个关键基础：

- `rambledesk-core` 已经持有独立于 Tauri、HTTP、MCP 的 application contract。
- `rambledesk-storage` 已经是独立 SQLite / Package Implementation。
- `rambledesk-local-server` 已经是 Axum Server，具备 bearer token、Host / Origin guard、`/mcp`、`/api/feedback/*` 和 loopback listener。

因此正确动作不是再创建第二套后端，而是逐步把现有 local server **加深**为 Web Server，并让它与 Tauri IPC 共享同一个 Backend Runtime。Desktop 不必为了形式上的“前后端分离”绕道 HTTP；关键是 UI 不再直接依赖具体 transport，两种 transport 也不复制业务规则。

当前真正的耦合集中在前端：多处 Svelte 文件直接调用 Tauri `invoke` 或监听 Tauri event。最重的区域包括 Session controller、设置、附件、引导、截图、权限、更新器和浮动 Ramble Console。`pnpm build:web` 目前只证明 SPA 能构建，不代表浏览器版具备完整业务能力。

## 二、目标架构

```text
                           RambleDesk Backend Runtime
                    ┌────────────────────────────────────┐
                    │ Core application + Storage         │
                    │ Request / Draft / Package / Config │
                    │ Session runtime + Timeline (ACP 后) │
                    └───────────────┬────────────────────┘
                                    │
                          ApplicationTransport
                 ┌──────────────────┴──────────────────┐
                 │                                     │
        Tauri IPC Implementation             HTTP + WebSocket Implementation
                 │                                     │
        Desktop Client                          Browser Web Client
     Shared Svelte Workbench                  Shared Svelte Workbench
                 │                                     │
      Tauri Capability Adapters              Browser Capability Adapters
  window / shortcut / native capture       media / clipboard / download
  dialog / tray / updater / notify         browser capture / notification
```

这里有两个关键 Seam：

### 1. WorkspaceShell Interface

Workspace Shell 只管理“用户当前打开了哪些视图”，不拥有业务事实。建议最小 Interface：

```ts
open(view)
focus(viewKey)
close(viewKey)
closeOthers(viewKey)
restore(snapshot)
```

`WorkspaceView` 使用有类型的联合结构：

```ts
type WorkspaceView =
  | { kind: 'session'; sessionKey: string }
  | { kind: 'settings'; section?: string }
  | { kind: 'profile'; profileId: string }
  | { kind: 'task'; requestId: string }
  | { kind: 'timeline'; sessionKey: string }
```

同一个 Session 最多一个 Tab。关闭 Tab 只关闭视图，不归档 Session、不删除数据、不停止 Agent runtime。

### 2. ApplicationTransport Interface

Shared Workbench 不应知道自己运行在 Tauri 还是浏览器中。业务访问统一经过：

```ts
call<T>(command, input): Promise<T>
subscribe(stream, handler): Unsubscribe
waitUntilReady(): Promise<void>
capabilities(): CapabilityManifest
```

最终保留 `TauriTransportAdapter` 与 `HttpTransportAdapter` 两个 Implementation：前者把 `call/subscribe` 映射到 Tauri invoke/event，后者映射到 HTTP/WebSocket。两者调用同一个 Rust application Module；UI 业务代码不得直接 import Tauri。窗口、全局快捷键、原生截图等设备能力继续使用独立的 Capability Adapter。

## 三、Workspace UI 重构评估

### 3.1 左侧 Session Rail

第一步可以严格按当前产品约束做成一个非常小的 PR：

- 取消按 Host 折叠；
- Session 直接平铺；
- 每个 Session 左侧显示 Host logo；
- 完整保留搜索、全部请求、重命名、归档、置顶和现有选择逻辑；
- 不触碰中栏请求列表和右侧反馈工作区。

这一 PR 不需要新的状态框架，也不应该顺手引入 ACP 字段。

### 3.2 多 Tab Workspace

右侧多 Tab 不是把多个现有页面全部常驻 DOM。ADR 004 已经接受“整个应用最多一个可编辑 `RichFeedbackEditor`”，因此必须保持：

- Tab descriptor 与后台 Session 状态可以同时存在；
- 只有当前激活的 Session Workbench 挂载可编辑器；
- 切换 Session Tab 时先保存当前 Draft，再卸载并加载目标 `document_json`；
- 后台 Ramble 继续通过 request id、JSON transformation、串行队列和 CAS 写 Draft，不依赖隐藏 Editor。

这条约束比选用哪一种 Tab 库更重要。若为了“保活页面”重新引入 per-session Editor，会直接重演 0.3.3 RC 已经否决的复杂度。

### 3.3 是否需要 Router 或全局状态框架

当前不需要。

- Tab bar、键盘导航和可访问性可继续使用 Bits UI。
- Workspace state 是一个小型前端 Module，用 Svelte store / rune 即可。
- Tab snapshot 可以先存在 Client 本地；业务事实仍由 Server / SQLite 持有。
- Browser URL deep link 可以在 Web Client 成型后增加一个很薄的 History Adapter，不必提前把 Workspace 变成传统页面路由。

只有在未来确认需要“多组分屏、拖拽停靠、跨窗口浮动、布局序列化”时，再评估 Dockview。当前目标只是一个 Tab strip，提前引入 docking framework 会扩大状态面和视觉改造范围。

## 四、前后端分离评估

### 4.1 “Web 服务”应与 Backend Runtime 分开理解

RambleDesk 的业务 Runtime 会随 Desktop 存在，但浏览器 listener 可以独立启停。建议区分：

- **Backend Runtime**：Core、Storage、配置和未来 ACP runtime；Desktop 启动后始终存在。
- **Local Integration Server**：现有 MCP / Adapter 使用的 authenticated loopback listener。
- **Web Access**：是否启动供浏览器访问的静态资源、HTTP API 和 WebSocket listener；这是用户可以启停和配置的功能。

Web Access 与 Local Integration Server 可以复用同一个 server crate 和 router Module，但生命周期和 route set 应明确。第一版 Web Access 只支持本机浏览器访问。LAN 访问应作为后续显式能力，不应第一天就绑定 `0.0.0.0`。独立 headless `rambledesk-server` 也应延后，不与内嵌 Web Access 同期产品化。

### 4.2 Command、查询与事件

推荐首期 transport：

- JSON HTTP：命令、查询、附件上传下载、设置；
- WebSocket：请求状态、Draft revision、Session 状态等轻量 invalidate / delta；
- 浏览器录音首期通过 HTTP 上传分段或完成后的 Blob；只有实时 partial transcript 成为明确需求时，才增加独立 binary WebSocket。

事件流从一开始必须定义 snapshot / reconnect，而不是假定广播永不丢失：

1. 页面初次进入先通过 HTTP 读取完整 snapshot；
2. WebSocket 只用于提示局部变化或使某份 snapshot 失效；
3. reconnect 后统一 refetch 相关事实状态；
4. WebSocket ready 后再发会产生事件的 command，避免订阅建立前漏事件；
5. UI 永远能从 Backend Runtime 重新建立事实投影。

Codeg 的 `Transport`、Web transport 以及 subscribe-with-snapshot 实现证明这条 Seam 是必要的；它同时暴露了“WebSocket 尚未 ready 时 command 已经产生事件”“断线期间 broadcaster 没有 receiver 导致事件丢失”等真实竞态。RambleDesk 应借鉴其 ready / reconnect / snapshot 思路，但首期不必照搬 sequence ring buffer、gap replay、lag-detach 和多 subscription multiplex。等 ACP Timeline 出现高频、长时间运行事件后，再为该类 stream 增加有范围的 sequence / replay 协议。

### 4.3 多 Client 并发

Desktop 与 Browser 同时打开后，Server 事实与 Client 工作区状态必须分开：

- Request、Draft、Package、Session runtime、Timeline 是 Server 事实；
- 当前打开的 Tab、Tab 顺序、侧栏宽度是 Client 本地状态；
- Draft 更新继续使用 revision / CAS；冲突必须可见，不允许 last-write-wins 静默覆盖；
- 同一 Permission / Ask / Feedback 的终态操作必须幂等；
- 关闭浏览器、关闭 Tab 或刷新页面都不能自动结束 Agent runtime。

## 五、原生与浏览器能力矩阵

| 能力 | Desktop Client | Web Client | 推荐归属 |
| --- | --- | --- | --- |
| Request / Draft / Package / Settings | 完整 | 完整 | Backend Runtime |
| Session / Timeline / ACP runtime | 完整 | 完整 | Backend Runtime |
| 全局快捷键 | 完整 | 只能处理页面聚焦时按键 | `ShortcutCapability` |
| 截图/图片 | 原生屏幕、窗口、区域、长截图、贴图 | 首期粘贴/上传；后续可用用户手势触发屏幕共享 | `CaptureCapability` |
| 剪贴板 | 可做系统级集成 | 受 secure context、权限和用户手势限制 | `ClipboardCapability` |
| 文件选择 | 原生文件/目录对话框 | 浏览器选择的是 Client 文件 | `FileCapability` |
| Server 工作目录 | 本机可用原生对话框 | 必须使用 Server-side folder browser / path picker | Server Interface |
| 录音 | Tauri / native microphone source | `getUserMedia` + MediaRecorder | `AudioCapture` Adapter |
| ASR | 本地 microphone + server-side engine | 音频上传到同一 engine | `SpeechEngine` Module |
| 系统通知 | 原生通知 | Web Notification，best effort | `NotificationCapability` |
| 更新器、Tray、窗口、系统权限 | 完整 | 不提供 | Desktop-only Capability |

浏览器截图和录音不能伪装成 Desktop 的等价实现：`getDisplayMedia()` 每次都需要用户选择共享源和瞬时用户操作；`getUserMedia()`、Clipboard 等能力依赖 secure context 与权限。远程浏览器捕获的是浏览器所在设备，而不是运行 RambleDesk Server 的设备。

### 5.1 录音与 ASR 的拆分

现有 `rambledesk-speech` 已有事件、重采样、VAD 和识别引擎资产，但 native microphone acquisition 与 recognizer 生命周期仍耦合。建议拆成：

- `AudioCapture` Interface：产出有格式说明的 audio chunk / blob；
- `NativeAudioCapture`：使用当前 cpal 采集，并可继续给本地 engine 提供 PCM；
- `BrowserAudioCapture`：使用 `getUserMedia` 与 `MediaRecorder`，按浏览器实际支持格式上传；
- `SpeechEngine`：在后端统一解码、resample、VAD / ASR，并发出统一 SpeechEvent。

不建议把 Web Speech API 作为主路径，因为浏览器实现、网络依赖、隐私和转写一致性都难以成为稳定产品合同。

## 六、安全模型

现有 local server 已经有很好的起点：随机 bearer token、常量时间比较、Host / Origin guard、loopback listener。Web 化后需要补全：

- Web Access 默认关闭；第一阶段只监听 `127.0.0.1`；
- HTTP 使用 `Authorization: Bearer`；
- Browser WebSocket 无法任意设置 Authorization header，可参考 Codeg 将一次性约定的 token 放入 `Sec-WebSocket-Protocol`；
- 不把 token 放 URL query，避免进入浏览历史、日志和 referrer；
- 静态资源尽量与 API 同源，严格校验 Origin / Host，防 DNS rebinding；
- 设置 body size、上传大小、速率和连接数上限；
- 安装 Agent、选择 Server 工作目录、打开/显示 Server 文件等敏感命令需要单独分类与授权；
- LAN 访问必须使用 HTTPS / WSS，或明确要求可信反向代理。普通局域网 HTTP 会暴露 token，同时浏览器的麦克风、剪贴板等能力在非 secure context 中不可用。

设置界面可以参考 Codeg 的“端口、访问 Token、自动开启、运行状态、访问地址、复制、二维码、打开”，但文案应使用 **Web Access**，不能让用户误以为停止它会停止 RambleDesk 后端。

## 七、PR 与 commit 路径

原则：每个 commit 必须能构建、能解释自己的回退方式；每个 PR 只建立一个主要 Seam 或迁移一类能力。迁移期间允许 Adapter 共存，但不允许长期维护两套业务规则。

### 版本 A：Workspace UI（6 PR，约 13 commits）

#### UI-1：平铺 Session Rail（1 commit）

1. `Flatten session rail and preserve existing session actions`

验收：Host 分组消失；每项有 Host logo；搜索、全部请求、选择、重命名、归档、置顶不退化；中栏和右栏无行为变化。

#### UI-2：抽取现有 Workbench 视图（2 commits）

1. `Extract session workbench without behavior changes`
2. `Add view descriptors and stable view keys`

验收：仍然只有一个 Session 视图；视觉与原版一致；现有 editor / attachment / speech 测试不变。

#### UI-3：建立 WorkspaceShell（2 commits）

1. `Introduce WorkspaceShell interface and reducer tests`
2. `Render the current session through WorkspaceShell`

验收：open / focus / close / deduplicate 都有单元测试；尚不开放多 Tab 时仍与旧行为等价。

#### UI-4：多 Session Tabs（3 commits）

1. `Add tab strip with keyboard navigation and overflow`
2. `Open and focus sessions from the sidebar`
3. `Preserve the single-editor save-unmount-load contract`

验收：同一 Session 不重复开 Tab；切换不串 Draft；关闭 Tab 不归档、不终止后台 Ramble；Tab 可键盘操作。

#### UI-5：Workspace 恢复与异常处理（2 commits）

1. `Persist client-local workspace snapshots`
2. `Recover missing archived and deleted session views safely`

验收：重启可恢复 Tab；目标 Session 不存在时出现可关闭的恢复页；Draft 保存失败时阻止危险切换并给出明确错误。

#### UI-6：把非 Session 页面接入 Tab（3 commits）

1. `Open settings as a singleton workspace view`
2. `Open task and profile views through workspace adapters`
3. `Remove superseded full-screen and modal navigation state`

验收：设置、Task、Rambelle Profile 能作为 Tab 打开；Settings 仍可在需要原生确认时使用子对话框；不重复实现正文工作区。

### 版本 B：Backend Runtime 与 Web Client（10 PR，约 27 commits）

#### WEB-1：术语与 ADR（2 commits）

1. `Define Workbench Client Backend Runtime and Web Access terminology`
2. `Record transport capability and security decisions`

先修改 `TERMINOLOGY.md`，再更新 Architecture / ADR。建议引入：Workbench Client、Desktop Client、Web Client、Backend Runtime、Local Integration Server、Web Access、Desktop Shell、Native Capability、Browser Capability。

#### WEB-2：ApplicationTransport Seam（3 commits）

1. `Define typed application command and error contracts`
2. `Add ApplicationTransport interface and test adapter`
3. `Route existing UI business calls through a Tauri adapter`

验收：UI 不再直接知道 command 名称散落在哪里；行为尚未切 HTTP；不改业务规则。

#### WEB-3：加深现有 local server（3 commits）

1. `Expose read and list application operations over HTTP`
2. `Expose draft mutation submit and cancel operations over HTTP`
3. `Add attachment streaming and transport contract tests`

验收：HTTP handler 只做认证、解析、DTO / error mapping；领域规则仍在 core；Tauri 与 HTTP contract test 对同一用例给出等价结果。

#### WEB-4：Transport parity 与直接依赖清理（2 commits）

1. `Add authenticated HttpApplicationTransport`
2. `Move desktop UI calls behind TauriApplicationTransport`

验收：Desktop 继续完整工作；普通 UI Module 不再直接 import Tauri；Tauri command 与 HTTP handler 都是同一个 application Module 的薄 Adapter；两种 transport 有 parity contract tests。

#### WEB-5：首期事件流（3 commits）

1. `Define snapshot invalidation and ready contracts`
2. `Add authenticated WebSocket events and health probing`
3. `Refetch application snapshots after reconnect`

验收：订阅尚未 ready、断线重连、漏 invalidate、Server 重启均有测试；Client 总能 refetch 回事实状态。sequence replay 延后到出现 ACP Timeline 等高频 stream 时再设计。

#### WEB-6：本机 Web Client（3 commits）

1. `Serve the shared Workbench SPA from Web Access Server`
2. `Add first-visit token authentication and browser bootstrap`
3. `Enable request draft attachment and package flows in the browser`

验收：本机浏览器可完成完整文字反馈闭环；刷新和重连不丢 Draft；尚不开放 LAN。

#### WEB-7：Capability Interfaces（3 commits）

1. `Define capability manifest and focused capability interfaces`
2. `Move native operations behind Tauri capability adapters`
3. `Add browser adapters and explicit unavailable states`

验收：视图中不散布 `isTauri` 分支；Web 不显示虚假的全局快捷键、Updater、Tray 或 Server 文件对话框。

#### WEB-8：Web 图片与文件语义（2 commits）

1. `Add browser image paste and upload to the attachment flow`
2. `Separate client file upload from server workspace selection`

验收：浏览器粘贴或上传的截图可进入现有编辑器与附件管线；远程 Client 文件与 Server path 不混淆。主动屏幕共享作为后续 `BrowserCaptureCapability`：只能通过用户手势调用 `getDisplayMedia()`，不宣称等价于 Desktop 全局区域截图。

#### WEB-9：Web 录音与 ASR（3 commits）

1. `Separate AudioSource from SpeechEngine`
2. `Add browser audio upload and server-side recognition sessions`
3. `Add BrowserAudioSource and preserve desktop speech behavior`

验收：Desktop 与 Web 产生相同 SpeechEvent 合同；浏览器通过 `getUserMedia` 采集并协商浏览器支持的格式；断线、停止、权限拒绝、设备丢失不会留下幽灵 recording session。首期可先做分段/结束后上传；只有实时 partial transcript 成为明确验收项时才增加 PCM streaming binary channel。

#### WEB-10：Web Access 产品化与清理（3 commits）

1. `Add Web Access settings status token and address controls`
2. `Add loopback security limits diagnostics and acceptance tests`
3. `Remove direct UI transport calls and run residual architecture scans`

验收：用户能启动/停止 Web Access、复制 Token、打开本机地址并看到明确状态；Backend Runtime 与 Local Integration Server 不被误关；普通 UI 不再直接调用 invoke，所有调用都经 ApplicationTransport 或明确列出的 native Capability Adapter。

## 八、每一阶段的质量闸门

### UI 闸门

- 单 Editor 不变量有自动化测试；
- Session 切换必须保存当前 Draft，再加载目标 JSON；
- 搜索、归档、重命名、置顶、全部请求不退化；
- Tab close 与 Session archive / runtime stop 语义完全分离；
- 原生 UI 仍需人工视觉验收，静态检查不能替代。

### Transport 闸门

- core 不依赖 HTTP、WebSocket、Tauri；
- HTTP 与 Desktop 走同一 application use case；
- typed DTO 与 error code 有 contract tests；
- reconnect 通过 snapshot / invalidation / refetch 恢复；
- 没有 silent fallback 和双写。

### Web 闸门

- 本机文字反馈闭环先于截图和语音；
- Chrome / Safari 至少覆盖登录、Draft、附件、提交、刷新恢复；
- insecure context 下明确禁用麦克风、截图或剪贴板能力；
- 第一版不把 listener 暴露到局域网；
- Token 不进入 URL、日志、反馈包和浏览器持久明文存储。

## 九、主要风险与控制

| 风险 | 等级 | 控制方式 |
| --- | --- | --- |
| 多 Tab 破坏单 Editor 所有权 | 高 | descriptor 常驻、Editor 仅 active view 挂载；以 ADR 004 为硬闸门 |
| Desktop / Web 形成两套业务实现 | 高 | Tauri IPC 与 HTTP 都只是同一 application Module 的薄 Transport Adapter |
| 断线漏掉 Request 状态事件 | 高 | snapshot + invalidate + reconnect refetch + ready gate；高频 Timeline 后续再加 scoped replay |
| Browser 与 Server 文件系统语义混淆 | 高 | client upload 与 server workspace picker 分开建模 |
| LAN token 被明文截获 | 高 | 首版 loopback；LAN 必须 HTTPS / WSS 或可信代理 |
| Web ASR 延迟和断线状态复杂 | 高 | 首期 Blob/分段上传、明确 recognition session、最后实施；实时流按需增加 |
| 前端框架重构吞噬产品开发 | 中 | 保留 Svelte / Vite；只引入两个深 Interface |
| 设置页“停止 Web”误杀 Desktop 后端 | 中 | Backend Runtime 与 Web Access 两个术语、两个生命周期 |

## 十、最终收敛点

完成这两个版本后，主分支应满足：

1. 左侧是全量 Session 目录，右侧是可恢复的多 Tab 工作区；
2. 原有请求列表与反馈工作流完整保留；
3. Desktop 与 Browser 共用同一套 Svelte Workbench 和 ApplicationTransport Interface，分别使用 Tauri 与 HTTP/WebSocket Implementation；
4. 所有领域事实只在 Core / Storage / Backend Runtime 中实现一次；
5. Tauri 被收敛为 Desktop Shell 与原生 Capability Adapter；
6. Web Access 可在本机安全开启，完整支持文字、附件、图片与录音反馈；主动屏幕共享可作为后续浏览器 Capability；
7. 首期事件流具有 snapshot / invalidate / reconnect-refetch 语义，并为未来高频 stream 保留 scoped replay 扩展点；
8. ACP 可以作为 Server 侧的新 runtime Module 接入，而不需要再重写 UI、持久化或远程访问层。

这时 ACP 的工作会从“同时重构产品、UI、运行时与通信”缩小为：实现 ACP Client Module、Session runtime、三类结构化请求以及 Timeline projection。它仍然是重要版本，但不再需要承担基础架构迁移。

## 参考

- [RambleDesk 当前架构基线](ARCHITECTURE.md)
- [ADR 004：单 Editor 结构化 Feedback Draft](adr/004-single-editor-structured-draft.md)
- [Codeg 专项调研](CODEG_WEB_SERVICE_RESEARCH.md)
- [Codeg Transport Interface（固定快照）](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/types.ts)
- [Codeg Web Transport（固定快照）](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src/lib/transport/web-transport.ts)
- [Codeg WebSocket Authentication（固定快照）](https://github.com/xintaofei/codeg/blob/46636d758ebd66ae4b29787b4206491c4a1fee03/src-tauri/src/web/auth.rs)
- [MDN: `getDisplayMedia()`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getDisplayMedia)
- [MDN: `getUserMedia()`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)
- [MDN: Clipboard API](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard_API)
- [MDN: WebSocket constructor and subprotocols](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/WebSocket)
