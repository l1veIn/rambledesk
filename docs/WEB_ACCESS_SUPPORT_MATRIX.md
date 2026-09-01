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
- 默认且固定入口是 `http://127.0.0.1:37643`；listener 只绑定 IPv4 loopback。
- Web Access 与 Local Integration Server 使用不同 listener、credential、auth domain、route set 和
  lifecycle。停止 Web Access 不停止 Backend Runtime，也不停止 Local Integration Server。
- 浏览器必须通过 same-origin `POST /api/auth/session` 用 durable credential 换取只存在 JavaScript
  内存中的短期 session；HTTP 使用 session bearer，WebSocket 使用受约束的 subprotocol credential。
- 自动化覆盖 bootstrap/HTTP/event/session/body 限制、event 连接预算恢复、body 超限无 mutation
  副作用，以及 Web Access 停止后 Local Integration 仍可写。

## 明确不支持或不在本轮范围

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| LAN bind / 远程局域网访问 | **Unsupported / out of scope** | 不绑定 `0.0.0.0`；loopback 的 secure-context 例外不能外推到 LAN。 |
| TLS / HTTPS / WSS | **Unsupported / out of scope** | 当前没有证书、TLS proxy 或远程 credential delivery 产品合同。 |
| Web Access autostart | **Unsupported / out of scope** | 默认关闭，不随 Desktop 自动启动。 |
| 用户可配置端口 | **Unsupported / out of scope** | 产品入口固定为 `127.0.0.1:37643`；端口占用时显示失败，不静默改端口。 |
| Headless Backend Runtime 或独立 Web deployment | **Unsupported / out of scope** | 当前 composition root 仍是 Desktop；启动 Web Access 不等于提供 headless server。 |

## 发布前人工验收

1. macOS 与 Windows Desktop：TipTap 编辑、附件、截图/overlay/pin、全局快捷键、tray、updater 与系统权限。
2. Chrome 与 Safari：Web bootstrap、重连、TipTap autosave/CAS、上传、图片粘贴、submit 与下载。
3. Chrome 与 Safari Browser local ASR pilot：冷/热模型启动、麦克风授权、真实 PCM、稳定出字、停止
   flush、拒绝/忽略权限、页面隐藏、设备中断和长会话资源释放。

上述人工项未记录通过前，只能说相应代码路径和自动化门禁存在，不能声称目标浏览器或设备已兼容。
