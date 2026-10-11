# 头脑风暴工作台

`brainstorm` v1 用卡片共同探索一个工作问题：人可以补充想法、改写卡片、分组、选择方向或暂放想法，再把下一步意图随看板一起提交给 Agent。每轮仍是一份独立请求；下一轮从已提交的看板继续，人的修改不靠 Agent 重新抄写。

这是当前源码的新类型，不代表既有安装包已包含。首版是同一看板上的自由探索，不是通用工作流引擎；任务也可以在不同轮次使用问答、文档审阅等其他工作台。

## Agent 输入与应用快照

先读取 `describe_workbench({type: "brainstorm", version: 1})`。Agent 的 `data` 只包含 `title`、可选 `inherit_from_request_id`、新增 `groups` 和新增 `cards`。首轮可以从空看板开始。

```json
{
  "type": "brainstorm",
  "version": 1,
  "data": {
    "title": "怎样让新用户完成第一次有用的反馈？",
    "groups": [{ "id": "first-action", "title": "先做一件小事" }],
    "cards": [
      {
        "id": "one-real-problem",
        "title": "先圈出一个真实问题",
        "body": "让用户带一张自己的截图，圈出一个位置，再用一句话解释。",
        "group_id": "first-action",
        "source_card_ids": []
      }
    ]
  }
}
```

应用创建请求时生成只读输入快照 `BrainstormData`：`title`、`source_version`、`board_id`、`round`、`inherit_from_request_id` 与 `initial_board`。`source_version` 是本轮请求 UUID，`board_id` 是首轮请求 UUID，首轮 `round=1`。`initial_board` 包含卡片、分组、选中 ID 和阶段。**这些字段由服务端生成，Agent 不得提交 `initial_board`、轮次或来源身份。** 开发预览使用的已解析快照不是请求模板。

卡片使用稳定 `id`，带 `title`、`body`、可空的 `group_id`、`source_card_ids`、`origin`、`origin_request_id` 和 `parked`。`origin` 为 `agent` 或 `human`，表示最初创建者；人改写 Agent 卡片不会改变其来源。`origin_request_id` 指向卡片首次出现的轮次，不是最后编辑者。

## 看板操作与提交

发散、整理、聚焦分别保存为 `diverge`、`organize`、`focus`，可以自由切换，不是必须按顺序通过的阶段。人可以新增卡片、编辑标题与正文、新增或调整分组、移动卡片及多选方向。请求初始看板中的卡片（包括继承卡）只能暂放：保留原内容并取消选择，之后仍可恢复。本轮新建的人卡可以移除；误加的空卡直接移除，已有文字的卡片需确认。删除分组时，卡片转到未分组位置，不随分组删除。

卡片正文和下一步说明复用共享文字、语音和附件输入；标题与分组名用于简短命名。整体反馈仍在公共正文中。“检查并提交本轮”只打开现有整体反馈抽屉，最终提交仍需通过本轮校验。草稿保存不会触发 Agent，也不会自动选择下一步。

最终提交需要显式选择 `intent`：

| 意图 | 含义 | 选择条件 |
| --- | --- | --- |
| `explore` | 再探索一些方向 | 可以不选卡片 |
| `expand` | 展开选中的方向 | 至少一张未暂放卡片 |
| `combine` | 结合选中的想法提出新方向 | 至少两张未暂放卡片 |
| `proceed` | 按选中方向继续原任务 | 至少一张未暂放卡片 |

可用 `instruction` 补充希望 Agent 怎样处理。它与公共反馈正文分别保存；两者都可以为空。卡片或分组标题可以暂时清空并保存草稿，但提交时每个标题必须有可见文字，包括暂放卡片。卡片正文可以为空。选择方向和 `proceed` 不授予原任务之外的执行或发布权限；这些选项在提交后才交给 Agent，不是实时 AI 按钮。

结构化结果保存在 `feedback_package.manifest.workbench.result`，包含完整 `cards`、`groups`、`selected_card_ids`、`phase`、显式 `intent`、`instruction`，以及本轮来源、看板与继承身份。提交后的请求只读，不能重新打开原请求改写已发布结果。

## 下一轮与保留人的修改

Agent 先读取本轮真实反馈包，再按意图继续。需要另一轮看板时，新建请求并将 `inherit_from_request_id` 设为已完成、已发布的 `brainstorm` v1 请求 ID。例如，上轮实际选中了 `one-real-problem`，可以追加：

```json
{
  "type": "brainstorm",
  "version": 1,
  "data": {
    "title": "把第一次反馈缩成一个小步骤",
    "inherit_from_request_id": "be715c00-83a1-4b8d-8d53-181208b7ca01",
    "groups": [],
    "cards": [
      {
        "id": "one-screenshot-loop",
        "title": "一张截图、一句解释、一次原会话续接",
        "body": "先完成一个真实问题的反馈，再让用户看到 Agent 的下一步。",
        "group_id": null,
        "source_card_ids": ["one-real-problem"]
      }
    ]
  }
}
```

示例 UUID 必须换成真实上轮 ID。只发送新增分组与新 ID 卡片，不重传已有卡片；与已有卡片或分组 ID 冲突会拒绝，不能借继承覆盖人的文字、分组或暂放决定。新卡的 `source_card_ids` 只能引用直接上轮已选中的卡片，每卡最多 20 个且不重复；首轮必须为空。不依赖卡片标题判断身份。

服务端在创建事务中保留上轮最终看板，包括人新增的卡片、改字、分组、顺序、选择、阶段和暂放状态，再追加 Agent 的新提案。下一轮清空 `intent`、`instruction` 和整体反馈，要求人重新给出下一步；上轮意见仍留在历史包中供 Agent 阅读。仅继承卡片正文中匹配父发布包 manifest 用户附件的 `attachment://` 引用：附件复制到子请求自己的记录并重写引用，不复用父请求的可变草稿路径。不自动搬运父请求输入附件、仅上一轮整体反馈或下一步说明中的附件，也不承诺复制任意链接。

相同请求 ID 与相同 Agent 原始输入的重试返回已有请求；不会再次继承或覆盖已经编辑的新草稿。不同输入不能借同 ID 重建看板。待处理、取消、无发布结果、未知类型或版本的请求不能作为继承来源。

托管路径要求父子属于相同可信 `managed_session_id`、host 和会话；外部路径要求父子均为外部请求且 host 与 `host_session_id` 一致。后者是防误接的关联检查，**不是认证机制**；外部入口沿用已有全局 token 权限。托管与外部不能混接。继承不改变等待方式：托管提交后续接原 Agent 会话，外部入口继续遵循原适配器或 MCP 的交接规则。

## 限制与验收

- 完整看板最多 200 张卡片、30 个分组，继承与新增合计计入限制；选中 ID 不重复且不得指向暂放卡片。
- 卡片／分组 ID 匹配 `^[a-z0-9][a-z0-9_-]{0,79}$`。标题最多 200 个 Unicode 标量；卡片正文与 `instruction` 各最多 4000 个，禁止 NUL。实际边界以 schema 与运行时校验为准。
- 人可编辑内容、分组与选择，但不能伪造已有卡片来源和派生关系；人新增卡片使用当前轮次身份，没有 Agent 派生来源。
- 同一看板的自动继承只解释 `brainstorm` v1 结果，不尝试把其他工作台的任意状态转换成卡片。Agent 可以在任务中选用不同类型，后续需要看板时再引用兼容的历史看板。

固定体验材料见 [Playground 第 14 场景](../../playground/workbenches/materials/14-brainstorm.md)。验收需区分单轮 UI、真实保存发布和两轮继承；内存预览不能证明 SQLite、附件复制或 Agent 续接通过。测试还应覆盖无效来源、跨会话拒绝、重复请求不覆盖人稿、暂放与选择保留，以及父请求删除后子请求附件仍可读。
