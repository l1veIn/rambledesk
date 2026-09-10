# Web Access 支持矩阵

> 状态：当前代码能力与人工待验边界；本文整理没有重新运行设备验收。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。
> 本表只陈述已经进入仓库的能力、仍需人工验收的能力和明确不支持的能力。

## 状态标记

- **Automated**：存在自动化 contract、unit 或 loopback black-box test。它证明代码合同，不等于真实
  操作系统、浏览器权限、麦克风或长会话已经人工验收。
- **Manual**：必须在真实 Desktop、Chrome 或 Safari 中人工验证；未完成时不得写成兼容承诺。
- **Unsupported**：当前产品不提供；若标记 Deferred，表示以后可以重新立项，不是排期承诺。

## Workbench 能力

| 能力 | Desktop Client | Browser Client（Web Access） | 当前证据与边界 |
| --- | --- | --- | --- |
| Requests、Host Sessions、列表与详情投影 | **Automated** | **Automated** | Tauri 与 HTTP Application Transport conformance、Web ready/refetch 与 session auth 已覆盖。 |
| TipTap Feedback Draft、autosave 与 revision/CAS | **Automated**；发布前仍做 **Manual** 编辑回归 | **Automated**；发布前仍做 **Manual** 多标签页/重连回归 | 两端使用同一 `document_json` 真源与 application mutation；浏览器不是第二份 Draft。 |
| 文件上传与图片粘贴 | **Automated**；原生文件选择做 **Manual** 回归 | **Automated**；浏览器文件 input 与 DOM image paste 做 **Manual** 浏览器回归 | 候选先经过 Capture Plugin/Attachment Candidate seam，再由 application mutation 持久化；浏览器文件不是服务器路径。 |
| Submit 与 published feedback 下载 | **Automated** | **Automated** | 两种 Transport 共享 terminal mutation、不可变 package 与安全下载投影。 |
| 语音识别 | Desktop native path 为 **Automated**；真实设备/权限为 **Manual** | Browser local ASR pilot 的模型下载、hash/cache、Wasm/Worker/AudioWorklet 合同和 recognizer creation 为 **Automated**；真实 Chrome/Safari 麦克风授权、PCM 输入、稳定出字、停止 flush 与长会话仍为 **Manual / unverified** | Browser 音频不上传 Backend Runtime；当前 pilot 使用本地 sherpa-onnx WebAssembly。自动创建 recognizer 不能替代真实浏览器验收。 |
| 系统截图、滚动截图、overlay 与 pin | **Automated** contract；真实 OS 交互为 **Manual** | **Unsupported（Deferred）** | Browser screen capture 尚未交付；不得用 `getDisplayMedia()` 的理论可用性冒充现有能力。 |
| 全局快捷键 | **Automated** contract；真实 OS 注册为 **Manual** | **Unsupported** | 浏览器只处理页面内用户手势，不模拟系统全局快捷键。 |
| Tray、updater、系统权限与系统路径/原生对话框 | **Automated** contract；安装包行为为 **Manual** | **Unsupported** | 这些属于 Desktop Shell / Native Capability，不属于 Application Transport。 |
| Web Access autostart | 设置与启动合同为 **Automated**；真实启动顺序为 **Manual** | 服务管理属于 Desktop | 默认关闭；用户可选择随 Desktop 启动开启 Web Access，失败必须在状态与诊断中可见。 |
| Web Access 端口配置 | **Automated** contract | 服务管理属于 Desktop | 默认 `127.0.0.1:37643`，可设为 1024–65535；下次启动服务生效，端口占用时不静默切换。 |

## Web Access 运行边界

- Web Access **默认关闭**。启停与可选自启动都由 Desktop 设置管理。
- 默认入口是 `http://127.0.0.1:37643`，可在浏览器服务设置中改为 1024–65535 的端口；listener 始终只绑定 IPv4 loopback。
- Web Access 与 Local Integration Server 使用不同 listener、credential、auth domain、route set 和
  lifecycle。停止 Web Access 不停止 Backend Runtime，也不停止 Local Integration Server。
- Web Access durable token：macOS/Linux 使用 Tauri `app_local_data_dir()/auth/web-access.token`，
  按应用 identifier 隔离，独立于资料库；Unix `auth` 目录 `0700`、文件 `0600`，原子创建/轮换，
  自动加载遇到 symlink、损坏内容或私有文件条件不满足时失败。用户显式 Refresh token 并确认后，
  可重新生成令牌恢复内容损坏的自有普通文件，仍不绕过路径、类型和归属检查。Windows 保留 Credential Manager。
  token 不进入通用配置、SQLite、日志、诊断或应用生成的备份/导出包；私有文件不等于加密存储。
- 浏览器必须通过 same-origin `POST /api/auth/session` 用 durable credential 换取 scope 受限的 session；
  该 session token 只存在 JavaScript 内存与 `HttpOnly; SameSite=Strict` 且仅作用于 bootstrap 路径的
  cookie 中，刷新、新标签页与浏览器重启会凭 cookie 恢复同一个 session，不需要重新粘贴 token。
  HTTP 使用 session bearer，WebSocket 使用受约束的 subprotocol credential。
- 自动化覆盖 bootstrap/HTTP/event/session/body 限制、event 连接预算恢复、body 超限无 mutation
  副作用，以及 Web Access 停止后 Local Integration 仍可写。

2026-09-10 的存储修订不读取、自动迁移或删除旧 macOS Keychain / Linux Secret Service 条目。
这两个平台升级后首次启用生成新的 Web token，需从 Desktop 设置重新复制到浏览器；Windows
保持既有凭据存储。之后同一运行中的 Web Access 可继续用有效 cookie 恢复 session；停止服务、
重启 Runtime 或轮换 token 仍撤销旧 session。新存储选择本身不是重新认证 UI 或平台实测的通过证据，
实际结果继续在下方质量记录中分项登记。

## 响应式 Workbench 合同

这些是同一 Workbench 的布局行为，手机仍处于 dogfooding 范围，**不构成手机浏览器兼容承诺**。
视口模拟不能替代真实触屏、软键盘、旋转和安全区验收。

| 模式 | CSS 视口宽度 | 导航与请求列 | 宽度调整 |
| --- | --- | --- | --- |
| desktop | ≥ 1024px | 常驻双列，可折叠 | 可拖动 |
| tablet | 768–1023px | 常驻双列，按容器预算收窄 | 可拖动 |
| phone | < 768px | 两个互斥抽屉，正文占满可用宽度 | 不渲染拖动手柄 |

断点唯一来源为 [mediaQuery.ts](../apps/desktop/src/lib/mediaQuery.ts)，
[shellMode.ts](../apps/desktop/src/lib/workbench/shellMode.ts)提供模式投影。

- [ShellLayoutSession](../apps/desktop/src/lib/workbench/shellLayoutSession.ts)分别持有持久化的桌面/平板
  折叠偏好与临时的手机抽屉开合。手机操作不写回桌面偏好；变宽后恢复原有列宽与折叠状态。
- 手机打开一个抽屉会关闭另一个。入口为标题栏导航按钮和左下请求列表按钮；折叠按钮、背板与
  `Escape` 可关闭。内层弹层已经处理的 `Escape` 不继续关闭外层抽屉，不提供滑动手势关闭。
- 关闭的抽屉使用 `inert`；抽屉打开时正文使用 `inert`。打开后焦点移入抽屉，关闭后在焦点仍属于该
  抽屉时归还有效入口。标题栏操作仍可用，不把整个 Shell 描述成锁死全部外部焦点的模态对话框。
- Browser Client 是整页应用，去掉 Desktop 窗口圆角、描边和发丝边距；外观由 `environment` 决定，
  不由视口猜测平台。手机保留 `safe-area-inset-*`，浮动入口为 44 × 44px。
- 触摸指针下关键页签/导航操作常显，点击目标扩大；手机 Agent 会话选项收进单一入口。页签宽度在
  112–192px 内分配，达到最小值后横向滚动；抽屉遵循 `prefers-reduced-motion`。
- tablet 仍为双列，接近 768px 时正文会较窄。手机网络接入也没有新增产品通道：Web Access 仍仅绑定
  loopback，不提供 LAN/TLS；用户自行转发不扩大本表的安全或兼容承诺。

布局 owner、渲染、断点、焦点/Escape 和页签预算有自动化测试。已有桌面浏览器视口观察为
390 × 844、844 × 390、844 × 260；真实手机的抽屉、请求切换、软键盘、附件、旋转与安全区仍待验，
图片预览关闭后的焦点归还也需单列。结果入口见[质量清单](quality/README.md)。

## 明确不支持或不在本轮范围

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| LAN bind / 远程局域网访问 | **Unsupported / out of scope** | 不绑定 `0.0.0.0`；loopback 的 secure-context 例外不能外推到 LAN。 |
| TLS / HTTPS / WSS | **Unsupported / out of scope** | 当前没有证书、TLS proxy 或远程 credential delivery 产品合同。 |
| Headless Backend Runtime 或独立 Web deployment | **Unsupported / out of scope** | 当前 composition root 仍是 Desktop；启动 Web Access 不等于提供 headless server。 |

## 发布前人工验收

1. macOS 与 Windows Desktop：TipTap 编辑、附件、截图/overlay/pin、全局快捷键、tray、updater 与系统权限。
2. Chrome 与 Safari：Web bootstrap、重连、TipTap autosave/CAS、上传、图片粘贴、submit 与下载。
3. 真实手机：抽屉互斥与关闭、请求切换、软键盘、触控、附件预览、真实旋转、安全区与焦点归还。
   当前用户没有手机与 Windows 验收环境，两类必验项都保留，不能用 macOS 或视口模拟补齐。
4. Browser local ASR：按下面的专门矩阵记录实际浏览器、硬件和测量数据。

上述人工项未记录通过前，只能说相应代码路径和自动化门禁存在，不能声称目标浏览器或设备已兼容。

## Browser local ASR 必验项

当前 pilot 的固定模型、下载/hash/cache、同源 Wasm/Worker/AudioWorklet 和 recognizer creation 有
自动化或隔离浏览器证据。**下面全部保持 Manual / unverified**；自动创建 recognizer 不证明真实
麦克风 PCM、出字或目标设备兼容。实现取舍见 [ADR 006](adr/006-edge-media-plugins-and-tiptap-ramble-core.md)。

| 验收面 | 必须记录的真实结果 |
| --- | --- |
| 设备与浏览器 | 先记录目标 Chrome / Safari 的确切版本、操作系统、架构与音频设备；若要宣称 Edge / Firefox 支持，也分别补验，不按内核推定。 |
| 冷暖启动与离线 | 首次安装下载 bytes/time、模型逐文件校验、Wasm 初始化、warm start time；完整安装后离线重启。未通过前不展示无条件“离线可用”。 |
| 真实采集与结束 | 麦克风授权、实际 sample rate、单声道 PCM、稳定出字、停止尾帧 flush；取消、设备断开与重复 start/stop/cancel 后资源释放，已有文字保持。 |
| 延迟与背压 | real-time factor、partial/stable p50/p95 latency、丢帧数、队列峰值；录音期间编辑、插图和 autosave 仍可用。 |
| 30 / 60 分钟会话 | 峰值与稳定内存、持续出字、停止后 Worker/AudioContext/音轨释放、页面 reload 后资源回收；模型文件体积不能代替峰值内存测量。 |
| 权限与后台 | 拒绝或长期不响应权限、拔出设备、页面隐藏、AudioContext suspend/resume、刷新与重新进入；失败时保留文字/附件编辑。 |
| 模型缓存与配额 | 损坏文件、取消与恢复下载、空间不足、quota/eviction、清站点数据、旧版本清理；逐模型复核发布包内 LICENSE/NOTICE 与再分发条件。 |
| 安全与资源装载 | 真实浏览器下 CSP、`.wasm` MIME、Worker/AudioWorklet 同源装载、SIMD feature failure；网络检查确认没有向 Backend Runtime 或其他服务上传音频，失败不静默改为上传识别。 |

## 已有验证与更新规则

2026-09-09 至 09-10 的指定构建已有 macOS、Chrome 与 Safari 的部分编辑保存、刷新恢复、下载哈希、
断线恢复、双向 CAS 和令牌轮换证据。这些结果没有把整列 Manual 改成通过，也不覆盖新的构建或所有
断线组合。当前结果与缺口统一维护在[质量与待验清单](quality/README.md)，历史操作记录从该页追溯。

每次新增验收记录源码/构建标识、平台与浏览器版本、步骤、预期、实际和通过/失败/未验。只更新有证据
支持的单项；真实图片剪贴板、设备权限与中断、Browser ASR、完整原生手势、托盘、自启动顺序和正式
安装升级等缺口继续保留。
