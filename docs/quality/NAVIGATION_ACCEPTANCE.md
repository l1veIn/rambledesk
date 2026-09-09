# 工作区导航与托管首发验收

> 2026-09-09–10；覆盖质量计划 Q04/Q05，以及 Q09 的前端组合缺口。
> 受测对象是 `v0.4.0-rc.3` / `b7ab9d088aa2a67322e8ee9b1c697efe9075f1ce`
> 之上的当前未提交工作区，不能把这些修复算作 rc3 标签原已具备的行为。

## 实现合同与阅读入口

- [workspaceNavigationController](../../apps/desktop/src/lib/workbench/workspaceNavigationController.ts)
  负责打开、切换、关闭、目标请求选择、加载与接受，以及后台刷新让位于用户导航。
  App 只传现有 owner 和释放组件引用的动作，不再装配 load/commit adapter。
- [navigationController](../../apps/desktop/src/lib/workbench/navigationController.ts)
  先查询 `PreparedNavigationScope`，工作区可接受后才同步提交 scope 与请求列表。
  失败和迟到候选从未成为可见事实，因此没有异步“切回旧 scope”的补偿过程。
  查询条件变化、更新的候选和过期导航均使旧候选失效；提交新 scope 后丢弃旧分页/轮询结果。
- [workspaceTransition](../../apps/desktop/src/lib/workspace/workspaceTransition.ts)
  继续持有保存、卸载、加载、接受和失败恢复的顺序。传入的 `expectedIntent`
  是同一次已保留意图，不再重复生成意图；完成后仍可检查，直到后续导航使它过期。
  自动离开条件在保存前后和加载后检查，每个 Client 仍最多一个可编辑 Editor。
- [startupController](../../apps/desktop/src/lib/workbench/startupController.ts)
  把恢复 resolutions 放进现有可订阅状态，不依赖隐藏 getter 触发 UI。
  `dispose()` 使迟到初始化/恢复失效；已经关闭的 Client 不会再通过 `onReady` 启动轮询。
- [managedSessionActions](../../apps/desktop/src/lib/workbench/managedSessionActions.ts)
  保留 prepared controller 缓存、晋升、归档与删除的责任；开页使用导航 owner。
  晋升后只更新同一活动 Agent 页的 scope，保留已经挂载的 composer 和未发送文字。

没有增加通用状态机、新的运行时或第二份可写 Workbench facts；单 Editor、后端 revision/CAS
和 prepared controller 的首发接纳算法继续保留。原先供 App 接线的 load/commit、重复 scope rollback、无用
context 参数与 Startup 的 raw store `set` 已移除。

## 自动化结果

2026-09-09 23:31（Asia/Shanghai），以下命令通过：**9 个文件、77 条测试**。
其中前 8 个文件是导航与启动的定向回归（76 条）；最后一个文件是实际挂载 App 的托管首发旅程。
测试数仅记录这次证据，不作为质量目标。

```sh
pnpm --dir apps/desktop test \
  src/lib/workbench/navigationControllerScope.test.ts \
  src/lib/workbench/navigationControllerRequests.test.ts \
  src/lib/workbench/navigationControllerInbox.test.ts \
  src/lib/workbench/navigationControllerStartup.test.ts \
  src/lib/workbench/workspaceNavigationController.test.ts \
  src/lib/workspace/workspaceTransition.test.ts \
  src/lib/workbench/startupController.test.ts \
  src/lib/workbench/managedSessionActions.test.ts \
  src/App.managedFlow.test.ts
```

| 合同 | 主要证据 | 观察结果 |
| --- | --- | --- |
| 保存后再接受目标 | workspaceNavigationController / workspaceTransition | 原草稿保存后，workspace、活动页签、scope 与新草稿一致 |
| 保存失败与重试 | workspaceNavigationController | 原文字、页签和 scope 保留；未卸载 Editor；重试可以完成切换 |
| 加载失败恢复 | workspaceNavigationController | 原 Editor 恢复，候选 scope 不曾暴露，pending 清除 |
| 快速 A→B→C | workspaceNavigationController | 迟到 B 不覆盖 C；B 与 C 均失败时仍留在已提交 A |
| 迟到 scope 与普通页面 | workspaceNavigationController | Settings 抢先生效后，旧 B 不能改写 rail 或重新打开反馈页 |
| scope 查询与旧轮询 | navigationControllerScope | 查询结果直到接受前不可见；条件变化使候选失效；旧轮询不覆盖新 scope |
| 关闭与恢复 | workspaceNavigationController | 后台页签无需保存；活动页签保存后加载 fallback；最后页关闭清空 workspace/scope；缺失会话展示空的解释视图 |
| 自动打开与后台刷新 | workspaceNavigationController / workspaceTransition | 自动打开被阻止后可重试；开始等待的刷新不抢占后续用户导航；自动离开条件在加载后再次检查 |
| prepared 晋升期间关闭 | workspaceNavigationController | 关闭晋升后的真实 view，不留下临时页签，不误删会话 |
| 状态订阅与销毁 | startupController | 同一 active key 的 recovery 变化通知订阅者；销毁后初始化不触发 ready/轮询 |

`workspaceNavigationController.test.ts` 使用真实 Navigation / Workspace / Draft / Shell sessions、
真实 Draft Controller 与真实 transition；只在 typed transport 处控制响应顺序和故障。
底层 transition 的独立测试继续验证 Editor 挂载上限；实际 DOM 和 Editor 组合由 App 测试承担。

## 保存广播保留当前 Editor 的历史

2026-09-10 的实际 Chrome 验收发现：编辑后 Undo 可以使用，但自动保存后再次变成 disabled。
`DraftController.acceptSaved` 本身没有推进 editor epoch；问题发生在后端的 `feedback_workspace`
invalidation 回到浏览器后，refetch 无条件 navigate 同一 request，卸载 Editor 并重新 adopt 文档。

[App.editorRefresh.test.ts](../../apps/desktop/src/App.editorRefresh.test.ts) 挂载真实 App、Editor 与一个
会在保存后发送应用事件的 `PreviewApplicationTransport` 子类。旧代码的红灯是保存后原 Editor DOM
`isConnected === false`。普通 PreviewTransport 的订阅没有事件，原有 autosave 测试不能覆盖这条路径。

修复后，refetch 先等待保存，再读取并核对当前 request/意图、revision 和结构化文档：

- 服务端文档等于当前文档，或等于同 revision 的已知保存基线时，只协调 workspace/draft 事实；不卸载 Editor、不推进 epoch。
- 读取期间出现的新输入保持 dirty；保存基线更新不覆盖它，当前 selection 和 Undo/Redo 保留。
- 真正改变的远端文档与终态继续由现有 transition 加载；用户已切换请求时，迟到响应直接失效。

2026-09-10 00:44（Asia/Shanghai），以下定向命令通过：**4 个文件、34 条测试**；类型检查为
0 errors / 0 warnings。包含真实 App 保存后点击 Undo/Redo，以及相同文档、外部新文档、读取中新输入、
读取中切请求四项导航合同；不以测试数量替代真实浏览器复验。

```sh
pnpm --dir apps/desktop test \
  src/lib/workbench/workspaceNavigationController.test.ts \
  src/lib/workspace/workspaceTransition.test.ts \
  src/App.editorRefresh.test.ts \
  src/lib/editor/RichFeedbackEditor.test.ts
```

## Q09：保留 prepared 设计，补全真实组合证据

现有 [draftManagedSessionController](../../apps/desktop/src/lib/agents/draftManagedSessionController.ts)
已经持有 prepared 资源、首发接纳不确定性、后续输入与晋升规则。当前没有证据支持把它改成另一套
通用任务/状态引擎；本轮保留它的生产实现，补调用方到真实 UI 的缺口。

[App.managedFlow.test.ts](../../apps/desktop/src/App.managedFlow.test.ts) 挂载真实 App、Draft Workspace、
Agent Composer、prepared controller、导航与正式 Managed Session Workspace。测试使用
`PreviewApplicationTransport` 子类制造可控应用响应，没有 mock Tauri 或替换业务 controller。

同一个旅程证明：

1. 首发已在模拟服务端接受，但响应丢失且查询暂时仍返回 prepared；UI 提供显式接纳检查，原首发仍有本地恢复内容。
2. 未知期间继续输入下一条消息；关闭页签先检查接纳，不能确认时保留原页和新文字，不 discard/delete 会话。
3. 查询终于确认 active 后，通过显式检查晋升一次；首条消息只发送一次，下一条输入进入正式会话 composer。
4. 关闭正式 Agent 页只释放 Client 订阅；没有 stop/discard/delete 命令，模拟服务端仍处于 running。
5. 整个 App 卸载后，该 transport 的订阅数归零。

## 证据限制与后续验收

这些测试证明确定性状态、异步顺序和真实组件组合，不证明真实 ACP 模型接纳、网络断连、原生进程
释放、手机键盘或触控体验。托管旅程中的“服务端已接受”由 typed transport 夹具模拟；真实持久化与
进程隔离证据见 [后端验收](BACKEND_ACCEPTANCE.md)，真实 Agent 仍按 Q19 单独登记。

Q05 的真实浏览器窄视口、键盘和焦点记录由对应 UI 验收补充；不能用 jsdom 的通过填写真机通过。
