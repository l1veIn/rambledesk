# 交互方案与相关研究

资料核对日期：**2026-10-09**。

[English](INTERACTION_LANDSCAPE.md) · [简体中文](INTERACTION_LANDSCAPE.zh-CN.md) · [返回 README](../README.zh-CN.md)

与 Agent 协作的界面，可以从两个角度理解：人怎样参与任务，以及开发者怎样构建这种交互。下面按主要关注点分类，帮助理解 RambleDesk 的位置。

## 二维图的口径

README 使用 Mermaid `quadrantChart` 展示五个示意点。坐标仅用于排版，不表示测量值、固定成本顺序或比例：

- **X 轴：UI 化程度**，从文字对话，经可视化展示，到直接操作材料或控件。
- **Y 轴：额外 Token 消耗**，从低到高，表示同一任务为提供交互界面额外引入的模型输入与输出。

五个点对应以下实现路线与设计目标：

- **经典 Chat：** 不要求模型生成 UI，以此作为额外 UI 开销的零基线；对话本身仍消耗 Token。
- **`askUserQuestion`：** 界面由应用预先实现，Agent 提交问题和选项等参数。
- **RambleDesk 预制工作台：** Agent 提交工作台参数；右下方表达 RambleDesk 的设计目标：支持直接操作，同时减少临时生成界面描述的需要。
- **A2UI 动态界面：** 模型生成组件与数据的 JSON，由客户端渲染；开销随描述规模和更新次数变化。
- **动态生成完整 HTML/JS：** 模型为本次交互生成界面代码，示意为右上方路线；预先编写并复用的 HTML 界面不属于这种情况。

动态 A2UI 与 HTML/JS 的相对成本可以随实现变化，图中点位不代表两者的固定高低关系。

工具 schema、输入协议说明、生成的参数及 UI 描述都可能产生额外开销，预制界面也不等于零开销。这里排除任务本身的上下文、推理、代码生成和回答；**图是设计示意，不是 benchmark，也不代表端到端 Token 总量或实际价格**。

不能按协议名称固定高度：[AG-UI](https://docs.ag-ui.com/introduction) 传递事件与状态，不强制生成 HTML；[MCP Apps](https://github.com/modelcontextprotocol/ext-apps) 可以加载预制界面。具体位置取决于实际实现和传入模型的内容。

## 各自解决什么问题

| 方案 | 主要关注点 | 解决的问题 | 与工作台的关系 |
| --- | --- | --- | --- |
| 经典 Chat | 交互方式 | 通过往返消息组织对话；消息也能包含图片、文件和自定义元素。参见 Chainlit 的 [Message](https://docs.chainlit.io/concepts/message) 与 [Element](https://docs.chainlit.io/concepts/element)。 | 对话可以承载或配合工作台；聊天并不等于只能输入纯文本。 |
| [RambleDesk](../README.zh-CN.md) | 桌面应用与交互方式 | 为一次人工参与准备材料、操作方式和返回结果，支持问答、审阅、标注与实际体验。 | 关注人需要完成的具体工作，以及怎样把答案、批注和附件带回 Agent 任务。 |
| [A2UI](https://a2ui.org/introduction/what-is-a2ui/) | 声明式 UI 描述协议 | Agent 发送描述组件和数据的 JSON，客户端根据自己的组件目录渲染界面。 | 解决如何描述界面；具体材料、审阅动作和结果结构仍由应用设计。 |
| [AG-UI](https://docs.ag-ui.com/introduction) | Agent 与前端的连接协议 | 通过事件连接 Agent 运行与用户界面，传递状态和用户交互。 | 解决交互过程如何连接；不限定应用必须采用聊天或某种工作台。 |
| [MCP Apps](https://github.com/modelcontextprotocol/ext-apps) | 工具界面嵌入 | MCP 工具关联 HTML 界面资源，由支持的宿主加载到沙箱 iframe，并经宿主交换数据。 | 为工具提供把表单、图表或其他交互界面带入宿主的方式。 |
| [CopilotKit](https://github.com/CopilotKit/CopilotKit) | 应用开发 SDK | 提供生成式 UI、共享状态和等待用户输入、确认或修改的流程。 | 帮助开发者构建包含人工参与的应用；具体工作台仍需产品设计。 |
| [assistant-ui](https://www.assistant-ui.com/docs/) | 前端组件与 runtime | 用组件构建 AI 界面，通过 runtime 连接后端，也支持[生成式 UI](https://www.assistant-ui.com/docs/tools/generative-ui)。 | 提供实现交互界面的构件，应用可以组合对话与专门的操作界面。 |
| [Chainlit](https://docs.chainlit.io/get-started/overview) | Python AI 应用框架 | 组织消息、步骤和界面元素；[Ask User](https://docs.chainlit.io/advanced-features/ask-user) 可收集文本、文件、动作选择和表单输入。 | 支持在应用流程中等待人的回应，并用更适合任务的输入方式收集结果。 |

## 这些关注点可以组合

聊天、问答与材料审阅可以出现在同一个应用里。类似 `askUserQuestion` 的交互围绕问题收集答案；工作台把这个思路扩展到文稿、图片、网页、终端或其他需要人判断和操作的材料。

界面描述和交互连接也可以配合使用。例如 A2UI 不绑定单一传输方式，其[官方传输文档](https://a2ui.org/concepts/transports/)列出了通过 AG-UI 传递消息的方案。SDK、组件库和应用框架则帮助开发者把这些能力组织成产品。

RambleDesk 的设计问题是：**这次需要人做什么、应提供什么材料、反馈如何对应到材料，以及怎样把结果交回任务。** 上表说明这些问题与其他项目关注点的关系，不表示 RambleDesk 已采用所列协议或框架。RambleDesk 提供 MCP 接口也不等于实现了 MCP Apps；实际接入方式见[兼容与接入说明](COMPATIBILITY.md)。

## 相关研究

以下使用正式发表年份，链接指向论文或作者、机构提供的原始资料。设计联系是我们据此提出的理解，不是论文对 RambleDesk 的评价。

### Principles of Mixed-Initiative User Interfaces

**Eric Horvitz，1999，CHI。** [论文与机构页面](https://www.microsoft.com/en-us/research/publication/principles-mixed-initiative-user-interfaces/)

讨论如何把自动化与人的直接操作结合，包括何时打断、如何澄清不确定目标，以及如何让人启动、终止和修正服务。对工作台的启发是：Agent 自主推进时，仍应保留清楚、低成本的参与和纠正入口。

### Guidelines for Human-AI Interaction

**Saleema Amershi et al.，2019，CHI。** [论文](https://www.microsoft.com/en-us/research/wp-content/uploads/2019/01/Guidelines-for-Human-AI-Interaction-camera-ready.pdf)

提出并多轮评估 18 条指南，涉及能力说明、相关上下文、便于纠正和细粒度反馈。它启发我们让用户知道正在审阅什么、意见对应哪里，以及提交后会发生什么。

### AI Chains: Transparent and Controllable Human-AI Interaction by Chaining Large Language Model Prompts

**Tongshuang Wu、Michael Terry、Carrie J. Cai，2022，CHI。** [论文](https://arxiv.org/pdf/2110.01691)

把 LLM 任务拆成可连接的步骤，让人查看和修改中间结果，并在 20 人研究中评估这种提示链交互。对工作台的启发是围绕具体的中间成果接受反馈。

### Grounded Copilot: How Programmers Interact with Code-Generating Models

**Shraddha Barke、Michael B. James、Nadia Polikarpova，2023，OOPSLA1。** [论文](https://shraddhabarke.github.io/raw/copilot.pdf)

观察 20 位程序员使用早期 Copilot，区分已知方向时的加速与不确定时的探索，并记录阅读代码、执行、静态分析和查阅文档等验证行为。这提示我们为比较、理解和修正提供空间。

### Validating AI-Generated Code with Live Programming

**Kasra Ferdowsi et al.，2024，CHI。** [论文](https://www.cs.cornell.edu/~lerner/papers/leap_chi24.pdf)

将代码生成与实时运行值展示结合，在特定实时编程环境中进行 17 人实验，研究如何支持验证。它启发我们提供可检查的结果与执行依据。

这些研究分别涉及通用设计原则、特定工具和有限实验场景，为本项目提供设计参考。RambleDesk 的效率、准确性，以及在现代长任务 Agent 中的使用效果，仍需单独评估。
