# 三种工作台实验

分支：`codex/workbench-three-types`。第一方静态实验，接口尚未冻结。本轮替换上一轮实验中的关键词搜索和“答案写进正文”模型；试用应创建新请求。

## Agent 从哪里开始

1. `list_workbenches({})`：直接浏览能力目录，不需要猜关键词。每张卡给出用途 `purpose`、预期结果 `returns` 和交互契约 `interaction`；当前默认一次返回三个工作台。可用 `offset` / `limit` 分页，默认 5、最多 20。
2. `describe_workbench({type, version?})`：仅取选中工作台的数据 schema、结果 schema 和示例。
3. `request_feedback({...公共字段, workbench: {type, version, data}})`：创建持久请求，类型请求省略顶层 `actions`。
4. 沿用宿主等待/续接规则：Generic MCP 按其宿主确认流程等待；托管 ACP 创建后结束轮次，等待 RambleDesk 续接。
5. `get_feedback({request_id})`：通用状态与反馈包；结构化答案在 `feedback_package.manifest.workbench.result`，补充正文和附件仍在原有位置。

已知契约可以直接创建请求，无需每次重复发现。能力数量少时平铺简介；未来按公开的能力分类展开，搜索仅作为辅助。当前 `interaction` 是目录元数据，尚未实现同一契约的多个可替换渲染器。

```sh
"$RAMBLEDESK_COMMAND" feedback list-workbenches
"$RAMBLEDESK_COMMAND" feedback describe-workbench --type questions
"$RAMBLEDESK_COMMAND" feedback request --input /absolute/path/request.json
"$RAMBLEDESK_COMMAND" feedback get --request-id <request_id>
```

MCP 工具说明也直接介绍三个能力，避免 Agent 不知道从哪里开始。Generic JSON 提供认证后的 `POST /api/workbenches/list` 与 `/api/workbenches/describe`；托管命令走现有私有 relay 与会话认证。目录查询不会创建请求或确认交付。

## 三种交互

| 工作台 | 输入 | 用户操作 | 提交结果 |
| --- | --- | --- | --- |
| `ramble` | `actions: [{id,instruction}]` | 自由反馈、语音、附件 | `kind: "free_feedback"`，正文与附件 |
| `questions` | `questions: [{id,label?,prompt,options,allowOther?}]` | 每次一题，选择后继续；支持自定义答案、返回修改、汇总复核 | `answers: [{id,value,label,wasCustom,index?}], cancelled: false` |
| `single_choice` | `prompt, options: [{id,label}]` | 选择一个方案，可清除或改选 | `status: "answered", selected_option_id` |

问答采用 [Pi questionnaire 示例](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/examples/extensions/questionnaire.ts) 的业务字段与流程：

- 选项是 `{value,label,description?}`，返回稳定的 `value`；普通选项的 `index` 从 1 开始。
- `allowOther` 默认 true；自定义回答 `wasCustom: true`，没有选项 index。
- 选完进入下一题，支持题目导航和提交前汇总；每题都需作答。单题也保留最后的显式提交，由 RambleDesk 的统一提交生命周期负责。
- 本实验限定 1–20 题、每题 2–6 个选项；自定义答案最多 4000 字符。短标题可省略。
- Codex 的 [request_user_input 模型](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/request_user_input.rs) 也有稳定题目 ID、选项说明和自定义回答，但字段命名与返回格式不同。本实验明确采用 Pi 的模型，不声称两者协议完全相同。

问答和单选的补充正文可以为空。未完成的结构化答案不能靠填写正文绕过；前端与后端都会阻止提交。取消请求不会发布作答结果（`result: null`）。服务端按照原始请求中的 value 重新确定选项 label/index，不相信客户端提交的展示文字。

提交分两层校验：通用层要求“工作台有效输入、反馈正文”至少有一项非空；工作台层再验证自身的完整性。空白自定义答案、未选方案和 Ramble 的类型标记不算用户输入。当前问答仍要求每题作答、单选要求选定一个方案；因此有正文也不能替代这些必答项。

MCP 的文本返回包含 manifest 路径、结构化结果预览及 `workbench.result` 读取指引。长答案预览会明确截断，完整内容保留在 manifest 和 `structured_content.feedback_package` 中；正文为空时明确说明没有补充说明。

请求示例：

```json
{
  "title": "确定第一版方向",
  "what_happened": "请选择首批用户，或填写自己的答案。",
  "workbench": {
    "type": "questions",
    "version": 1,
    "data": {
      "questions": [{
        "id": "audience",
        "label": "用户",
        "prompt": "最先服务哪类用户？",
        "options": [
          {"value": "individuals", "label": "独立开发者", "description": "先做好单人工作流。"},
          {"value": "teams", "label": "小团队", "description": "优先考虑协作评审。"}
        ],
        "allowOther": true
      }]
    }
  }
}
```

外部 Generic MCP/JSON 另外提供 `host_session_id`。现有不传 `workbench` 的旧请求保持 Ramble 行为。

## 工作台共同规则

共同约束是**交互结果可保存、恢复、验证、提交，并且始终属于确定的请求**。不要求所有操作向右侧正文插入文字。

- 工作台拥有自己的结构化交互状态；选择、表单、排序或标注可以直接操作相应实体。
- 富文本编辑器负责自由表达和补充材料；Ramble 的语音/附件仍走现有编辑器路径。
- 共享层负责 revision/CAS、保存队列、提交冻结、发布与 Agent 续接。
- 结构化结果是权威答案；Markdown 摘要可以从结果派生，但不能反向猜测选择。
- 文本编辑器的撤销只影响正文，不会悄悄撤销工作台选择。工作台通过改选、清除和回到题目修改答案。
- Cooking 仅处理正文；答案不交给模型改写。没有正文的结构化提交跳过 Cooking。

把每次操作都写成正文，对自由反馈和简单批注可行；遇到拖拽排序、表格编辑、区域标注时，会产生重复表示、噪声及撤销歧义。因此不把这种具体 UI 策略提升为共同规则。

## 持久化与边界

`document_json` 仍作为请求草稿的原子保存载体，其中两个字段各司其职：

```json
{
  "schemaVersion": 2,
  "doc": {"type":"doc","content":[]},
  "workbenchState": {
    "type":"single_choice",
    "selected_option_id":"compact"
  }
}
```

`workbenchState` 位于富文本文档外，不是 TipTap 节点，不渲染到正文；`body_markdown` 只投影 `doc`。两部分一起走同一 revision/CAS，避免提交得到不同版本的正文与答案。当前编辑、后台采集、重载与协调保存均保留该字段。

迁移 21 仍只增加可空的请求 `workbench_json`，本轮不增加新的数据库列。没有 `workbench` 的旧请求及幂等哈希保持不变；类型、版本、完整输入参与新请求的不可变哈希。旧程序不理解这个实验的答案状态，不能用它编辑实验请求。当前不对上一轮未发布实验的问答数据做迁移。

外部 Pi/dsh 原生工具的参数 schema 仍是原来的 Ramble 入口；类型工作台通过 Generic MCP、Generic JSON 或托管命令调用。引用 Pi 的问答模型不等于已替换 Pi 的原生工具。

## 试用

```sh
pnpm -C apps/desktop exec vite --host 127.0.0.1 --port 5178
```

打开 `http://127.0.0.1:5178/workbenches.html` 切换三种工作台。预览复用实际容器、编辑器和交互组件，但只使用页面内存；提交展示独立答案与补充正文。真实持久化、CAS 和发布由后端集成测试验证。
