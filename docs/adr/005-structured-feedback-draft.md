# ADR 005：结构化 Feedback Draft 是未终态正文的持久化真相

- 状态：Accepted
- 日期：2026-08-26
- 取代：ADR 004 第 4、5 节中“崩溃重启只恢复已保存 Markdown”和“Draft 仍是整篇 `body_markdown`”

未终态反馈请求必须在重启后完整恢复 Feedback Draft 的文档树、节点类型、属性和 marks，因此 SQLite 原子保存版本化的 TipTap 文档 JSON 及由同一文档生成的 Markdown 投影，并以结构化文档作为继续编辑的持久化真相。Markdown 投影只服务于 Cooking、提交和兼容旧草稿；已终态请求的历史展示直接渲染不可变反馈包中的 `feedback.md` / `uncooked.md`，不再依赖 live draft，也不从 Markdown 分隔线反向重建 TipTap 节点属性。Action 归属在导出 Markdown 中只是人类可读的分隔线，不是可逆序列化协议。selection、Undo、当前 Action、cleanup 任务和 Active Ramble 属于编辑会话状态，不在恢复承诺内。旧数据库中只有 Markdown 的草稿作为普通 Markdown 打开，首次由新版工作台保存时升级为结构化文档，不猜测曾经存在的自定义节点属性。
