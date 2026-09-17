# Agent 视角：工具面、路由与上下文（讨论稿）

状态：**讨论稿，未实现**。本文只描述现状与可选做法，用来判断多工作台该不该改协议、改多少。

## 1. 今天 Agent 看到什么

| 面 | 内容 |
| --- | --- |
| Generic / native 适配器 | `request_feedback`、`get_feedback`、`cancel_feedback`（`crates/rambledesk-mcp/src/lib.rs`） |
| 托管 ACP 会话 | `request_feedback`、`get_feedback`、`recover_feedback`（`crates/rambledesk-mcp/src/managed.rs`），同一套 payload 也可由 CLI `feedback request/get/recover` 使用 |
| 服务器说明 | 每个 endpoint 一段 `with_instructions(...)`；单个工具描述 1–1.5 千字符（等待方式、不轮询、幂等、`allow_finish` 用法都在描述里） |
| 每请求字段 | 见 [registry-and-data.md](registry-and-data.md) 第 1 节 |
| 读取 | `get_feedback` 返回状态 + 回复文本（markdown 路径、附件路径、预览）+ `structured_content.feedback_package` |

**要点：今天的协议形状就是默认工作台的形状**（`what_happened` = 简报、`actions` = 体验动作、正文 = 自由反馈）。
协议里没有"工作台"这个词，所以"多工作台"等价于"要不要把隐含形状显式化"。

## 2. 路由的三种做法

| | 形态 | 代价 |
| --- | --- | --- |
| **A. 类型是数据**（建议） | 仍是 `request_feedback` / `get_feedback`；请求加**可选** `workbench: {type, version?, data?}`，缺省 = 默认工作台 | 工具面不变；新工作台 = 新类型 + 新 `inputSchema`；旧 Agent 与旧请求完全不受影响 |
| B. 一个工作台一个工具 | `request_ramble_feedback`、`request_question`… | 十几个工作台 = 十几个工具与描述，直接吃掉 Agent 上下文；幂等/等待/取消语义要复制 N 份 |
| C. Agent 直接发界面结构 | 通用 UI schema | 路线已明确暂缓；无法校验与降级，等于把 UI 变成协议 |

## 3. 路由发生在三处，都不是 transport

1. **Agent 选类型**：按用途挑；缺省即默认工作台。
2. **服务端创建时校验**：类型与 `data` 必须匹配注册项（见 [registry-and-data.md](registry-and-data.md) 第 4 节）。
3. **客户端渲染时解析**：按 `type` 选视图；版本不认识就降级为"请求材料 + 自由反馈"。

## 4. Agent 的上下文预算分三层

| 层 | 内容 | 频率 | 是否随工作台数量膨胀 |
| --- | --- | --- | --- |
| 常量层 | 协议规则 + 工作台目录（type / name / 用途 / data 提示 / 结果形态） | 每会话一次 | 目录线性增长，但每项应是两三行；**不是工具描述** |
| 每请求层 | `workbench.type` + 该类型 `data` + 共享材料（`what_happened` ≤200、`attachments`） | 每请求 | 只与选中的那一个类型成正比 |
| 结果层 | Markdown 正文 + 结构化结果 + 附件 | 每请求一次 | 固定 |

目标：**工具数恒定，目录一次，类型数据只出现在用它的那次请求里。**

## 5. 结果形态

- 默认：`feedback.md`（人读）+ `uncooked.md`（原始证据）+ 附件；已有 Agent 只读 markdown 即可。
- 阶段 4：`structured_content` 增加 `workbench: {type, version, result}`（逐项回答）。
  Markdown 仍是可读投影，不是结构真源。两者同时提供，避免"想拿结构就得解析 markdown"。

## 6. 兼容边界

- 不带 `workbench` 的请求 = 默认工作台，行为与今天完全一致。
- 未知类型：**创建时**拒绝；**渲染时**（客户端版本旧）降级。
- 托管 ACP：会话身份由 controller 固定，`type` 只是内容选择，不改变归属与续接语义。
- Generic MCP 的等待语义（宿主确认工具 + `get_feedback`）与工作台类型无关，不因新类型增加工具。

## 7. 待定

1. 路由走 A 吗？（B 的唯一好处是"工具名自解释"，但代价随工作台数量线性增长。）
2. 目录注入方式：先只写进 MCP `instructions`，还是现在就加 `list_workbenches` 查询？
   十几个工作台时，目录还能塞进 instructions 吗？
3. 人类能不能改变 Agent 选定的类型（例如"这次我想要问答题而不是自由反馈"）？这属于产品决策，
   若要支持，是在工作台里切换类型（同一请求换视图）还是新建请求？
