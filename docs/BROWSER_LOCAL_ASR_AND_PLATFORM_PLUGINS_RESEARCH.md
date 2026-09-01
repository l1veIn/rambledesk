# 浏览器本地 ASR 与平台插件边界调研

> 状态：方向校准研究，不是已完成实现  
> 日期：2026-09-01  
> 外部基线：sherpa-onnx `v1.13.7`（`917bed95c8e5c7c18aa4d69fea42e9ef8ef0a60e`）  
> 仓库基线：`4ea6fbd` 与其后的未提交 Web Speech 实验  
> 证据标记：**官方事实**来自 sherpa-onnx 官方仓库/文档或 Web Platform 官方文档；**仓库事实**来自当前 RambleDesk；**架构推论**是据此为 RambleDesk 作出的设计判断。

## 1. 结论

**浏览器端 ASR 应改为浏览器本地 sherpa-onnx WebAssembly，不应把 WAV 上传给 Desktop Web Access 后端识别。** Desktop、Browser、Mobile 各自在本机完成录音、重采样、VAD、ASR 和模型管理；Ramble 核心只接收统一的转录事件并把稳定文本写入唯一的 TipTap `document_json`。截图同理：由当前平台取得图像，核心只接收待持久化的附件候选。

这个判断不是因为 WebAssembly “理论上可行”，而是因为 sherpa-onnx 已提供以下官方实现面：

- **官方事实：** WebAssembly 导出同时覆盖 streaming ASR、non-streaming ASR 和 VAD；官方 JS wrapper 已有 online/offline recognizer 与 VAD 调用接口。[WASM 总览](https://k2-fsa.github.io/sherpa/onnx/wasm/index.html)、[当前共享导出列表](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/wasm-common.cmake)、[ASR wrapper](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/asr/sherpa-onnx-asr.js)、[VAD wrapper](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/vad/sherpa-onnx-vad.js)
- **官方事实：** 当前仓库已有“麦克风 → Web Worker → VAD → non-streaming ASR”的 Web 示例。模型字节被写入 Emscripten 文件系统，推理在 Worker 中运行，并把分段与文本发回 UI。[Worker 管理代码](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/lib/worker_web.dart)、[Worker 实现](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/web/vad-asr-worker.js)
- **官方事实：** Android 有 JNI/本地库构建路径，iOS 有 Swift/XCFramework 分发路径；没有技术理由让手机绕回 Desktop Runtime。[Android](https://k2-fsa.github.io/sherpa/onnx/android/index.html)、[Android 构建](https://k2-fsa.github.io/sherpa/onnx/android/build-sherpa-onnx.html)、[iOS Swift](https://k2-fsa.github.io/sherpa/onnx/ios/build-sherpa-onnx-swift.html)、[Swift Package](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/Package.swift)
- **架构推论：** 浏览器上传 WAV 的路线把设备能力错误地穿过 Application Transport，并让浏览器依赖 Desktop 进程的模型、并发槽和生命周期。它既违反“音频仅在当前设备处理”的产品承诺，也阻断未来独立 Web/Mobile Client。

因此建议停止继续产品化未提交的 Web Speech 上传协议，保留 `4ea6fbd` 中“采集与识别分离”的内部模块化成果，但把它收进 Desktop Speech Plugin，而不是把 Desktop Speech Engine 提升成所有客户端共享的后端能力。

## 2. sherpa-onnx WebAssembly 的实际能力

### 2.1 构建与运行形态

**官方事实：** 官方仓库目前同时保留了三类 Web 构建入口：

1. `build-wasm-simd-asr.sh` + `wasm/asr`：面向 ASR 的旧式构建，可把模型预加载进 Emscripten `.data`。
2. `build-wasm-simd-vad-asr.sh` + `wasm/vad-asr`：VAD 与 offline ASR 的旧式组合构建。
3. `build-wasm-simd-web.sh` + `wasm/web`：当前模块化 Web 构建，生成 `SherpaOnnx` factory，统一导出 streaming ASR、offline ASR、VAD 等能力，模型不必编进 `.data`。[当前 Web 构建脚本](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/build-wasm-simd-web.sh)、[Web CMake 入口](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/web/CMakeLists.txt)、[Flutter Web 产物组装](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/build-flutter-web-wasm.sh)

**架构推论：** RambleDesk 应采用第 3 条作为维护基线，固定 sherpa-onnx tag、Emscripten 版本、构建参数和产物 SHA。旧式 `.data` demo 适合验证，不适合把大模型与应用版本强绑定。

### 2.2 JS/WASM 调用接口

**官方事实：** online recognizer 的标准生命周期是：创建 recognizer/stream，向 stream 调用 `acceptWaveform(sampleRate, Float32Array)`，在 `isReady` 时 `decode`，读取 `getResult`，在 endpoint 后 `reset`，结束时 `inputFinished` 并释放资源。offline recognizer 则在送入完整语音段后 `decode` 并读取结果。输入是 `[-1, 1]` 的单声道 `Float32Array`，并显式传入 sample rate。[ASR wrapper](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/asr/sherpa-onnx-asr.js)

**官方事实：** VAD wrapper 支持 Silero VAD 与 TEN VAD 配置，并提供 `acceptWaveform`、`detected`、`front`、`pop`、`flush`、`reset` 等操作，可把连续 PCM 切成有界语音段再送 offline recognizer。[VAD wrapper](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/vad/sherpa-onnx-vad.js)

**官方事实：** 官方资产说明明确展示了 streaming Zipformer transducer、streaming Paraformer，以及“Silero VAD + 任一支持的 non-streaming model”的组合方式。[Streaming ASR 资产说明](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/asr/assets/README.md)、[VAD + offline ASR 资产说明](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/vad-asr/assets/README.md)

**官方事实：** 当前 Flutter Web Worker 示例的配置分支还覆盖 Zipformer CTC、SenseVoice、Whisper、NeMo Parakeet TDT、Moonshine、Qwen3 ASR、FunASR Nano 与 FireRed ASR CTC。这里能证明 wrapper/config 路径存在，不能证明每个模型都适合 RambleDesk 的目标浏览器与内存预算。[官方 Web Worker 配置](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/lib/worker_web.dart)

### 2.3 Worker 与 AudioWorklet

**官方事实：** sherpa-onnx 当前完整示例把 VAD/ASR 放在专用 Web Worker 中，而不是 UI 主线程。旧 `wasm/asr/app-asr.js` 仍是示例级代码，使用已废弃的 `ScriptProcessorNode`；Web 平台建议把实时音频处理迁到 AudioWorklet，AudioWorklet 的处理逻辑运行在独立音频渲染线程。[官方 Worker 示例](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/web/vad-asr-worker.js)、[MDN AudioWorklet](https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API/Using_AudioWorklet)、[MDN ScriptProcessorNode](https://developer.mozilla.org/en-US/docs/Web/API/ScriptProcessorNode)

**架构推论：** Browser Speech Plugin 应使用两级执行面：

- AudioWorklet 只做有界、实时安全的采集、单声道下混与 PCM 传递；不得在音频线程运行 ONNX。
- 专用 Web Worker 拥有 sherpa WASM、模型文件、VAD/recognizer 与识别状态；通过带背压的有界消息协议接收 PCM，并只向 UI 发标准化 SpeechEvent。

官方 Flutter 示例动态 `eval` JS 源码并创建 Blob URL。RambleDesk 不应照搬这一装载细节：应把 glue、wrapper 与 Worker 作为同源静态资源发布，以维持严格 CSP。可复用的是 Worker 内推理、模型写入 Emscripten FS 和消息生命周期。

### 2.4 SIMD、线程与 COOP/COEP

**官方事实：** 当前 Web 构建明确使用 ONNX Runtime 的 wasm-simd 静态库；`wasm-common.cmake` 设置 `INITIAL_MEMORY=512MB`、`ALLOW_MEMORY_GROWTH=1`，但官方构建脚本和 CMake flags 没有 `-pthread`/`-sUSE_PTHREADS`/shared-memory 开关。更直接地，sherpa-onnx C API 对 WASM 明确把大于 1 的 `num_threads` 强制降为 1；官方 Web Worker 代码也把 recognizer `numThreads` 固定为 `1`。[ORT SIMD 依赖](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/cmake/onnxruntime-wasm-simd.cmake)、[WASM flags](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/wasm/wasm-common.cmake)、[C API WASM 单线程限制](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/sherpa-onnx/c-api/c-api.cc#L62-L75)、[Worker 配置](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/lib/worker_web.dart)

**结论：当前上游单线程 SIMD 路径不要求 COOP/COEP。** SIMD 与 Wasm threads 是两件事。若未来 RambleDesk 自行打开 Wasm threads/shared memory，WebAssembly threads 会依赖共享内存，而 `SharedArrayBuffer` 的可靠使用要求 cross-origin isolation；届时需要至少配置 COOP `same-origin` 与 COEP `require-corp` 或 `credentialless`，并以 `crossOriginIsolated` 做运行时门禁。[MDN WebAssembly threads](https://developer.mozilla.org/en-US/docs/WebAssembly/Understanding_the_text_format#webassembly_threads)、[MDN COOP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cross-Origin-Opener-Policy)、[MDN crossOriginIsolated](https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope/crossOriginIsolated)、[MDN Wasm SIMD `v128`](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Value_types/v128)

**架构推论：** 第一版不要为了“可能更快”提前开启线程和 COOP/COEP。先按官方单线程 SIMD 基线完成真实设备测量；只有测量证明单线程不达标、并验证所有同源/跨源资源可满足 COEP 后，再另立性能 ADR。

## 3. 浏览器采集、重采样与权限

**官方事实：** `getUserMedia()` 只在 secure context 可用，需要用户授权，而且用户忽略权限提示时 Promise 可能一直不 settle。[MDN getUserMedia](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)

**官方事实：** Secure Contexts 规范把 loopback 地址视作 potentially trustworthy，因此当前 `http://127.0.0.1` Web Access 可以在符合规范的现代浏览器中申请麦克风；一旦开放普通 LAN 地址，必须使用 HTTPS，不能把 loopback 例外外推到局域网。[W3C Secure Contexts](https://www.w3.org/TR/secure-contexts/)、[MDN Secure Contexts](https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Secure_Contexts)

**官方事实：** `AudioContext.sampleRate` 表示该 context 全部节点使用的实际采样率；构造时请求指定采样率并不等于设备一定按 16 kHz 工作，不支持的值还可能失败。[MDN AudioContext constructor](https://developer.mozilla.org/en-US/docs/Web/API/AudioContext/AudioContext)、[MDN sampleRate](https://developer.mozilla.org/en-US/docs/Web/API/BaseAudioContext/sampleRate)

**架构推论：** Browser Speech Plugin 必须读取实际 sample rate，稳定地下混为单声道，再用保持跨块状态的流式 resampler 转为模型要求的 16 kHz；不能假定 `new AudioContext({ sampleRate: 16000 })` 已解决重采样。采集协议还要有明确的队列上限、drop 计数、停止时尾帧 flush 和设备中断状态。

权限拒绝、长期未响应、设备拔出、页面刷新、后台节流都应成为 typed state。浏览器本地 ASR 不应在失败时静默回退为上传服务器；产品降级应是明确提示“此浏览器/设备暂不可用”，并保留文字编辑。

## 4. 模型资产、体积与浏览器缓存

### 4.1 已知体积

**官方事实：** sherpa-onnx 旧版 WASM 构建文档中的一个 streaming Zipformer 示例，产物约为 199 MB `.data` 加约 10 MB `.wasm`；这是一个具体示例，不是所有模型的固定体积。[官方构建文档](https://k2-fsa.github.io/sherpa/onnx/wasm/build.html)

**仓库事实：** RambleDesk 当前 manifest 的落盘文件合计为：

| 模型 | 当前文件体积 | 浏览器首批判断 |
| --- | ---: | --- |
| X-ASR streaming | 169,347,218 bytes；下载包 133,895,136 bytes | 首选 feasibility candidate，但须验证这个具体模型与当前 WASM build |
| SenseVoice | 239,549,735 bytes | 可作为 VAD + offline 后续候选，首装和内存成本较高 |
| FunASR Nano | 1,009,605,061 bytes | 不应成为浏览器默认；先排除首批 |

这些数字只描述模型文件，不包含约 512 MB 的初始 Wasm memory 设置、WASM/glue、JS/ArrayBuffer 与 Emscripten FS 装载时可能出现的临时副本。**不能用模型文件大小推导峰值内存。**

**官方事实：** sherpa-onnx 没有发布可直接套用到 RambleDesk 目标浏览器/设备的首载时间、峰值内存、实时率或长会话延迟 SLA。官方 demo 和构建成功只能证明运行路径存在，不能替代 RambleDesk 的浏览器 benchmark。

**架构推论：** 浏览器 Phase 0 先验证 X-ASR streaming，从“持续 partial + endpoint 后 stable”的 Ramble 体验入手。它是候选，不是已确认兼容：官方 wrapper 支持 online transducer，但当前资料没有直接证明 RambleDesk 这份 2026 X-ASR manifest 已在目标浏览器矩阵中跑通。SenseVoice/VAD 可进入第二阶段；约 1 GB 的 FunASR Nano 不进入浏览器默认路径。

### 4.2 分发与缓存

**官方事实：** 当前官方 Worker 示例把模型作为应用 assets 读成字节，通过消息交给 Worker，再写入 Emscripten MEMFS；它没有提供 RambleDesk 所需的跨版本模型安装、校验或持久缓存管理。[Worker 管理代码](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/lib/worker_web.dart)、[Worker 文件装载](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/flutter-examples/vad-non-streaming-asr-from-microphone/web/vad-asr-worker.js)

**官方事实：** Cache API 可以持久保存 Request/Response 对，但生命周期与配额受浏览器管理，应用需要自己做版本和清理；OPFS 是 origin-private 的本地文件存储，也受配额和站点数据清理影响。应用可用 `navigator.storage.estimate()` 评估配额，并请求 persistent storage，但不能把它视为永不驱逐的保证。[MDN Cache](https://developer.mozilla.org/en-US/docs/Web/API/Cache)、[MDN OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system)、[MDN storage quotas](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria)

**架构推论：** Browser Speech Plugin 内部应有版本化 Model Store：

- key 至少包含 `model_id + model_version + file_sha256`；manifest 记录每个文件的 URL、byte size、SHA-256、license、notice 和 browser eligibility；
- 下载前估算空间，下载后逐文件验 size/hash，全部成功才原子地标为 installed；支持取消、损坏恢复、显式删除与旧版本 GC；
- Cache API 与 OPFS 二选一应由 feasibility spike 以装载成本和浏览器兼容性决定；不管采用哪个，当前 sherpa runtime 仍需要把模型暴露给 Emscripten FS；
- “离线可用”只能在一次完整、校验通过的安装和离线重启测试之后展示。

### 4.3 CSP 与静态资源

**仓库事实：** `crates/rambledesk-local-server/src/web_access_server.rs` 当前 CSP 只有 `default-src 'self'` 等规则，没有允许 WebAssembly 编译的 `script-src 'wasm-unsafe-eval'`；静态文件服务也需要为 `.wasm` 返回正确的 `application/wasm` MIME。

**官方事实：** CSP 的 `script-src`/`default-src` 会控制 WebAssembly compilation；`'wasm-unsafe-eval'` 可以只放开 Wasm compilation，而不等同于放开一般 JS `eval()`。[MDN CSP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy)

**架构推论：** Web pilot 应加入最窄的 `script-src 'self' 'wasm-unsafe-eval'`，Worker 使用同源静态 URL，并明确 `worker-src 'self'`。不要为复制官方示例的动态源码装载而加入 `'unsafe-eval'` 或宽泛 Blob script。COOP/COEP 仍只在未来启用 Wasm threads 时加入。

## 5. 许可证与模型分发

**官方事实：** sherpa-onnx 框架代码是 Apache-2.0。[LICENSE](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.7/LICENSE)

**官方事实：** 这不自动覆盖模型。官方预训练模型说明会单独指出模型包中的 LICENSE；例如 SenseVoice 页面要求查看解压目录的模型许可证，FunASR Nano 也有独立的模型导出与来源说明。[SenseVoice 模型](https://k2-fsa.github.io/sherpa/onnx/sense-voice/pretrained.html)、[FunASR Nano](https://k2-fsa.github.io/sherpa/onnx/funasr-nano/export.html)

**仓库事实：** 当前 RambleDesk manifest 已把 SenseVoice 标为 `FunASR Model Open Source License Agreement 1.1`，FunASR Nano 标为 `FunASR Model License`；X-ASR manifest 目前没有独立 `license` 字段。

**架构推论：** 将模型下载到用户浏览器仍是模型分发。上线前必须逐模型确认官方发布包中的许可证、notice、再分发权限与展示义务；不能以 sherpa-onnx 的 Apache-2.0 代替模型许可证。X-ASR 缺失的 manifest license 是 browser pilot 之前的阻断项。

## 6. Ramble 的平台插件设计

### 6.1 核心原则

这里引入待写入 `docs/TERMINOLOGY.md` 的新术语 **Platform Plugin（平台插件）**：一个在单一客户端平台内实现设备能力、权限、资源与生命周期的深模块。它不是 Host Adapter，也不经 Application Transport 代理设备操作。

Ramble 核心收敛为 TipTap 编辑流程：

- canonical 真源始终是版本化 TipTap `document_json`；Markdown 只是导出、提交与历史投影；
- 核心拥有 SpeechEvent 到 TipTap transaction 的映射、stable segment identity、pending/cleaned 标记、autosave/CAS、附件 node 插入和撤销；
- 核心不拥有麦克风/屏幕权限、PCM/WAV、重采样、VAD、模型文件、WASM/原生线程、浏览器缓存或系统截图 overlay；
- Application Transport 只传输 Feedback Draft/附件等业务事实，不传输实时音频、识别 session 或平台权限。

### 6.2 两条独立 Interface

不要做一个包含所有设备功能的浅 `PlatformPlugin` 大接口。建立一个很薄的 capability registry，下面挂两条独立深 Interface：

```text
TipTap Ramble Core
  ├─ SpeechRecognitionPlugin.start(options, emit) -> SpeechSession
  │    SpeechSession.stop()   # flush 尾帧并产生最终 stable event
  │    SpeechSession.cancel() # 释放平台资源，不提交残余文本
  │
  └─ CapturePlugin.capture(request) -> AttachmentCandidate | Cancelled
```

`SpeechRecognitionPlugin` 的 Interface 只暴露平台无关的 availability/model status、start/stop/cancel 与标准化事件：`started`、`level`、可选 `partial`、`stable(segment_id, text)`、`warning`、`stopped`、`error`。权限、模型安装、采集、重采样、VAD、backpressure、recognizer 和线程/Worker 全部隐藏在 Implementation 内。平台之间共享的是事件语义与合同测试，不要求相同的 partial 节奏、引擎进程或模型。

`CapturePlugin` 只返回待由 Draft 存储验证和持久化的 `AttachmentCandidate`（bytes/Blob、MIME、dimensions、source metadata）。它不直接编辑 TipTap，也不写最终附件路径。

### 6.3 平台映射

| 平台 | SpeechRecognitionPlugin Implementation | CapturePlugin Implementation |
| --- | --- | --- |
| Desktop | `cpal`/系统音频 + Rust sherpa-onnx + native worker/thread；模型在 Desktop 本地资料库 | OS 截图、全局快捷键、overlay、文件选择 |
| Browser | `getUserMedia` + AudioWorklet + 流式 resampler + dedicated Worker + sherpa-onnx WASM + origin model cache | `getDisplayMedia`、文件选择、粘贴/拖入；遵守用户手势与浏览器权限 |
| Android | `AudioRecord` + sherpa-onnx JNI/Kotlin；模型在 app sandbox | 系统 picker/camera/media projection，按 Android 权限模型 |
| iOS | `AVAudioEngine` + sherpa-onnx Swift/XCFramework；模型在 app sandbox | Photos/camera/share sheet 等 iOS 能力 |

**官方事实：** 浏览器 `getDisplayMedia()` 需要 secure context 和 transient user activation，每次都必须让用户选择/授权，权限不能持久化。[MDN getDisplayMedia](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getDisplayMedia)

**架构推论：** Browser Capture Plugin 不能承诺 Desktop 的全局快捷键或无提示 overlay 语义。平台可以拥有不同的 acquisition UX，但交给 TipTap 核心的附件候选合同一致。

## 7. 对 `4ea6fbd` 与未提交实验的处置

### 7.1 保留 `4ea6fbd` 的部分

`4ea6fbd` 将 native audio capture 与 recognizer 分离，这个方向应保留：

- `PcmAudioChunk` 的单声道 Float32、显式 sample rate 与有界时长合同；
- `SpeechEngineConfig`、`SpeechEngineSession`、`SpeechEngineHandle.try_push` 的 session/lifecycle 与有界队列/backpressure；
- recognizer 内部统一重采样到模型采样率；
- native source 与 engine composition，以及相应资源释放和背压测试。

但这些应成为 **Desktop Speech Plugin 的 Implementation 细节**。需要撤回“Speech Engine 是 Backend Runtime 共享能力”“Browser Audio Source 把 Blob 上传给它”的文档含义。Desktop 内部的 `Audio Source`/`Speech Engine` seam 可以存在，不应成为跨客户端网络 seam。

### 7.2 撤销未提交 Web Speech 上传实验

根据本次检查，以下整条路线应撤销，而不是继续修补：

- Web Access `/api/speech/recognition-sessions` 的 POST/PUT WAV/WebSocket/finish/cancel 协议；
- Web 端 `HttpSpeechRecognitionSession` 和 canonical WAV 编码/上传；
- server 端 recognition coordinator、session 状态机、上传上限与协议错误映射；
- Desktop `DesktopWebSpeechBackend` 与 uploaded WAV parser；
- 让 Desktop 麦克风和 Browser 上传识别共用一个 `SpeechRuntimeGate` 的跨平台并发槽；
- 仅为这些 speech routes 增加的 router/auth/capability 字段与协议测试。

若 Desktop 本机仍需要“同一时间一个识别 session”的 gate，可以在 Desktop Speech Plugin 内保留或重写；它不能限制另一个设备上浏览器自己的本地识别。现有协议测试只有在改写为平台无关 SpeechEvent/TipTap 插入合同测试后才值得保留。

### 7.3 后续文档零残留

实施前至少需要用新 ADR 同步修订：

- `docs/TERMINOLOGY.md` 中 Audio Source、Speech Engine、Native/Browser Capability 的定义；
- `docs/ARCHITECTURE.md` 中 browser Blob/chunk 送后端识别的描述；
- `docs/adr/005-shared-workbench-transport-capabilities.md` 第 5 节的 server-side recognition 路线。

目标不是把旧段落补一句例外，而是消除“设备能力经 Application Transport 代理”和“浏览器复用 Desktop Speech Engine”的残留。`Adapter` 继续专指 Host Adapter；新概念使用 Platform Plugin，避免术语碰撞。

## 8. 建议落地顺序与验收门

### Phase 0：浏览器 feasibility spike

只做 X-ASR streaming 单模型闭环：同源静态 WASM/Worker、AudioWorklet、实际采样率读取、流式重采样、Worker 内 recognizer、最小 Model Store、SpeechEvent → 当前 TipTap Editor。它不新增后端 speech route。

必须在承诺兼容前记录真实目标设备，而不是只跑单元测试：

- 浏览器矩阵：RambleDesk 实际支持的 Chrome/Edge/Firefox/Safari 版本与机器架构；
- cold install bytes/time、warm start time、WASM 初始化、模型 hash 验证、离线重启；
- 峰值内存、稳定内存、30/60 分钟长会话、页面 reload 后资源释放；
- real-time factor、partial/stable p50/p95 latency、丢帧数、队列峰值；
- 权限拒绝/不响应、设备拔出、tab background、AudioContext suspend/resume、模型损坏、quota/eviction；
- CSP、`.wasm` MIME、SIMD feature failure 与无服务端上传的网络检查。

### Phase 1：合同固化

以同一组 black-box contract tests 验证 Desktop/Browser Speech Plugin：start/stop/cancel 幂等、每个 stable segment 只插入一次、错误不破坏 Draft、停止会 flush、取消会释放设备、识别事件不携带 PCM/WAV。TipTap transaction 和 CAS 冲突处理留在 Ramble 核心测试。

### Phase 2：offline 模型与 Mobile

在体积/性能门通过后再加入 Browser VAD + SenseVoice offline；Android/iOS 分别以官方 native binding 实现同一 SpeechRecognitionPlugin Interface。不要把 WASM 当作所有平台的统一运行时，统一点应是 Interface 与 TipTap 事务语义。

## 9. 决策摘要

1. 接受“ASR 在输入发生的设备本地执行”为产品与架构原则。
2. Browser 采用 sherpa-onnx single-thread SIMD WebAssembly，AudioWorklet 采集，专用 Worker 推理；当前不要求 COOP/COEP。
3. 停止 Web Access WAV 上传/后端识别实验；Web Access 不新增实时音频协议。
4. 保留 `4ea6fbd` 的采集/recognizer 分离、PCM、重采样和背压设计，但收进 Desktop Speech Plugin。
5. Ramble 核心只拥有 TipTap Draft、SpeechEvent 插入和附件持久化；语音与截图是独立 Platform Plugin。
6. 浏览器模型必须版本化缓存、逐文件校验并单独审计模型许可证；X-ASR 是首个待验证候选，不是未经测量的兼容承诺。
