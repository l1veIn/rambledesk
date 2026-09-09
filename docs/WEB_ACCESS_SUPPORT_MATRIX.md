# Web Access 支持矩阵

> 状态：WEB10 当前基线。
> 术语源：[TERMINOLOGY.md](TERMINOLOGY.md)。
> 本表只陈述已经进入仓库的能力、仍需人工验收的能力和明确不支持的能力。

## 状态标记

- **Automated**：存在自动化 contract、unit 或 loopback black-box test。它证明代码合同，不等于真实
  操作系统、浏览器权限、麦克风或长会话已经人工验收。
- **Manual**：必须在真实 Desktop、Chrome 或 Safari 中人工验证；未完成时不得写成兼容承诺。
- **Unsupported**：当前产品不提供；若标记 Deferred，表示以后可以重新立项，但不属于 WEB10。

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

## Web Access 运行边界

- Web Access **默认关闭**。用户只能从 Desktop 设置显式启动或停止。
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

## 明确不支持或不在本轮范围

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| LAN bind / 远程局域网访问 | **Unsupported / out of scope** | 不绑定 `0.0.0.0`；loopback 的 secure-context 例外不能外推到 LAN。 |
| TLS / HTTPS / WSS | **Unsupported / out of scope** | 当前没有证书、TLS proxy 或远程 credential delivery 产品合同。 |
| Web Access autostart | **Automated** contract；真实启动顺序为 **Manual** | 默认关闭；开启后随 Desktop 启动，启动失败仍只体现在状态与诊断中。 |
| 用户可配置端口 | **Automated** contract | 默认 `127.0.0.1:37643`，可在 1024–65535 内修改；改动在下次启动 Web Access 时生效，端口占用时显示失败，不静默改端口。 |
| Headless Backend Runtime 或独立 Web deployment | **Unsupported / out of scope** | 当前 composition root 仍是 Desktop；启动 Web Access 不等于提供 headless server。 |

## 发布前人工验收

1. macOS 与 Windows Desktop：TipTap 编辑、附件、截图/overlay/pin、全局快捷键、tray、updater 与系统权限。
2. Chrome 与 Safari：Web bootstrap、重连、TipTap autosave/CAS、上传、图片粘贴、submit 与下载。
3. Chrome 与 Safari Browser local ASR pilot：冷/热模型启动、麦克风授权、真实 PCM、稳定出字、停止
   flush、拒绝/忽略权限、页面隐藏、设备中断和长会话资源释放。

上述人工项未记录通过前，只能说相应代码路径和自动化门禁存在，不能声称目标浏览器或设备已兼容。

## 2026-09-10 质量收敛复验

rc3 之上的未提交候选已在 macOS 原生、Chrome 152 和 Safari 26.3.1 完成部分实际操作：结构化编辑与
刷新/重启恢复、Native/Chrome 文件选择、Safari Markdown 附件预览、提交，以及两种浏览器的实际
下载与包哈希核对。Chrome 未保存离页确认也已实际观察；精确版本、构建和逐项边界见
[原生与浏览器记录](quality/NATIVE_BROWSER_ACCEPTANCE.md)。

build6 的实际 Native Web Access 还完成两端 Stop → 离线编辑 → Start → 重新认证 → 保存与刷新恢复；
同一草稿交换 Chrome/Safari 先后认证顺序的两轮 CAS 均只保存胜者，输家显示 Save failed 并保留完整原文。
SQLite 核对两轮胜者及邻居请求未受影响。Native 实际确认轮换 Quality 令牌后，两端 session 均被撤销，
Safari 的旧 token 被明确拒绝，新 token 两端可用；Chrome 冲突稿保留，Safari 已保存稿保持 r4。
具体操作与证据见上述记录，未据此宣称所有断线组合均已通过。

这些记录没有把整列 Manual 改为通过：真实图片剪贴板、其他媒体手势、麦克风与录音设备权限／中断、
Browser local ASR 真机 pilot、完整原生手势、托盘、自启动实际启动顺序、正式安装升级及 Windows
仍有未验项。手机真机暂不可用，浏览器视口结果单列于
[响应式验收](quality/RESPONSIVE_ACCEPTANCE.md)。完整收敛状态以 [计划账本](PROJECT_QUALITY_PLAN.md#8-实施账本与交付方式) 为准。
