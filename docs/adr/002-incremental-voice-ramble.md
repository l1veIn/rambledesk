# ADR 002：语音 Ramble 采用增量转写与独立文档流

- 状态：Accepted
- 日期：2026-07-31
- 修订：2026-08-26。有界音频队列允许为 recognizer 后台加载放大到 4096；见 ADR 004。
- 参考：`docs/KOTONE_REUSE.md` 与本机 Kotone 语音实现审计

## 上下文

RambleDesk 的用户会一边真实操作目标软件，一边说话、截图、粘贴图片和修改正文。
因此“完整录音结束后再一次性转写”会冻结最重要的交互闭环，也会让长录音失败变成
整段反馈失败。

Kotone 已验证了 Rust 音频 ports、`cpal` 采集、16 kHz 单声道处理和可替换 STT
引擎的方向，但其实现面向短句 push-to-talk。RambleDesk 需要长时间会话、持续草稿、
有界背压和可编辑 Markdown 文档，不能直接复用 Kotone orchestrator。

## 决策

### 1. MVP 使用本地 Sherpa online，而不是 Windows 系统听写 API

Windows 侧审计首先评估了 `Windows.Media.SpeechRecognition`。该 API 当前要求应用具有
MSIX package identity；RambleDesk 现阶段使用 Tauri 开发运行和普通 NSIS/MSI 路径，
不能把它作为可靠的默认后端。为了不在 MVP 同时引入 MSIX 签名、商店身份和两套分发，
首版采用下列路径验证完整链路：

- `cpal` 从系统默认麦克风持续采集；
- 内存中的有界队列承接音频 callback；
- 持续线性重采样到 16 kHz mono，并以 50ms 帧喂入 Sherpa；
- X-ASR 480ms streaming zh/en punct int8 持续输出可变 partial；
- 自然端点形成 stable 文本并追加到富文本 Markdown 正文。

这不是“停止录音后再整段转写”：录音过程中会连续写入正文，图片、附件和人工编辑仍可
并行进行。

### 2. 语音与文档是两条独立、可并行的输入链路

```text
microphone
  -> cpal capture
  -> bounded in-memory frame queue
  -> mono + 16 kHz normalization
  -> Sherpa X-ASR 480ms online stream
  -> partial / level status (ephemeral UI only)
  -> stable segment (insert into Markdown editor + autosave)

paste / drag / file picker
  -> attachment storage
  -> attachment://<id> image node in the same Markdown document
```

录音期间，图片插入、正文编辑、草稿保存和提交前检查不得等待 STT。STT 完全不可用时，
用户仍可只用文字和图片完成反馈。

### 3. 默认 provider 升级为 sherpa-onnx X-ASR online

Windows MVP 默认使用 X-ASR 480ms streaming zh/en punct int8。音频以 50ms 帧持续
喂入，变化中的假设通过 `Partial` 只显示在工具条；自然停顿约 0.8 秒或连续语句达到
10 秒时形成 `Stable`，追加到 Markdown 正文。停止时补 800ms 静音尾帧，避免丢失
transducer lookahead 中的句尾 token。

首版不引入多模型 UI、自动下载器或设备选择。早期 Whisper spike 已证明采集到正文的
链路，但不进入当前 Windows 二进制：Sherpa 上游预编译静态库使用 `/MT`，whisper.cpp
使用 `/MD`，同时链接会触发 MSVC C runtime mismatch。MVP 不为备用 provider 维护
定制 C++ 构建链。

不具备 streaming 能力的引擎只能以 2–5 秒带重叠窗口的近实时分块方式接入，并且必须
持续产出稳定片段。只在停止录音后处理整段音频的 provider 不进入交互式 Ramble 主路径。

### 4. 转写事件区分瞬时状态与 stable

语音 port 输出：

- `Level` / `Processing`：只更新录音工具条，不写入草稿；
- `Partial`：Sherpa 当前可变假设，只显示在工具条，不写入草稿；
- `Stable`：包含 session、chunk index 和文本，追加为 Markdown 段落并触发普通自动保存；
- `Stopped`：会话结束标记，不重新整体替换此前稳定片段；
- `Warning` / `Error`：结构化、可恢复，不清空已经稳定的正文。

这样可以避免每个 token 都制造草稿 revision，也不会让模型修订覆盖用户已经手动修改的
文字。

### 5. 有界队列与长会话降级

- capture callback 不做 STT、磁盘 I/O 或 UI 调用；
- 音频帧进入有界 callback buffer 队列。默认 512；为覆盖 recognizer 后台加载，允许将
  队列加大到 4096，但仍不得随时长无界增长（见 ADR 004）；
- provider 变慢且队列满时丢弃新的 buffer，并向 UI 发出 `Warning`；
- MVP 不写临时音频文件，不允许 PCM 随录音时长无界增长；
- 20 分钟压力测试中内存必须稳定，停止后所有 worker 可在有限时间内退出。

### 6. 音频保留与隐私

原始音频默认只存在于有界内存队列和当前转写片段中，不落盘、不写入 Feedback Package。
后续可增加“将原始音频作为附件保留”的显式选项，但必须在录音前
显示状态，并把保留策略写入 manifest。

日志只记录设备、采样率、队列水位、延迟和错误码，不记录音频内容或完整转写。

### 7. 平台边界

- `rambledesk-core` 不依赖音频或 STT；
- `rambledesk-speech` 拥有事件 contract、capture、resample、模型与 worker；
- Windows 首版使用 `cpal` 默认 WASAPI 输入，不持久化设备选择；
- Tauri 只暴露开始、停止和事件桥接，并持有当前进程内 session；
- macOS 权限文案与签名配置在对应发行阶段补齐，但不得改变 core/speech contract。

Kotone 仓库使用 MIT 许可证。RambleDesk 仅改写其 online recognizer 配置与停止尾帧
经验，不建立 sibling path dependency，也不复用其 orchestrator。

## 实施顺序

1. 建立 `rambledesk-speech` crate、有界队列、重采样和事件 contract；
2. 实现 Windows `cpal` capture 与 Sherpa X-ASR online 50ms 帧输入；
3. 在桌面端加入录音状态、音量、错误提示和 stable 文档插入；
4. 真实麦克风签收中文、图片并行操作、停止时尾段 flush；
5. 跑 5、10、20 分钟中文与中英混合 ramble 评测；
6. 依据数据调优端点参数、模型与设备选择。

## 验收门槛

- 录音时仍可连续插入/删除图片、编辑正文和自动保存；
- 音量和 partial 状态应近实时可见，MVP stable 文本目标为自然停顿后约 1 秒；
- 20 分钟录音队列和内存保持有界；
- 设备断开或 provider 崩溃不会丢失已有 stable 文本；
- 中文、英文和混合文本以 UTF-8 原样进入 Markdown；
- 停止/取消后内存音频和 worker 被释放；
- 无模型、无麦克风权限或 STT 初始化失败时，文字与图片提交仍完整可用。

## 后果

正向：

- 语音不会阻塞截图和文档编辑；
- 本地、在线和近实时分块 provider 共用稳定 contract；
- 长录音风险被限制在有界 worker 内；
- 已稳定文本进入现有 Markdown/CAS/Feedback Package 链路，不创建第二套正文状态。

代价：

- 本地模型需要单独的许可证、约 75 MiB 体积和真实语料评测；
- 4 秒无重叠分块可能切断跨边界词句，属于 MVP 已知限制；
- Windows 音频设备与权限仍需要真实硬件人工签收。
