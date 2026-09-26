# 文稿审阅工作台 v1

状态：第一方 `document_review` v1 合同与设计说明。它复用[反馈协议](../PROTOCOL.md)的请求、草稿、提交和交付，不建立文档编辑器或独立审阅会话。总体决策见 [ADR 008](../adr/008-typed-human-feedback-workbenches.md)。

## 使用场景与界面

Agent 提供视频脚本、发言稿或其他分段文稿，人类在独立工作台区域阅读原文，通过段落标记、局部批注和建议改写表达判断。该区域包含原稿与自己的批注面板；右侧通用反馈列继续承载整体意见、语音、截图、附件和整份请求的提交。

原文始终只读。新增批注、修改建议或切换标记只改变 Interaction State，不能直接改写请求里的原稿。人类最终选择“原稿可用”或“需要修改”，再经统一提交形成结果；选择“原稿可用”不调用最终总结的直接批准入口。

## 请求输入

```json
{
  "type": "document_review",
  "version": 1,
  "data": {
    "title": "发布会开场发言",
    "source_version": "draft-3",
    "paragraphs": [
      {"id": "opening", "label": "开场", "text": "大家好，今天介绍我们的新产品。"},
      {"id": "benefit", "label": "价值", "text": "它能帮助团队更快整理客户反馈。"}
    ]
  }
}
```

| 字段 | 含义与限制 |
| --- | --- |
| `title` | 非空原稿标题，最多 200 个 Unicode 标量值。 |
| `source_version` | 非空来源版本标签，最多 128 个 Unicode 标量值；不等于工作台协议版本或草稿 revision。 |
| `paragraphs` | 1–200 个有序段落。每个段落有稳定且唯一的 `id`、可选 `label` 与原文 `text`。 |
| `paragraphs[].id` | 匹配 `^[a-z0-9][a-z0-9_-]{0,63}$`；不能以展示标题代替身份。 |
| `paragraphs[].label` | 可选段落标题，最多 128 个 Unicode 标量值。 |
| `paragraphs[].text` | 非空原文，每段最多 8000 个 Unicode 标量值，全部段落正文合计最多 120000。 |

原稿、段落顺序和 `source_version` 都是不可变请求输入，参与幂等比较。原稿变化时 MUST 创建新请求；不能把旧批注锚点悄悄搬到新内容上。标题、标签的展示不产生额外的段落身份。

字段长度和数量上限不替代整份消息的预算。请求还受实际 transport 的总 body 上限约束；草稿 `document_json` 继续使用既有 1,000,000 字符总预算。批注引用、意见、建议及自由正文共同消耗草稿预算，不能因为每项单独合法就越过整份保存限制。

## 草稿状态

```json
{
  "type": "document_review",
  "verdict": "changes_requested",
  "annotations": [
    {
      "id": "note-opening",
      "paragraph_id": "opening",
      "start": null,
      "end": null,
      "quote": null,
      "kind": "suggestion",
      "body": "先讲听众会得到什么，再介绍产品。",
      "replacement": "大家好，今天我们一起看看怎样更快整理客户反馈。",
      "status": "open"
    }
  ],
  "paragraph_marks": [{"paragraph_id": "opening", "decision": "revise"}]
}
```

| 字段 | 合同 |
| --- | --- |
| `verdict` | `null` 表示尚未作出整稿判断；`ready` 表示可以使用，`changes_requested` 表示需要修改。保存允许未完成，提交要求显式非空判断。 |
| `annotations` | 最多 500 条批注。`id` 是稳定且唯一的批注身份，使用与段落 id 相同的格式；`paragraph_id` 必须引用原请求中的段落。数量上限不保证 500 条均可填满每项最大长度。 |
| `kind: "comment"` | 普通批注；`body` 是意见，`replacement` 必须为 `null`。 |
| `kind: "suggestion"` | 建议改写；`body` 是理由，`replacement` 必须为字符串，可以是空串以建议删除。建议最多 8000 个 Unicode 标量值。 |
| `body` | 非空批注意见或建议理由，最多 4000 个 Unicode 标量值；编辑中可保留未写完的草稿，提交前须补全或移除。 |
| `status` | `open` 或 `resolved`，表达意见是否仍待处理；不表示 Agent 已改稿或已接受建议。 |
| `paragraph_marks` | 对指定段落的 `keep` / `revise` / `remove` 判断；同一段落只能有一个标记。未标记不等于默认保留，不要求每段都有标记。 |

正文 `doc` 与此状态一起写入版本化 `document_json` envelope，共享 revision/CAS、保存队列、恢复与提交冻结。批注不写成 TipTap 节点，不靠 Markdown 恢复。Cooking 不处理原稿、批注、建议文字或段落判断。

## 文字锚点

整段批注的 `start`、`end` 和 `quote` 同时为 `null`。局部批注使用单段内、按 Unicode 标量值计数的非空半开区间 `[start,end)`，并保存同一原文片段 `quote`，满足 `0 ≤ start < end ≤ 段落长度`。不能混用 UTF-16 code unit、字节偏移或展示字符宽度。客户端选择文本后生成锚点，服务器按不可变段落正文重新验证范围与引用；引用不匹配或超出段落时拒绝，不能静默改成整段批注。

例如原文 `A😀中B` 的 `[1,3)` 对应 `😀中`。emoji 在本合同中占一个 Unicode 标量值；组合字符可能包含多个标量值，因此索引规则不能等同于人眼看到的字形数量。

单条批注不能跨段。跨段选区应提示改为逐段批注或整体说明。原稿只读使同一请求内的锚点稳定，不需要编辑器位置映射或协同转换。

## 提交结果与 Agent 消费

成功提交的 `manifest.workbench.result` 为：

```json
{
  "source_version": "draft-3",
  "verdict": "changes_requested",
  "annotations": [],
  "paragraph_marks": [{"paragraph_id": "opening", "decision": "revise"}]
}
```

结果引用原请求来源版本，包含校验后的全部批注和段落标记；批注字段与草稿一致。`ready` 可以没有批注或标记，但必须由人类显式选择；不允许依靠空白草稿自动推断通过。`changes_requested` 也必须显式选择，具体修改方向可在批注、段落标记或通用正文中表达。

Agent 应先核对 `source_version` 和原请求材料，再读取整稿判断、段落标记、`open` 批注及补充正文。`resolved` 批注仍随结果保留，以免丢失审阅记录；它们不证明原稿已经修改。`replacement: ""` 是删除建议，不是空值遗漏；`remove` 也是审阅判断，RambleDesk 不据此自动删除原稿。

请求取消时 `workbench.result` 为 `null`，不发布一个伪造的文稿判断。`ready`、`keep`、`resolved` 都没有执行权限授权含义。Agent 对原稿的修改发生在反馈提交后的正常任务流程中，需要再次审阅时以新稿创建新请求。

## 参考与采用范围

Word 的现代批注把文本位置和评论面板关联，并区分活跃与已解决意见；本工作台采用原文定位、批注列表与可解决状态。它不复制 Word 的多人评论和通知功能。[Microsoft：Using Modern comments in Word](https://support.microsoft.com/en-us/word/using-modern-comments-in-word)

Google Docs 将建议改写与原文区分，是否应用由后续审阅决定；其评论可以从文本选择定位。本工作台采用“意见及建议先独立保存、原文保持不变”的语义，由 Agent 在取得提交结果后处理。[Google：Suggest edits](https://support.google.com/docs/answer/6033474)、[Google：Use comments](https://support.google.com/docs/answer/65129)

首版不支持直接改原文、跨段单条批注、多人评论线程、跨版本锚点迁移、视频播放与时间轴、导入 Word/Google Docs 协作历史或自动应用替换。这里的“视频脚本”是文稿使用场景，不表示视频编辑能力。

## 验收重点

原稿和来源版本不可变；中文、emoji 和整段锚点可验证；无效引用不能发布；批注与段落标记刷新可恢复；正文变换不丢审阅状态；未选整稿判断拒绝提交；无补充正文也可提交有效判断；已解决意见与删除建议保留；取消没有审阅结果。浏览器预览、真实持久化和 Agent 取得结果分别验收。
