# ADR 004：单 Editor 结构化 Feedback Draft

- 状态：Accepted
- 日期：2026-08-28
- 替代：0.3.3-rc 的 FeedbackDraftSession / hidden Editor / 自动 Light cleanup 路线

## 背景

0.3.2 只有一个可编辑 `RichFeedbackEditor`。程序性输入按明确的 `requestId` 在当前 Editor transaction 和后台草稿写入之间二选一。

0.3.3-rc（PR #13）验证了真实产品需求：草稿要保存富文本结构、`@Action` 归属、ASR 节点身份，以及 Active Ramble 在用户查看其他 request 时仍能写入。它同时引入了错误假设：后台 request 要持续接收结构化内容，因此必须持续保留自己的 Editor。

该假设带来了 `SessionDraftEditor`、hidden Editor、`draftSessionHost` / `MAX_MOUNTED`、`bindEditor` / `capturedEditor`，以及与 Editor 生命周期耦合的自动 cleanup。补丁不断修复切换与竞态，但没有消除复杂度来源。

## 决策

实现路线失败，产品需求没有失败。0.3.2+ 恢复 0.3.2 的所有权模型，并加入经过验证的结构化能力：

1. 整个应用最多一个可编辑 `RichFeedbackEditor`。允许固定的 Svelte `bind:this` 作为命令入口，禁止 request/session 与 Editor 的动态绑定。
2. `document_json` 是 Feedback Draft 的 canonical representation；`body_markdown` 是同一份 Document 的派生投影。
3. 程序性输入必须携带 `targetRequestId`，再进入 `routeDraftOperation`：当前 request 走 Editor transaction，后台 request 走 TipTap JSON transformation + 单一 `rambleDocumentQueue` + CAS。
4. Action 归属用标准 Blockquote 容器表达，不再给每个内容节点盖章，也不计算相邻节点的视觉连续区间。
5. ASR 段落带有稳定 `speechSegmentId` 和 `pending` / `cleaned` 状态。Tidy 完全手动，只处理当前 Editor 中的 pending 节点，使用严格 `[n]` 标签协议。
6. `setContent` 只用于 request 级 load/reload。程序性插入不得重置 selection、IME 或 undo history。

## 明确不做

- hidden Editor、per-request Editor、session 持有 editor handle
- 自动 Tidy、idle timer、stop/settle cleanup
- Action 相邻节点扫描、decoration grouping、reopen-group 修补
- 把 Markdown 再变成第二真源
- 从 PR #12 恢复 capture marker / clip 模型

## 后果

后台 Ramble 不再依赖第二份 Editor 存活。切换 request 时先保存当前草稿，再加载目标 Document。截图或附件在切走后仍按原 `requestId` 写入后台 JSON，而不是误写当前 Editor。
