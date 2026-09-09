# Q06 / Q08：语音结束与反馈终态验收

2026-09-09，本记录针对 `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce` 之上的未提交工作区改动。受测平台为 macOS 26.3.1 arm64、Node v22.23.0；受测产物为源码测试，不是打包 Desktop 或真实录音设备。

## Q06：语音会话的异步归属

实现与测试：[voiceRambleSession.ts](../../apps/desktop/src/lib/workbench/voiceRambleSession.ts)、[voiceRambleSession.test.ts](../../apps/desktop/src/lib/workbench/voiceRambleSession.test.ts)。

先写失败用例，再修复已复现的行为：

| 触发窗口 | 修复前证据 | 修复后结果 |
| --- | --- | --- |
| 取消旧启动、开启新录音后，旧 `ready` 拒绝 | 新 `sessionId` 被清空，phase 变为 error | 旧启动返回 false，新录音保持 listening |
| `cancel` 在途时收到 stable 尾段 | 已取消的文字进入真实 SpeechDraftQueue 并开始写入 | 取消立即使旧事件失效；结束后可以再启动 |
| 新录音开始后旧订阅触发 `onError` | 新录音被置为 error | 旧订阅回调不再改变新状态 |
| 旧 `stop` 在新录音启动后成功或失败 | 成功被当成当前操作完成；失败改写新 phase，并将新音量归零 | 旧完成返回 false，不改变新录音 |
| starting 期间主动 stop，随后 started / ready 到达或 ready 拒绝 | 正常停止被恢复成 listening 或误报启动失败 | 保持停止意图，不把迟到启动当作成功录音或新故障 |

同一 controller 用 generation 校验回调与每次 await 后的写入；`reset` 和 `cancel` 使旧归属失效。新增 `createVoiceRambleState()` 与可注入同一 store 的构造参数，原默认调用保持兼容。Ramble 与麦克风共享事实的装配由 [rambleSession.ts](../../apps/desktop/src/lib/workbench/rambleSession.ts) 负责。

保留合同也有行为测试：

- graceful stop 仍接收最后稳定段；串联真实 SpeechTargetTracker 与 SpeechDraftQueue，在写入未完成前不结束 stop。
- 开始说话时绑定的 request 不会因为界面目标后来变化而被改写。
- 旧取消操作在 reset / restart 后完成，不会清空替代录音。
- 设备、worker、模型与底层释放继续由 speech capability 管理；本改动没有替换 Browser/Tauri 的识别实现。

证据边界：speech capability 使用受控 fake 安排 ready、stop、cancel、event 的顺序；tracker、queue、voice store 是真实实现。测试不证明真实麦克风权限、录音硬件、后台音频或模型转写质量，也不将启动/停止测试等同于原生窗口生命周期验收。

### 准备接口的组合复核

[rambleInputController.test.ts](../../apps/desktop/src/lib/workbench/rambleInputController.test.ts) 挂载真实
[RambleSessionController](../../apps/desktop/src/lib/workbench/RambleSessionController.svelte)，补齐底层 voice
独立测试与终态调用方之间的两个窗口：

1. **启动中准备提交。** 旧 `exitRamble` 与 toggle 共用 single flight，只加入 start 的 Promise，根本未执行退出。
   红灯观察到 stop 预期 1 次、实际 0 次。现在退出意图独立持有 `exitFlight`，先等待启动，再执行停止与排空；
   最后 stable 段保存完成前不能返回 ready，toggle / retry 也不能在退出期间重启麦克风。
2. **排空期间新接受 clipboard。** 旧单次快照等待会在新 capture 仍未返回时给出 ready。现在持续纳入退出期间
   新接纳的 import。进一步的失败分支先复现“写入 reject 却仍 ready”，再以本次参与 capture 的身份保留错误；
   本次失败返回 failed，过去已结束的失败不会永久阻挡后来的准备。

2026-09-09 23:55（Asia/Shanghai）最终 **6 条 mounted input 测试通过**，包含上述先红后绿场景，以及既有
capture 单次接纳、失败传递和销毁后候选释放。终态 Controller 无需读取输入内部细节，只依据
`prepareFeedback(requestId)` 的 ready / pending-speech / failed 决定是否继续。
详细场景与证据边界见 [输入验收](INPUT_ACCEPTANCE.md)。

## Q08：批准 / 取消先完成输入与保存

实现与测试：[submissionController.ts](../../apps/desktop/src/lib/workbench/submissionController.ts)、[submissionController.test.ts](../../apps/desktop/src/lib/workbench/submissionController.test.ts)。

原实现会跳过保存直接批准或取消；取消还把未保存文档的显示阶段改成 saved。真实 WorkspaceSession、DraftSession、DraftController 与受控 ApplicationTransport 的用例先复现该问题，之后验证完整序列：

1. 同步预留批准 / 取消共用的单个操作；同类重复调用共享未完成 Promise，不同类调用不误发另一终态。
2. `prepareFeedback(requestId)` 返回 ready、pending-speech 或 failed。最终语音仍须进入可编辑草稿，所以这一阶段只保留操作归属，不提前锁住文档写入。
3. 输入准备完成且目标仍相同后锁定文档，等待 `saveDraftNow()` 的真实保存结果。
4. 保存成功且 request 仍可操作才调用终态命令。保存失败保留原文、dirty 状态和错误，不批准、不取消、不伪装 saved。
5. 后端终态立即进入 WorkspaceSession。后续导航刷新失败单独报告，不撤销终态，也不让重复点击再次发送。

测试还验证准备中切换 request、终态响应迟到时切换 request、停止输入失败与待审语音、保存中编辑锁，以及未知响应后的显式重试。未知响应不会自动重发；显式重试继续使用既有后端幂等终态接口。底层幂等、取消不产生托管续接等证据另见 [BACKEND_ACCEPTANCE.md](BACKEND_ACCEPTANCE.md)。

保留原批准确认文案和 `window.confirm` 默认行为，同时提供可注入的 `confirmApproval`。反馈包打开 / 下载仍委托既有 PublishedFeedbackAction；没有更改终态 API、取消 reason、包格式或外部适配器协议。

## 实际执行结果

| 命令 | 结果与范围 |
| --- | --- |
| `pnpm -C apps/desktop exec vitest run src/lib/workbench/voiceRambleSession.test.ts src/lib/speech/speechTargetTracker.test.ts src/lib/speech/speechDraftQueue.test.ts src/lib/workbench/rambleSession.test.ts src/lib/workbench/rambleSessionControllerRender.test.ts` | 执行时 5 文件、48 条通过；其中 voice 17 条 |
| `pnpm -C apps/desktop exec vitest run src/lib/workbench/submissionController.test.ts src/lib/workbench/draftController.test.ts src/lib/workbench/draftSession.test.ts src/lib/workbench/workspaceSession.test.ts` | 4 文件、43 条通过；其中 submission 22 条 |
| `pnpm --dir apps/desktop test src/lib/workbench/rambleInputController.test.ts` | 23:55 复核后 1 文件、6 条通过；启动中退出、新 import 及其失败分支先红后绿 |
| `pnpm -C apps/desktop check` | 整合当时 0 errors / 0 warnings |
| `node scripts/check-frontend-module-size.mjs` | 通过；执行时 608 文件、700 行限制、1 个既有例外 |
| `git diff --check`（限定本分支修改文件） | 通过 |

以上是本次已执行的结果，不是测试数量目标。后续整合引起的文件和用例数量变化以最终工作区门禁为准。真实 App、HTTP/SQLite、真实设备与打包平台仍按总计划各自验收，不能用本记录替代。
