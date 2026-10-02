# 差异评审工作台

`diff_review` v1 聚焦改动本身：浏览固定 diff、按文件导航，对修改前后的具体行、行范围或整个改动块批注，再提交整体意见。代码、配置和文档的文本改动都可使用。评审阶段、参与者、审批条件和后续修改由 Agent 或外部流程编排。

## 固定输入

Agent 先读取 `describe_workbench` 的 schema，再提供 `title`、`source_version` 及 `files`。每个文件包含稳定 `id`、`old_path`、`new_path` 和统一格式的 `diff`。路径用于显示和结果定位，不用来读取当前磁盘文件；内容在请求创建时保存且不能在同一请求内替换。

支持普通统一文本 diff，改动块使用 `@@ -old,count +new,count @@` 行和上下文／增删行。校验改动块计数、行号和实际内容；二进制及 combined diff 不作为可批注的文本改动。新增／删除文件可以用 `/dev/null` 表示对应路径。

```json
{
  "type": "diff_review", "version": 1,
  "data": {
    "title": "评审超时设置", "source_version": "config-v2",
    "files": [{ "id": "config", "old_path": "config.json", "new_path": "config.json",
      "diff": "@@ -1 +1 @@\n-timeout=30000\n+timeout=0\n" }]
  }
}
```

最多 100 份文件，每份 diff 120000 个 Unicode 标量、合计 500000 个；具体约束及支持格式以 schema 与运行时校验为准。

## 批注锚点

每条批注保存稳定 `id`、`anchor` 和 `body`。锚点包含：

- `file_id`：原请求内的文件 ID。
- `hunk_index`：该文件内零基改动块编号。
- `side`：`old` 或 `new`，对应修改前或修改后。
- `start_line/end_line`：源文件实际行号的闭区间；整块批注时两者均为 null。

行范围必须属于同一文件、同一个改动块和同一侧，且包含真实存在的行，不能使用渲染索引、跨块范围或将新增行定位到旧侧。“No newline” 标记不计作源文件行。

最多 500 条批注。空意见可以暂存草稿，提交时已有批注必须填写意见，单条最多 4000 个 Unicode 标量。批注使用共享输入字段，整体说明使用右侧正文；语音和附件保留原请求及批注身份。

## 提交与恢复

文件导航、行／范围／块定位和批注回看都指向保存的 diff。较大 diff 分段展示；选择批注会显示其对应位置。全屏页签继续同一草稿，终态只读显示相同快照和意见。

结果为 `source_version` 与 `comments`，位置和意见直接来自本轮保存及验证的状态。允许只提交整体说明并返回空批注列表；也允许仅提交非空批注、不写补充正文。没有任何人类输入时不能提交，不强制选择“通过／拒绝”或评分。

体验清单见[差异评审](../../playground/workbenches/materials/diff-review.md)。
