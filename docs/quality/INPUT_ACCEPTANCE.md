# 人类输入与附件验收

> CURRENT：2026-09-09，记录本轮输入合同的实现、验证范围与待验项。附件部分对应 Q07；这里不代替原生设备或手机验收。

## 提交前输入准备：Q06 / Q08

[RambleSessionController](../../apps/desktop/src/lib/workbench/RambleSessionController.svelte) 对外提供
`prepareFeedback(requestId)`，返回 `ready`、`pending-speech` 或 `failed`。
Publisher、Cooking 预览与批准/取消等待这一结果后才锁定并保存草稿。待审语音和已接受文档写入仍使用
各自原有队列；调用方不再自己拼接停止、队列等待和若干状态 getter。

本轮独立审查通过真实 mounted controller 复现并修复两个准备窗口：

| 触发窗口 | 修复前红灯 | 当前结果 |
| --- | --- | --- |
| Start recording 的 `ready` 尚未返回时准备提交 | start 结束后 `stop` 调用为 0，准备却能 ready；退出回调被同一个 single flight 吞掉 | 独立 exit flight 先等待 start，再真正 stop；最后 stable 段写入完成前准备不结束，随后 microphone / Ramble 回到 idle |
| 已经等待旧 document queue 时接受新的 clipboard import | capture 尚未返回，prepared 已为 true；后来的提交锁可使该输入被丢弃 | 同一 exit flight 继续排空期间新接纳的 capture、speech 与 document 写入 |
| 上一行的新 import 最终写入失败 | 补上排空后仍返回 ready：入口处没有 pending capture，失败未被算入本次准备 | 以实际参与 capture 的身份保留失败；返回 failed 和写入错误，不被较早的空快照或后续成功覆盖 |

第三行是第二个窗口的失败分支，补测后进一步修正了失败归属。旧操作已报告的失败不会永久阻挡
下一次准备；本次参与的失败必须返回给终态调用方。toggle / retry 在退出过程中加入退出，不再另起录音。

2026-09-09 23:55（Asia/Shanghai），[rambleInputController.test.ts](../../apps/desktop/src/lib/workbench/rambleInputController.test.ts)
**6 条通过**。其中上述三条均观察过红灯；其余合同验证既有 capture 的单次接纳和写入等待、参与写入失败、
销毁后迟到 candidate 的释放。测试挂载真实 Ramble 组件及 voice/session/queue，实现仅在设备 capture / speech
和文档写入边界控制 Promise 与事件；ResizeObserver 使用 jsdom polyfill，不伪装实际录音设备。

```sh
pnpm --dir apps/desktop test src/lib/workbench/rambleInputController.test.ts
```

麦克风 generation 以及批准/取消保存顺序的其他证据见 [终态验收](TERMINAL_ACCEPTANCE.md)。

## 附件：Q07

附件控制器继续持有取得候选、持久化、文档插入和状态收据这一完整操作。
`AttachmentSession` 只提供可订阅状态和明确动作，已移除为跨组件 `bind:` 保留的 raw `set`；
Ramble 组件读取 busy，并通过消息回调报告错误。

| 行为 | 当前合同与结果 |
| --- | --- |
| 文件、粘贴、截图候选、server paths、删除与排序 | 同一控制器写入队列串行仲裁；busy 持续到所有已入队操作结束 |
| 多附件批次 | 每个已接受附件先插入目标文档并保存，再读取下一次上传的 revision；后续失败保留前面的实际成功 |
| 请求和 Action 归属 | 取得候选时固定目标；切换页面不把后台附件改投当前请求 |
| 终态与销毁 | 已知终态先拒绝候选；销毁后的字节或请求返回不能继续写入、复活预览或覆盖新控制器状态 |
| 上传响应丢失 | 只读 refetch 刷新附件事实；返回失败，不重发上传，不猜测哪个新附件应该自动插入 |
| 删除/排序前等待保存 | 等待后重新核对目标；不能把新页面的 revision 用于旧请求 |
| 截图启动 | 在等待草稿保存前占用 capture busy，连续点击只开始一次 |
| 预览 URL | 独立资源 owner 处理 read generation；过期读取只释放自己新建的 URL，不释放仍在显示的 URL |
| 可选预览读取 | 不阻塞已持久化附件的文档插入与保存收据；视图释放使晚到读取失效 |
| 预览对话框 | readKind 进入加载身份；旧系统打开/定位结果不写入新附件，也不在关闭后提示旧错误 |
| 对话框图片解码 | 关闭时保留仍被 pending decoder 使用的候选 URL，解码结束后释放；显示 URL 在 DOM 不再使用后释放 |

响应不明或插入失败时，附件列表与错误保留可见的实际状态。可以检查已有附件、删除不需要的附件、
恢复草稿保存后重试相应动作；控制器不提供未经后端支持的上传幂等保证。
候选处置继续依照 `AttachmentCandidate.dispose()` 的幂等合同执行。

入口：

- [Attachment Controller](../../apps/desktop/src/lib/workbench/attachmentController.ts)
- [Preview 资源生命周期](../../apps/desktop/src/lib/workbench/attachmentPreviews.ts)
- [预览对话框](../../apps/desktop/src/lib/workspace/RequestAttachmentPreview.svelte)

## 验证记录

基于 `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce` 之上的未提交工作区。
新增 17 条行为场景；附件相关 6 个测试文件、39 条测试通过：

```sh
pnpm --filter rambledesk-desktop test src/lib/workbench/attachmentControllerLifecycle.test.ts src/lib/workbench/attachmentControllerPreviews.test.ts src/lib/workbench/attachmentControllerCapture.test.ts src/lib/workbench/attachmentControllerWrites.test.ts src/lib/workbench/attachmentControllerImport.test.ts src/lib/workspace/RequestAttachmentPreview.test.ts
pnpm --filter rambledesk-desktop check
```

类型检查为 0 errors / 0 warnings。测试使用真实 AttachmentSession、typed transport 边界、受控异步
顺序和真实挂载的预览组件；对照修改前复现了预览旧结果/释放顺序、候选归属与销毁、批次失败和
对话框旧回调等失败。已有捕获、多个 server paths、Action 归属与候选只处置一次的合同继续通过。

真实 HTTP/SQLite/发布包与附件哈希入口见 [隔离反馈验收](FEEDBACK_ACCEPTANCE.md)。它证明后端和
持久化合同，不取代本节尚未执行的真实 UI 输入验证。

## Q15 局部交互与待验

附件预览对话框改用动态视口高度；手机宽度使用四边 safe-area inset，coarse pointer 下对话框
按钮和附件卡按钮保留至少 44 px 目标。焦点陷阱、Escape 和焦点返回继续使用现有 Dialog primitive。

上述组件统计是代码与挂载组件验证。9 月 10 日主任务另有实际输入观察：Native 和 Chrome 文件选择器
选择 51 B TXT，Chrome 实际下载 JSON，Safari 预置 Markdown 附件 modal 显示 H2。
[Chrome 下载包核对](evidence/chrome-download.json) 和 [Safari 下载包核对](evidence/safari-download.json)
已独立存档；包哈希核对本身不证明所有选择、粘贴或预览手势。

以下场景仍不能填写“通过”：

- 图片粘贴、Safari 新文件选择、图片预览与各浏览器尚未执行的手势组合。
- 原生截图/overlay/pin、权限拒绝、系统文件打开与定位。
- 手机真机键盘、旋转、safe area 与触控焦点；用户明确没有手机验收环境，保留待验。
- 浏览器真实图像解码和大图缩放的设备表现；JSDOM 的受控 decoder 只证明释放顺序。

这些验收按 [全局质量计划](../PROJECT_QUALITY_PLAN.md) 的属性继续记录，不扩大浏览器截图或
LAN/TLS 的产品范围。
