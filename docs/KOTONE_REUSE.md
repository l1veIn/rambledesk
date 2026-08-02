# Kotone 架构与语音复用审计

> 参考仓库：`/Users/yangchen/Desktop/kotone`  
> 审计日期：2026-07-29  
> 结论：复用架构和部分 Rust 实现，不复用 Kotone 业务编排。

## 1. 总结

Kotone 已经验证了四个对 RambleDesk 有直接价值的选择：

1. 根目录作为 Cargo + pnpm workspace，而不是单个 Tauri 应用目录；
2. 领域 core 不依赖 Tauri，平台和重型 STT 依赖位于适配 crate；
3. Tauri 只是 composition root 和 IPC/窗口壳；
4. CLI 作为第二个真实消费者，支持无人值守集成测试。

这些结构直接采用。

Kotone 的语音实现面向游戏短句、热键和即时发送；RambleDesk 面向更长的自由
ramble、草稿持续保存和可编辑反馈。因此语音代码只能按组件迁移，不能复用
Kotone 的 Orchestrator。

## 2. 可直接采用的架构

| Kotone 经验 | RambleDesk 采用方式 |
|-------------|---------------------|
| `members = ["apps/desktop/src-tauri", "crates/*"]` | 相同 workspace 形态 |
| 根 `package.json` 只转发脚本 | 相同 |
| 单一 pnpm lock | 相同 |
| `[workspace.dependencies]` 集中版本 | 相同 |
| core 持有 ports，适配器向 core 单向依赖 | 相同 |
| Tauri/CLI 在启动时注入实现 | 相同 |
| 重 STT 依赖受 feature 控制 | 相同 |
| CLI 提供 WAV fixture 无 GUI 测试 | 在 M3 采用 |

## 3. 代码级候选

以下内容可以作为迁移源，但必须复制到 RambleDesk、改名、补测试并记录来源，
不得建立指向桌面 sibling repo 的 path dependency。

### 3.1 高价值迁移

#### 音频端口与 cpal 采集

来源：

- `crates/kotone-core/src/audio.rs`
- `crates/kotone-platform-windows/src/audio.rs`

可迁移：

- 输入设备枚举；
- 原生采样格式转 `f32`；
- 多声道混合为 mono；
- 16 kHz 重采样；
- 50 ms PCM/RMS 事件；
- Resampler 单元测试。

RambleDesk 必须修改：

- unbounded channel 改为有界通道，定义背压/丢帧策略；
- 采集错误通过结构化事件返回，不使用 `eprintln!`；
- 支持长录音和分段落盘，不能把整段 PCM 永久留在内存；
- 平台 crate 不命名为 Windows-only；
- 增加 macOS 麦克风权限和 Linux backend 验证；
- 设备 ID 不以可能重复的 display name 作为唯一标识。

#### STT ports 与 registry

来源：

- `crates/kotone-core/src/stt.rs`
- `crates/kotone-stt/src/lib.rs`

可迁移：

- `SttEngine` / `SttSession` 分层；
- capability 声明；
- 外部注册具体 engine，core 不反向依赖实现；
- feature-gated engine。

RambleDesk 必须修改：

- `Transcript` 增加 provider/model/language/segment provenance；
- 支持长文本分段和增量稳定结果；
- 事件通道使用有界队列；
- 错误使用稳定 enum/code，不直接传任意字符串；
- session 不使用“按下到松手”的游戏语义；
- 转写失败不得阻止用户提交已有文字。

#### 模型下载与校验

来源：

- `crates/kotone-stt/src/download.rs`
- `crates/kotone-stt/src/model.rs`

可迁移：

- 流式下载；
- SHA-256 校验；
- 临时文件后原子 rename；
- 模型 manifest；
- feature 隔离；
- 失败清理和进度通知。

RambleDesk 必须修改：

- 删除 `~/.kotone`、Kotone settings 和中文游戏提示耦合；
- 下载目标由调用方注入；
- 不默认启用第三方镜像；
- 原子替换不得先删除唯一可用旧版本；
- 增加取消、磁盘空间检查和恢复策略；
- 每个模型保留许可证与来源元数据。

#### sherpa-onnx engine adapters

来源：

- `crates/kotone-stt/src/online_transducer.rs`
- `crates/kotone-stt/src/offline_sherpa.rs`
- `crates/kotone-stt/src/xasr.rs`
- `crates/kotone-stt/src/sensevoice.rs`
- `crates/kotone-stt/src/funasr_nano.rs`

价值：

- 已有官方 Rust binding 的实际接线；
- 已处理 streaming partial/final；
- 已有模型加载和 feature 策略；
- 已验证本地中文/中英 STT 路径。

进入 RambleDesk 前必须用 5–20 分钟真实 ramble 语料重新评测。Kotone 的短句
CER 和首字延迟不能证明长录音、标点、分段和内存行为合格。

### 3.2 只借鉴模式

- Tauri Rust → frontend 的全量状态事件；
- 前端收到事件后重新查询事实状态；
- runtime 预热和 restart-needed 模型；
- 本地隐私日志只记阶段、耗时和错误码；
- WAV fixture、回放和评测报告；
- 模型/原生依赖 feature 门控。

这些代码带有 Kotone 的窗口、热键和运行时假设，重写通常比抽取更安全。

## 4. 不复用

- `kotone-core::orchestrator`：绑定 push-to-talk、VAD 判停和“发送文字”终点；
- `inject`、`FocusBackend`、Windows SendInput；
- 全局热键与游戏 profile；
- overlay、提权、游戏进程探测；
- Kotone 归档和 eval 的产品语义；
- Kotone Svelte 页面和品牌资源；
- `~/.kotone` 配置或数据格式。

## 5. 许可证与来源门禁

两个仓库由同一所有者控制，所有者已明确允许推进复用。审计时 Kotone 尚未提交
代码库级 `LICENSE`，所有者计划补充；其 `THIRD_PARTY_NOTICES.md` 也明确说明
第三方 notices 不授予 Kotone 本身许可证。

因此：

- workspace 结构和 M0/M1 工作可立即推进；
- 正式复制 Kotone 源文件的提交应在 Kotone LICENSE 落地后进行；
- RambleDesk 对迁移文件记录来源 commit/path，避免后续无法追溯；
- 两个仓库公开发布前必须使用兼容许可证，或明确记录例外授权；
- 第三方模型许可证单独处理，不能由代码库许可证覆盖。

M3 的语音迁移 PR 必须包含 `THIRD_PARTY_NOTICES.md` 和模型 manifest，不允许把
Kotone 的模型条目无审计复制过来。

## 6. 推荐迁移顺序

1. M0 只采用 workspace/CLI/composition-root 结构；
2. M1/M2 不引入语音依赖；
3. M3 先移植 AudioBackend + CpalBackend，并用长录音压测背压；
4. 扩展 STT ports 后再迁移一个 streaming engine；
5. 用 RambleDesk 真实 ramble 语料决定默认模型；
6. 最后迁移下载器和多模型 UI。

## 7. 接受标准

语音复用只有满足以下条件才算完成：

- 10 分钟录音内存不随 PCM 时长无界增长；
- UI 消费变慢时不会造成进程 OOM；
- 设备拔出、权限拒绝和模型缺失有稳定错误码；
- 取消后采集线程、模型 session 和临时音频都被释放；
- 原始音频保留策略由用户设置决定；
- 文字输入和提交在 STT 完全不可用时仍正常工作。
