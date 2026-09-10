# Web 性能与资源验收

这里维护可重复的测量方法和预先固定的预算。现有结果及未覆盖场景集中在[质量清单](README.md)；2026-09-10 的逐次调试、原始样本与统计保留在[固定历史快照](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/PERFORMANCE_ACCEPTANCE.md)。历史通过不代表当前构建自动通过。

## 准备受测对象

[性能入口](../../apps/desktop/quality-benchmark.html) 加载真实 App，通过现有 DOM 操作真实 HTTP/SQLite 夹具；不导入业务 store 或私有编辑器实例，也不替换 transport。

```sh
RAMBLEDESK_QUALITY_BENCHMARK=1 pnpm --filter rambledesk-desktop build:web
python3 scripts/feedback-acceptance.py start --keep --dist /absolute/path/to/frozen-production-dist
```

先将完整 `apps/desktop/dist` 复制到独立目录，再把该路径传给 launcher。受测期间不得覆盖它。按[夹具指南](FEEDBACK_ACCEPTANCE.md)认证，使用英文，结束 onboarding，保持 ordinary 与 long 两条请求可见，打开同源 `/quality-benchmark.html`。此入口仅在显式 opt-in 构建中出现。

每份结果附上 launcher manifest、完整 dist 清单及哈希、源码 SHA 和工作区差异、设备/浏览器版本、viewport、起止时间及原始样本。动态导入 shim 的体积不能当作整个 App bundle。

普通草稿与 240 段长草稿使用固定 seed。长文不等于含附件长文，切换也不覆盖真实录音、附件预览、多页签或 Agent 生成；这些场景单独验收。

## 固定预算

预算用于本机 Web 场景。P95 取排序后第 `ceil(0.95 × n)` 项；保留原始样本、最大值和失败状态。

| 指标 | 样本 | 预算或调查线 |
| --- | --- | --- |
| reload → 可编辑 DOM + 两帧 | 5 | median ≤ 1500 ms |
| 普通 / 长草稿切换 → 正确 DOM + 两帧 | 各 30 | P95 ≤ 300 / 500 ms |
| 普通 / 长草稿编辑 → marker 可见 + 两帧 | 各 30 | P95 ≤ 100 / 200 ms |
| 普通 / 长草稿编辑 → 新 Saved revision + 两帧 | 各 30 | P95 ≤ 2000 / 3000 ms |
| 长 Agent 历史进入 / 真实 HTTP 分页 | 各 30 | P95 ≤ 500 / 750 ms |
| 展开历史过程 / 工具详情 | 各 30 | P95 ≤ 200 / 200 ms |
| ObjectURL outstanding / Worker withoutObservedTerminate | 30 分钟 | 同视图热身基线至结束增量 ≤ 0；需有真实创建活动才验证生命周期 |
| App DOM、元素与 attached editor views | 逐分钟及结束 | 同视图增量 ≤ 0；可编辑反馈 DOM 为 1，只读预览另计 |
| 可用时的 JS heap | 逐分钟及结束 | 增长超过 `max(10 MiB, 10%)` 或低谷持续上移，触发调查 |

同条件 median 恶化超过 10% 时先复测排除噪声。不同 viewport、构建、harness 或 cache 条件不能直接归因于产品回归。保存时间包含 autosave debounce 与真实持久化；输入时间不是 INP 或硬件键盘延迟。未控制 GC 的 heap 变化不自动等于泄漏。

## 三项反馈测量

三项操作手动启动、互斥运行。保持页面前台，不在测量中手动编辑、关闭请求或切换语言。隐藏、提前停止、reload 中断应保留对应状态，不能当作完整通过。

1. **Reload / 切换。** 点击 `Measure fixture: 5 reloads + 60 switches`，完成 5 次 reload 和普通/长草稿各 30 次切换；从 `#quality-results` 导出完整 JSON。认证已在测量前完成，浏览器 cache 未受控，因此结果不是 OS 冷启动或原生启动时间。
2. **资源观察。** 点击 `Start 30-minute resource observation`（`#quality-resource-run`）。先热身并在普通视图静置 3 秒，然后约每 5 秒切换、每分钟回同一视图采样，最后再静置 3 秒。完整运行目标是 31 个快照；记录实际时间，不补造错过的样本。使用 `#quality-resource-export` 导出完整结果，摘要或中断 checkpoint 不能替代原始 JSON。
3. **编辑 → 保存。** 在独立夹具或所有只读测量之后，点击 `Append + save: 30 ordinary and 30 long edits`（`#quality-edit-run`）。每条草稿新增 30 个唯一 marker，不自动清理数据库。标准 DOM Selection 与浏览器编辑命令执行真实 contenteditable 编辑；必须观察到 input、marker、新保存 revision 及重新打开后的 marker，否则失败。通过 `#quality-edit-export` 导出结果。SQLite/包字节核对仍由夹具 `verify` 单独完成。

再次比较未编辑的固定 seed 时，必须新建夹具，不能把已追加 marker 的数据库当作相同基线。

资源探针在 App import 前安装，观测当前 window 的 ObjectURL create/revoke、Worker 构造和显式 terminate、可用的 `performance.memory` 与 App 内 DOM。它不强引用 Worker，导出自身的 URL 不计入 App。`withoutObservedTerminate` 不等于 OS 存活线程；Worker 自行 close、其他 realm、脱离 DOM 的 editor、订阅、原生进程及 App 退出释放均不在这些计数的证明范围内。创建数始终为 0 表示未触发，不能算生命周期通过。

编辑采样不模拟逐字输入、IME、触摸键盘或拖选；浏览器拒绝编辑 API 时保留失败，不通过私有 editor API 兜底。

## 独立长历史测量

[managed_preview quality 模式](../../crates/rambledesk-local-server/examples/managed_preview.rs) 将确定性活动写入真实 repository，再经现有 Application / HTTP 分页读取。它不发模型 prompt，内容明确标记为 synthetic fixture。

```sh
cargo build --locked -p rambledesk-local-server --example managed_preview
RAMBLEDESK_MANAGED_PREVIEW=1 RAMBLEDESK_MANAGED_PREVIEW_QUALITY=1 RAMBLEDESK_MANAGED_PREVIEW_DIST=/absolute/path/to/frozen-production-dist target/debug/examples/managed_preview
```

每次创建独立临时数据库和项目，使用随机 loopback 端口。输出 `PREVIEW`、`PREVIEW_QUALITY_MANIFEST` 和 `PREVIEW_TOKEN_FILE`；凭证只在临时文件中，Unix 权限为 0600。停止前复制 manifest 和所需结果，记录 dist、binary、source 哈希。停止只创建本次 manifest 的 `stopFile` 或向该实例发送 Ctrl+C；退出会清理临时目录。

`Website project` 包含 60 turns、300 条固定活动，`CLI project` 用作相邻会话。最近 100 条和两页更早历史由真实 HTTP 读取；数据库条目数不等于同时挂载的 DOM 数。

认证并切为英文后，从两项目的 `View Agent` 打开真正的 Agent 页签。展开 `Managed history · separate quality fixture`，点击 `#quality-history-run`；普通 Ramble scope 页签不能替代 Agent 页签。通过 `#quality-history-export` 导出完整 JSON。

[历史探针](../../apps/desktop/src/dev/qualityManagedHistoryBenchmark.ts) 每次重新进入会话，确认 session、标题及 041–060 的初始窗口，再测过程、工具展开和分页至 021–060。分页必须观察到一条真实 `listManagedSessionActivity` 网络传输及正确内容，不能把缓存展开计为 HTTP。缺少网络证据保留 `unverifiedPage`，四项各 30 次完整样本才算 `done`；前置条件不足是 `setup-required`。

这些测量说明前端历史读取与展示成本，不说明真实 Agent 的生成速度。未触发媒体资源、真实设备与 retained heap 的剩余工作继续列在[质量清单](README.md)。
