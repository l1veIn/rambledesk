# Third-party notices

RambleDesk Windows MVP 使用下列独立第三方组件。这里记录来源与许可证，
不改变各组件自己的许可证条款。

| 组件 | 用途 | 许可证 | 来源 |
|------|------|--------|------|
| cpal 0.16 | 跨平台麦克风采集 | Apache-2.0 | <https://github.com/RustAudio/cpal> |
| sherpa-onnx 1.13.4 | 本地流式 ASR runtime 与 Rust binding | Apache-2.0 | <https://github.com/k2-fsa/sherpa-onnx> |
| X-ASR 480ms streaming zh/en punct int8 | 默认本地流式中英标点模型 | Apache-2.0 | <https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05.tar.bz2> |
| SenseVoice zh/en/ja/ko/yue int8 | 可选多语言非流式模型 | FunASR Model Open Source License Agreement 1.1 | <https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17> |
| FunASR-Nano zh/en/ja int8 | 可选非流式模型 | FunASR Model License | <https://huggingface.co/csukuangfj/sherpa-onnx-funasr-nano-int8-2025-12-30> |
| Silero VAD ONNX | 非流式语音的本地活动检测与分段 | 见上游发布条款 | <https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models> |
| Vercel AI SDK (`ai`, `@ai-sdk/openai`) | 可选 Feedback Cooking 的 OpenAI-compatible 模型调用 | Apache-2.0 | <https://github.com/vercel/ai> |
| tauri-plugin-http 2.5.9 | 从桌面 WebView 安全代理用户配置的模型 API 请求 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/plugins-workspace> |
| xcap 0.9.7 | 鼠标所在显示器的本地区域截图 | Apache-2.0 | <https://github.com/nashaofu/xcap> |
| tauri-plugin-global-shortcut 2.3.2 | Windows 全局截图快捷键 | Apache-2.0 OR MIT | <https://github.com/tauri-apps/plugins-workspace> |
| image 0.25 | 内存截图裁剪和 PNG 编码 | Apache-2.0 OR MIT | <https://github.com/image-rs/image> |

模型不提交到 RambleDesk Git 仓库。开发机按 `crates/rambledesk-speech/models/`
中的模型清单获取并校验。

Sherpa online session 的配置与尾帧策略，以及 SenseVoice、FunASR-Nano、
Silero VAD 的接线，均由 Kotone 的 MIT 实现改写而来。RambleDesk 改为有界
音频队列，并使用 VAD 持续切分非流式长录音；未复用 Kotone orchestrator。
详情见 `docs/KOTONE_REUSE.md`。
