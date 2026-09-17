# 注册项与工作台数据（讨论稿）

状态：**讨论稿，未实现**。本文回答：注册项长什么样、今天默认工作台的输入实际是什么、
多个工作台的数据怎么分、以及这些选择会牵动什么。

## 1. 今天没有 `workbench` 字段：默认工作台的 data 就是请求本身

`RequestFeedbackInput`（`crates/rambledesk-core/src/feedback/model.rs`）现在只有一套形状：

| 字段 | 必需 / 默认 | 属于 | 约束 |
| --- | --- | --- | --- |
| `request_id` | 可选，服务端生成 | 框架 | UUID；创建幂等 key |
| `host_id` / `host_session_id` | 可选 / 必需 | 框架 | 会话关联，不是认证凭据 |
| `title` | 可选 | 框架（展示） | — |
| `what_happened` | 必需 | **默认工作台** | 1–200 字符（`MAX_WHAT_HAPPENED_CHARS`） |
| `actions` | 必需 | **默认工作台** | 1–20 项，每项 `{id, instruction}`；`id` 匹配 `^[a-z0-9][a-z0-9_-]{0,63}$` |
| `context_refs` | 可选 | 框架（可读线索） | `{label, uri}` |
| `attachments` | 可选 | 框架（材料） | `markdown` / `contents_base64` / `path` 三者取一 |
| `source_hint` | 可选 | 框架 | 不是身份字段 |
| `allow_finish` + `final_summary` | 可选 | 框架（终态捷径） | `final_summary` 只在 `allow_finish` 时给出 |

传入路径：MCP 工具参数 / JSON API body（托管会话用 `feedback request --input <json>`）
→ `RequestFeedbackInput` → 校验 → 落在请求上 → 工作台视图读 `request.what_happened` / `request.actions`
渲染简报（`TaskBriefPanel`），自由正文走 Feedback Draft。

**结论：默认工作台的"数据"是 `what_happened` + `actions`，只是今天没有独立的 data 容器。**

## 2. 注册项草案

```jsonc
{
  "type": "ramble",           // 稳定类型标识（`id` 已被 request/host/session 占用）
  "version": 1,               // 工作台类型版本
  "dataVersion": 1,           // 专用数据版本（可与类型版本分开演进）
  "default": true,            // 缺省工作台：请求未指定类型时走它
  "name": "Ramble",
  "purpose": "默认工作台：一段可扫读的现状摘要 + 一组可体验动作，收集自由反馈与附件。",
  "ownedFields": ["actions"], // 该类型占用的顶层请求字段（兼容现状的关键）
  "inputSchema": {            // 类型专属输入的 JSON Schema；约束写在这里，不另开 configSchema
    "type": "object",
    "required": ["actions"],
    "properties": {
      "actions": {
        "type": "array", "minItems": 1, "maxItems": 20,
        "items": {
          "type": "object", "required": ["id", "instruction"],
          "properties": {
            "id": { "type": "string", "pattern": "^[a-z0-9][a-z0-9_-]{0,63}$" },
            "instruction": { "type": "string" }
          }
        }
      }
    }
  },
  "resultSchema": {           // 阶段 4：这一类型上报的结果长什么样
    "type": "object",
    "properties": { "answers": { "type": "array" } }
  }
}
```

与最初设想的差别：

- `type` + `version`（而不是 `id`）：避免与 `request_id` / `host_id` / `session_id` 混淆，并给类型留版本位。
- 约束直接写进 `inputSchema`：`what_happened` 的 200 字上限属于**框架共享字段**，已经写在生成的 JSON Schema 里
  （`maxLength`），不需要第二份 `configSchema`。一套约束只有一处真源。
- `default: true`：现有请求（不带类型）必须继续进默认工作台。
- `ownedFields`：见第 3 节。
- `resultSchema`：与输入对称，阶段 4 用。

## 3. 共享材料与类型专属字段

| 类别 | 字段 | 归属 |
| --- | --- | --- |
| 身份 | `request_id` / `host_id` / `host_session_id` | 框架 |
| 共享材料 | `title` / `what_happened` / `context_refs` / `attachments` / `source_hint` | 框架（任何工作台都要） |
| 终态捷径 | `allow_finish` / `final_summary` | 框架 |
| 类型专属 | Ramble：`actions`；将来：题目、卡片顺序等 | 工作台类型 |

`ownedFields` 的作用是**不迁移就能表达归属**：Ramble 继续按顶层传 `actions`（幂等哈希不变），
新类型把专属数据放进 `workbench.data` 并接受 `inputSchema` 校验。
等真出现第二个类型，再决定要不要把 Ramble 的 `actions` 也搬进 `data`。
**搬动会改变幂等哈希**（`PROTOCOL.md` 的不可变输入），需要专门的迁移与冲突策略，不急着做。

## 4. 校验与降级是两件事

| 时机 | 谁 | 行为 |
| --- | --- | --- |
| 创建请求 | 服务端（知道注册表） | 类型不存在或 `data` 不匹配 `inputSchema` → `INVALID_ARGUMENT`，不落库 |
| 渲染请求 | 客户端（版本可能更旧或更新） | 类型不认识、或版本高于本客户端 → 保留请求材料 + 自由反馈，明确提示"专用交互未加载" |

创建时拒绝保证"不会有人渲染不了的数据"；渲染时降级保证"旧客户端不会被新数据卡死"。
两者都不能把未解析的数据当成已完成的结果。

## 5. 版本要分三种，不要合成一种

- **工作台类型版本**：交互/组件语义变化。
- **专用数据版本**：`workbench.data` 的结构变化。
- **首次引导版本**：引导文案与启用状态的变化。

合在一起会导致"改个文案就要迁移存储"。三者如何与请求、草稿、结果关联，见
[工作台演进路线](../WORKBENCH_EVOLUTION.md)第 6 节。

## 6. 待定

1. `ownedFields` 折中可接受吗？还是要把 Ramble 的 `actions` 也搬进 `workbench.data`（须设计幂等迁移）？
2. `inputSchema` 用 JSON Schema 的子集（我们只需要 type/required/properties/pattern/min/max/enum），
   还是自定义一个小 DSL？前者能直接复用 `schemars` 的产物，后者更受控但要多写一套校验。
3. 现在就要 `resultSchema` 吗？还是先只写 `inputSchema` + `purpose`，等阶段 4 再补？
