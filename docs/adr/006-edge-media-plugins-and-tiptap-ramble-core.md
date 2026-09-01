# ADR 006：边缘媒体插件与 TipTap Ramble Core

- 状态：Accepted
- 日期：2026-09-01
- 术语源：[TERMINOLOGY.md](../TERMINOLOGY.md)
- 调研：[BROWSER_LOCAL_ASR_AND_PLATFORM_PLUGINS_RESEARCH.md](../BROWSER_LOCAL_ASR_AND_PLATFORM_PLUGINS_RESEARCH.md)

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

## Rejected

- Browser 把 MediaRecorder Blob、WAV 或 PCM 上传给 Desktop / Backend Runtime 识别；这会把设备能力
  穿过 Application Transport，使 Web Client 依赖另一设备的模型、并发槽和生命周期。
- 把 Ramble 定义为必须启动语音的独立采集状态机；文字和 TipTap 编辑才是基础流程，语音只是可选输入。
- 让 Speech / Capture Plugin 直接编辑 TipTap 或写最终附件路径；这会让平台实现拥有 Draft 语义。

## Consequences

Browser 必须承担 WASM 与模型的版本、下载、校验、缓存、许可证、内存和真实设备性能验收。作为回报，
原始音频留在输入设备，Desktop / Browser / Mobile 可以独立运行，Web Access 不增加 speech route 或
音频流协议，Ramble 的唯一持久输入面仍是版本化 TipTap `document_json`。
