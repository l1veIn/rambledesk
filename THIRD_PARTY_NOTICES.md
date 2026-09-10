# Third-party notices

RambleDesk 使用下列独立第三方组件。这里记录来源与许可证，
不改变各组件自己的许可证条款。

Codeg 的 Agent 管理、Chat 与外观配色模块按固定 commit `3ebdfed1d7c0b71d71880a3d2e0f8e09545feae1`
提取并修改。来源 <https://github.com/xintaofei/codeg>，版权归 Codeg 作者和贡献者；Apache-2.0
全文随附于 `licenses/codeg-APACHE-2.0.txt`。逐文件来源、修改与验收见 `docs/CODEG_PORTS.md`。

| 组件 | 用途 | 许可证 | 来源 |
|------|------|--------|------|
| Inter Variable 5.3.0 | 内置界面字体 | SIL OFL 1.1（随附 `licenses/font-inter-OFL-1.1.txt`） | <https://github.com/rsms/inter> |
| Geist Variable / Geist Mono Variable 5.3.0 | 内置界面与代码字体 | SIL OFL 1.1（随附各 `licenses/font-geist*-OFL-1.1.txt`） | <https://github.com/vercel/geist-font> |
| JetBrains Mono Variable 5.3.0 | 内置代码字体 | SIL OFL 1.1（随附 `licenses/font-jetbrains-mono-OFL-1.1.txt`） | <https://github.com/JetBrains/JetBrainsMono> |
| Fira Code Variable 5.3.0 | 内置代码字体 | SIL OFL 1.1（随附 `licenses/font-fira-code-OFL-1.1.txt`） | <https://github.com/tonsky/FiraCode> |
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

## Kotone 语音实现来源

来源：[l1veIn/Kotone](https://github.com/l1veIn/kotone)；MIT，Copyright (c) 2026 l1veIn。
上游 [LICENSE](https://github.com/l1veIn/kotone/blob/main/LICENSE) 与本仓库随附的
`LICENSE` 使用同一 MIT 条款及版权署名。第三方语音运行库、VAD 与模型仍分别遵循上表许可证，
不会因 Kotone 的代码许可证而改变。

Sherpa online session 的配置与停止尾帧策略，以及 SenseVoice、FunASR-Nano、Silero VAD
的接线，由 Kotone 的实现改写而来。2026-07-29 复用审计中的参考路径包括
`crates/kotone-stt/src/online_transducer.rs`、`offline_sherpa.rs`、`xasr.rs`、
`sensevoice.rs` 与 `funasr_nano.rs`；该历史审计未固定来源 commit，不补造移植版本。

RambleDesk 改为有界音频队列和结构化错误，使用 VAD 持续切分非流式长录音，并保留自己的
Feedback Draft、模型目录与会话生命周期。未复用 Kotone orchestrator、游戏/热键配置、
文字注入、窗口界面或数据格式，也没有建立指向 Kotone 仓库的本地路径依赖。
