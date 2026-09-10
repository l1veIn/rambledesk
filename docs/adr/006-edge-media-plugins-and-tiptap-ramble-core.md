# ADR 006：边缘媒体插件与 TipTap Ramble Core

- 状态：Accepted
- 日期：2026-09-01
- 术语源：[TERMINOLOGY.md](../TERMINOLOGY.md)
- 当前支持与待验：[Web Access 支持矩阵](../WEB_ACCESS_SUPPORT_MATRIX.md)

## Decision

Ramble 是以 TipTap Feedback Draft 为中心的编辑流程，不是录音 session。语音识别、截图和未来相机
输入由当前 Workbench Client 设备上的第一方 Platform Plugin 完成；Application Transport 不代理
设备权限，也不传输实时音频或 recognition session。

Speech Recognition Plugin 在每个平台本地组合采集、重采样、VAD、Speech Engine 与模型管理，只向
TipTap Ramble Core 输出统一 SpeechEvent。Capture Plugin 只返回 Attachment Candidate，由共享 Draft
流程验证和持久化后插入 TipTap。平台共享合同与黑盒测试，不共享同一个引擎进程、模型、权限或
acquisition UX。

Desktop 保留 `rambledesk-speech` 内部的 Audio Source / Speech Engine seam；Browser 使用
`getUserMedia`、AudioWorklet、dedicated Worker 与 sherpa-onnx WebAssembly；Mobile 未来使用各自
原生音频 API 与 sherpa-onnx binding。Platform Plugin 首期表示静态装配的 typed 深 Module，不承诺
任意第三方动态插件系统。

## Browser 实现取舍

- AudioWorklet 只负责实时安全的采集、单声道下混和有界 PCM 传递；识别在 dedicated Worker 内执行。
  读取真实采样率，使用保留跨块状态的流式重采样，不能假定设备本身以模型所需的 16 kHz 采集。
- Worker 持有 WASM、模型、recognizer 与识别生命周期。stop 处理尾帧并产出最终 stable event；cancel
  释放本地设备与 Worker，不把剩余音频提交给 Backend Runtime。
- 当前 pilot 使用 sherpa-onnx 单线程 SIMD 路径和 Zipformer Small streaming CTC。模型与 runtime
  分开发行、固定版本和哈希，模型经用户显式安装进入版本化 Cache Storage；模型文件大小不等于峰值内存。
- Wasm、glue、Worker 与 AudioWorklet 使用同源静态资源；保持 `script-src 'self' 'wasm-unsafe-eval'`
  与 `worker-src 'self'`，`.wasm` 使用 `application/wasm`。不为动态装载示例引入宽泛 JS eval。
  当前不启用 Wasm threads；若未来引入共享内存，必须重新评估 COOP/COEP 和真实浏览器条件。
- 不支持、权限拒绝、模型损坏或资源不足时保留文字与附件路径，不静默回退为服务器上传识别。
  模型缓存的配额、驱逐、离线重启，以及每个模型的许可证与 notice 都需独立验证。

冷暖启动、真实 PCM/flush、长会话和浏览器安全策略的验收集中维护在
[支持矩阵](../WEB_ACCESS_SUPPORT_MATRIX.md#browser-local-asr-必验项)，不在历史研究中另建一份通过状态。

## Current implementation status

- Desktop Speech/Capture Plugin contract 已进入自动化；真实 OS 权限、设备、截图 overlay/pin 与
  全局快捷键仍需发布前人工回归。
- Browser local ASR pilot 已实现固定模型的下载/hash/cache、同源 Wasm/Worker/AudioWorklet 装载与
  recognizer creation 自动化。真实 Chrome/Safari 麦克风授权、PCM 输入、稳定出字、停止 flush 与
  长会话仍为 Manual / unverified，不能从自动化门禁外推浏览器兼容承诺。
- Browser 当前支持 file input 与 image paste 的 Attachment Candidate；Browser screen capture 是
  Deferred / Unsupported，不以 `getDisplayMedia()` API 存在为交付证据。
- 完整 Desktop / Browser 支持边界见
  [WEB_ACCESS_SUPPORT_MATRIX.md](../WEB_ACCESS_SUPPORT_MATRIX.md)。

## Rejected

- Browser 把 MediaRecorder Blob、WAV 或 PCM 上传给 Desktop / Backend Runtime 识别；这会把设备能力
  穿过 Application Transport，使 Web Client 依赖另一设备的模型、并发槽和生命周期。
- 把 Ramble 定义为必须启动语音的独立采集状态机；文字和 TipTap 编辑才是基础流程，语音只是可选输入。
- 让 Speech / Capture Plugin 直接编辑 TipTap 或写最终附件路径；这会让平台实现拥有 Draft 语义。

## Consequences

Browser 必须承担 WASM 与模型的版本、下载、校验、缓存、许可证、内存和真实设备性能验收。作为回报，
原始音频留在输入设备，Desktop / Browser / Mobile 可以独立运行，Web Access 不增加 speech route 或
音频流协议，Ramble 的唯一持久输入面仍是版本化 TipTap `document_json`。
