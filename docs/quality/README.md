# 质量与待验清单

这里记录当前还需要验证什么，以及如何复验。已有结果来自 2026-09-09 至 09-10 的指定构建，不代表未来提交自动通过；文档整理没有重新运行产品验收。

## 目标与完成标准

全局质量收敛的目标是让数据、指令和资源各有明确的所有者，以最少必要实体完成可靠的反馈闭环；性能改动由实际测量驱动。M1–M4 的工程整理已落地，M5 尚有资源观察缺口，M6 尚有平台与真实 Agent 必验项。只有必验完成、待合并版本的检查通过，才关闭全计划并重新评分。

## 当前状态

| 范围 | 已有证据 | 仍需完成 |
| --- | --- | --- |
| 工程与反馈合同 | `27b50f1` 收拢保存、导航、输入、发布和生命周期；当时前端 1330、Rust 544 条测试通过 | 待合并 SHA 的跨平台 CI 单独检查，不能沿用历史绿灯 |
| macOS 核心 UI | 独立数据的 debug/ad-hoc Quality App 完成编辑、提交和包核对 | 正式安装/升级、设备权限与中断、托盘、自启动及完整原生手势 |
| Chrome / Safari Web Access | 编辑保存、刷新、导出，断线后的草稿恢复，双向 CAS、令牌轮换；Unix 启动不再依赖钥匙串 | 真实图片剪贴板、其余媒体与设备生命周期；支持范围以 [矩阵](../WEB_ACCESS_SUPPORT_MATRIX.md) 为准 |
| Windows | 现有平台代码与自动化覆盖 | 安装、升级与原生设备操作；用户明确暂无环境，保留必验 |
| 手机 | 桌面浏览器窄视口中的抽屉、编辑、附件与设置布局 | 至少一台真实手机的软键盘、触控、旋转、安全区；图片预览关闭后的焦点归还仍需补验 |
| 主要真实 Agent | Claude/Codex 的握手与模型参与有证据；Claude 在权限请求处停止，Codex 首轮 IPC 失败，修正后复验在 initialize 阶段退出 | 请求 → 反馈 → 继续 → 第二轮 → 原会话 ID 恢复尚未通过；不能用 fixture 或握手补齐 |
| 真实 Cooking | 已有配置下真实模型 → 独立变体 → 发布 → HTTP/SQLite/包哈希核对通过 | 原生可视流程、浏览器 CORS 与其他提供商不在这次证据范围内 |
| 性能与资源 | 已测编辑保存、切换、长历史预算通过；30 分钟观察 DOM 稳定，heap 净增约 7.71 MiB，低于登记的数值调查线 | heap 低谷仍有上移；未取得同 GC 条件的 retained heap；录音、附件预览、ObjectURL、Worker、订阅和多页签释放仍需单列 |
| Browser ASR | 合同自动化与 pilot 路径 | 真实设备仍为观察项，不扩大成全面兼容承诺 |

工程部分已交付；这些必验缺口未关闭前，原全局质量计划的 M5 / M6 仍未完成，不给出完整总分。合并代码、发布版本与产品验收是不同的状态。

## 条件性后续工作

以下保留为有前提的设计方向，不作为已实现能力：

- 正式 ACP 连接的 idle GC：先建立反馈创建、等待与投递的保活合同及竞争保护，再验证原 remote ID 恢复；当前 disconnected 的投递 worker 不主动启动 Agent。
- 内部 scope 简化：先把撤销、排空与隔离证据迁到 JSON 入口，再移除旧的内部形态。
- 完整包体与超大历史：先做 profiling，再决定拆包、内存窗口或虚拟列表；不能用一般规模的测量保证无限展开历史。

## 按任务复验

- [隔离反馈夹具](FEEDBACK_ACCEPTANCE.md)：真实 HTTP、SQLite、CAS、发布包与浏览器下载。
- [性能测量](PERFORMANCE_ACCEPTANCE.md)：固定构建、数据集、预算和资源观察。
- [ACP 后端探针](../ACP_BACKEND_PROBE.md)：真实 Agent、双会话隔离、续接和恢复；fixture 不是模型证据。
- [质量阅读地图](QUALITY_WALKTHROUGH.md)与[反馈链路示范](../FEEDBACK_FLOW_WALKTHROUGH.md)：找行为的责任模块。
- [发布检查](../RELEASE_CHECKLIST.md)：产物、平台与版本交付。

每次新增结果记录场景、源码/构建标识、平台、步骤、预期、实际、通过/失败/未验，以及必要的日志或哈希。凭据和用户正文不进入文档。当前结果更新到这张表；原始截图、导出与逐次排查放本地 `.local-artifacts/`，需要长期引用的证据单独选取。

## 历史追溯

完成的计划、逐轮排查和原始 JSON 已从当前目录移出。它们仍保存在整理前的 Git 快照中；以下为固定 commit 链接，不是会随分支变化的当前说明。

- [原全局计划与实施账本](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/PROJECT_QUALITY_PLAN.md)：M0–M6、评分口径和交付 SHA。
- [工程记录](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/ENGINEERING_ACCEPTANCE.md)与[原生/浏览器记录](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/NATIVE_BROWSER_ACCEPTANCE.md)：各构建的证明边界。
- [真实 Agent 失败记录](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/AGENT_ACCEPTANCE.md)与[Cooking 记录](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/COOKING_ACCEPTANCE.md)。
- [性能原始记录](https://github.com/l1veIn/rambledesk/blob/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/PERFORMANCE_ACCEPTANCE.md)与[完整证据目录](https://github.com/l1veIn/rambledesk/tree/b4273fae2eeae964f427dce87c6c64b6edd863a7/docs/quality/evidence)。

本地可用 `git show b4273fa:docs/PROJECT_QUALITY_PLAN.md` 读取原文。文档收拢没有把失败或未验项改成通过。
