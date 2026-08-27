# 调研：用 TipTap 做“话题归属（Topic Attribution）”的更优模型

> 日期：2026-08-27　范围：码钉加（RambleDesk desktop）“Action 频道/话题标记”实现方式的调研与探索。
> 结论先行：**数据层继续用节点属性 + 分组装饰是正确且足够简单的**；更值得投入的是“**选中文本 → 直接标记/改标为某话题**”的显式交互，而不是引入自定义块节点。

---

## 1. 需求本质

用户在表达（语音/输入/截图/剪贴板/文件）时，希望声明一段内容**归属于哪个话题**（当前产品形态：任务简报里的“Action 1/2/3…”）。本质是**话题引用**：

- 数据：某一块内容（一个或多个块级节点，含图片、引用、列表）归属话题 N；
- 视觉：归属清晰可辨（整块连续底色 + 块首话题标记）；
- 导出：Markdown 里能看到归属分隔线（`---------------- Action N ----------------`），但编辑器内**不出现**这行文字；
- 交互：点一下序号即进入“打标模式”，之后所有输入自动归属；不选时全部不归属。

当前实现（已落地）：

| 层 | 手段 | 状态 |
| --- | --- | --- |
| 数据 | `actionIndex` 全局节点属性（paragraph/heading/blockquote/image/list/codeBlock/table/HR） | 已实现，无损往返 |
| 视觉 | `ActionChannel` 插件按“连续同频道”分组下发节点装饰类（lead/mid/end/solo），CSS 合并为一个连续底块 | 已实现 |
| 导出 | `serializeDocWithActionChannels` 在频道变化处插入分隔线；回填时 `migrateActionChannelSeparators` 把分隔线恢复为印章（横线形态视为“回默认频道”并丢弃） | 已实现 |
| 交互 | 面板/全屏简报里的序号按钮切换“当前频道”；会话持有频道状态，所有插入路径（语音/截图/剪贴板/文件/粘贴块）插入时向会话实时取频道 | 已实现（单例 store 作单一来源） |

已知的坑（都已排掉）：编辑器实例重建后内部存储失同步 → 改为会话 store；光标停在 0 → 程序性插入一律文档末尾；粘贴带分隔线的导出文本 → 解析时按频道边界处理。

---

## 2. TipTap / ProseMirror 能力盘点

| 能力 | 适合做什么 | 不适合做什么 |
| --- | --- | --- |
| **全局节点属性**（`addGlobalAttributes`，[官方文档](https://tiptap.dev/docs/editor/core-concepts/extensions)） | 给任意块节点加一个 `data-action-index` 属性；零 schema 改动，历史文档无需迁移，常规往返无需自定义序列化 | 不能在属性里组织“一组节点”的视觉容器 |
| **Marks**（[官方文档](https://tiptap.dev/docs/editor/core-concepts/extensions)） | 行内强调、链接 | **不能作用于图片/引用/列表等块级归属**，选它做归属模型语义错误 |
| **自定义块节点**（`Node.create` + `content: 'block+'`，[官方文档](https://tiptap.dev/docs/editor/core-concepts/extensions)） | 真正的 DOM 容器：整块 header/chip、可折叠、拖拽、右键菜单锚点 | 需要配套：parseHTML、加入/离开规则（join/paste/栅格嵌套）、自定义 Markdown 序列化器、历史迁移；与现有 speech cleanup 遍历、粘贴拆分逻辑要逐一适配 |
| **NodeView**（[嵌套 NodeView 指南](https://tiptap.dev/docs/guides/nested-node-view-content)） | 单节点的自定义 DOM（比如给某个块画一个左栏标签） | 不能跨节点“包住一组兄弟块”（组的概念必须来自容器节点或父视图） |
| **装饰**（Decoration widget/inline/node） | 不改 schema 的轻量视觉（分组底色、`@ Action N` 前缀标记） | 装饰不能成为数据；每次状态重建时计算 |
| **块级装饰**（`Decoration.block`） | 理想形态：一个包装层包住一组块 | **我们依赖的 prosemirror-view@1.42.2 没有该 API**（类型定义里只有 widget/inline/node）；升级路径未知，不能作为方案依赖 |
| **@tiptap/markdown 双向**（[MarkdownManager](https://tiptap.dev/docs/editor/markdown/api/markdown-manager)、[自定义解析](https://tiptap.dev/docs/editor/markdown/advanced-usage/custom-parsing)） | 自定义节点→Markdown 文本的序列化 | 序列化规则要自己写；当前“分隔线”方案已跨过这层（属性→文本投影），无需节点级 token |

---

## 3. 候选方案对比

### A. 现状：属性 + 分组装饰（推荐保留）

- 数据粒度：每节点一个 `actionIndex`（可空）。
- 视觉：连续同频道节点由一个装饰组渲染成连续块（已验证可行）。
- 导出：频道变化处插分隔线；回填：分隔线→频道（无横线、无文字残留）。
- 优点：schema 零改动；历史文档自动兼容；cleanup/替换/粘贴/撤销照旧；测试面小。
- 缺点：不能给整块做“唯一的 DOM 包装”（无 header/chip/折叠/拖拽）。

### B. 自定义“话题块”节点（TopicSection）

- 形态：`topicSection(N) { content: block+ }` 包装一组块，NodeView 渲染一个带标签的容器。
- 优点：视觉容器最彻底；可折叠、可重命名、可拖拽排序；块与话题一一对应，心智最直。
- 代价：① Markdown 侧需要一种稳定文本形态（要么保持现有分隔线——那新节点反而多一层；要么新 token，破坏现有导出可读性）；② 列表/表格/引用嵌套的 join/split/paste 规则；③ speech cleanup 与历史替换逻辑要适配容器内的段落定位；④ 文档迁移（现有 `actionIndex` 属性要升级成节点）；⑤ undo/merge 边界测试多。
- 结论：**收益大于成本，但不是现在**——等“话题”真正需要自己的标题/折叠/排序时再上。

### C. 选区显式标记（建议新增的交互）

- 用户选中任意内容 → 点“标记为 Action N”（或右键/浮动条）→ 给选中节点加盖/改盖印章。
- 优点：直接解决“刚刚那段其实是话题 2”、“说错了我改一下”这类**事后改标**；不打断语音流（语音仍用现在的一点即进入模式）；实现零 schema 改动（选中节点集合→重新盖章，走现有 attrs+装饰）。
- 这正对准你说的“有没有更简单更直接的交互”——把“模式开关”和“事后改标”组合起来就是完整的归属能力。

### D. 其他（不推荐）

- 行内 mention/`@@话题`：把归属做成行内标签，破坏了“块归属”语义，且导出/回溯复杂。
- 侧边 topic 栏/大纲：与正文分离的平行对象图，背离“只在编辑器会话里”的架构约定。

---

## 4. 探索记录（本仓可实现性验证）

1. **分组装饰**：本仓库 `ActionChannel` 插件已按“连续同频道”把节点分为 lead/mid/end/solo 并下发装饰类；CSS（RichFeedbackEditor）把类合并为一个连续底色块。这是未升级 prosemirror-view 之前做“整块视觉”的可行路径（已上线）。
2. **包裹容器不可行（经源码确认）**：`node_modules/prosemirror-view@1.42.2` 的 `Decoration` 仅有 widget/inline/node 三类，无 `Decoration.block`；因此“装饰自动包一组兄弟节点”在当前依赖版本上不可行——要么升级 prosemirror-view 并验证，要么走方案 B（自定义容器节点）。
3. **粘回分隔线**：解析 `--------------------------------`（默认频道分隔线）会得到 `horizontalRule` 节点（CommonMark 语义），已在回填迁移中统一按“默认频道边界”处理：丢弃该行、剥离后续残留印章；`---- Action N ----` 含字母、解析为段落，按“开启频道”处理（盖章）。
4. **嵌套容器适配成本样例**：`SpeechBlockMetadata` 的 cleanup 候选遍历是 `doc.descendants`（可进入新容器），但“锁段替换/插入位置”等按段落位置计算的逻辑需要按容器边界重算——这构成方案 B 的主要工作量。

---

## 5. 推荐演进路线

1. **近期（零 schema 改动）**：保留 A（数据+视觉+导出）+ 新增 C（选中→标记/改标）。交互路径变为：
   - 语音/截图流：点序号进入打标模式（现状）；
   - 事后流：选中内容 → 标记/改标为 N（新）；
   - 停止/提交：自动退出打标模式（已完成）。
2. **中期（可选）**：给分组块加“组首标签条”（chip）表现话题名（仍用装饰，不引入容器）。
3. **远期（当话题有生命周期时）**：评估 `topicSection` 容器节点（方案 B），为可折叠/排序/重命名做准备；届时先升级 prosemirror-view 并做块级装饰可行性验证。

## 6. 参考

- https://tiptap.dev/docs/editor/core-concepts/extensions
- https://tiptap.dev/docs/guides/nested-node-view-content
- https://tiptap.dev/docs/editor/markdown/api/markdown-manager
- https://tiptap.dev/docs/editor/markdown/advanced-usage/custom-parsing
- https://tiptap.dev/blog/release-notes/introducing-bidirectional-markdown-support-in-tiptap
- 本仓代码：`actionChannelExtension.ts`（分组装饰）、`workbench/actionChannel.ts`（导出/回填迁移）、`workbench/actionChannelState.ts`（单一频道来源）、`RichFeedbackEditor.svelte`（连续块样式、尾部插入）
