# 生成式 UI 路线与相关研究

地图资料核对日期：**2026-10-11**；交互与头脑风暴参考核对日期：**2026-10-12**。

[English](INTERACTION_LANDSCAPE.md) · [简体中文](INTERACTION_LANDSCAPE.zh-CN.md) · [返回 README](../README.zh-CN.md)

生成式 UI 让模型参与决定展示什么界面、填入什么内容，或怎样组合界面。它既包括选择和填充预置组件，也包括生成结构化组件树或界面代码，并不一定要从头生成 HTML 或 JavaScript。例如，[AI SDK 官方指南](https://ai-sdk.dev/docs/ai-sdk-ui/generative-user-interfaces)就把工具结果交给开发者预先编写的 React 组件呈现。

**预置模板与注册组件是已有路线，并非 RambleDesk 的原创。** RambleDesk 是把这类思路用于工作场景的一次应用层尝试：Agent 选择预先准备的工作台，提供材料，再接收人的答案、批注或建议。它是桌面工作台应用，不是通用 UI 框架。本文不表示 RambleDesk 已集成下面列出的项目；实际接入方式见[兼容与接入说明](COMPATIBILITY.md)。

## 交互参考与相关产品

这里区分本轮交互改进实际借鉴的实践，以及后续对照中找到的相关产品。开发后发现同类项目，不等于它就是 RambleDesk 最初的灵感来源。

| 参考 | 已有交互 | 与本轮工作的关系 |
| --- | --- | --- |
| [Plannotator](https://plannotator.ai/) · [官方仓库](https://github.com/backnotprop/plannotator) | 在方案或 Markdown 中选文、批注和建议替换，审阅代码差异，再把结构化反馈交给 Agent。 | 本轮文档审阅的参考：展示的选区与实际反馈对象一致，意见贴近原文。RambleDesk 的 Markdown 锚点和反馈合同仍由本项目实现。 |
| [Agentation](https://www.agentation.com/) · [交互指南](https://www.agentation.com/features) · [官方仓库](https://github.com/benjitaylor/agentation) | 对网页元素或文字批注，携带定位上下文，通过复制或 MCP 交给 Agent；指南说明了快捷键和取消操作。 | 本轮网页评审退出选择模式、保留草稿和恢复焦点的参考，不表示 RambleDesk 嵌入了 Agentation。 |
| [MCP Feedback Enhanced](https://github.com/Minidoracat/mcp-feedback-enhanced) | MCP 工具打开反馈界面，收集选项、文字或图片，将人的意见交回 Agent。 | 后续对照中发现的直接同类反馈流程，不将它写成项目最初的灵感或依赖。 |
| [Miro 分组](https://help.miro.com/hc/en-us/articles/4409706795410-Clustering) · [FigJam 整理与总结](https://help.figma.com/hc/en-us/articles/18711926790423-Sort-and-summarize-stickies-with-FigJam-AI) | 成熟白板产品已支持选择与组织便签；FigJam 可生成分组副本后由人调整，Miro 可将选中的便签归组。 | 为头脑风暴的可编辑卡片、分组和选择提供已有交互模式，不据此推导 RambleDesk 的跨轮继承或附件保留保证。 |

这里记录交互借鉴与对照。代码复用和第三方依赖另见[第三方声明](../THIRD_PARTY_NOTICES.md)。

## 二维图的口径

README 的“RambleDesk 在生成式UI地图上的位置”直接标注项目和交互方式的选定用法：经典 Chat、askUserQuestion、Chainlit、AI SDK、assistant-ui、CopilotKit、RambleDesk、MCP Apps、Tambo、A2UI、json-render、OpenUI、OpenIntelligentUI。AG-UI 在图标题中标为连接层，不单独设点，也不表示所有项目都使用它。

- **X 轴：UI 化程度**，从文字对话，经可视化展示，到直接操作材料或控件。
- **Y 轴：额外 Token 消耗**，表示同一任务为提供交互界面额外引入的模型输入与输出。

**坐标是选定用法的示意，不是实测成本或项目的固定排名。** 同一个项目可以支持多种路线。复用界面可以提供丰富交互，而不必每次生成界面代码，但 schema、参数、材料和返回反馈仍有成本。图中排除任务本身通常需要的上下文、推理、代码生成和回答，不代表端到端 Token benchmark 或价格比较。

## 社区已有地图

- [CopilotKit 的 Generative UI Spectrum](https://www.copilotkit.ai/generative-ui-spectrum)按开发者控制程度与 Agent 自由度，划分 Controlled、Declarative、Open-ended 三类。
- [OpenUI 的 State of Generative UI in 2026](https://www.openui.com/blog/state-of-generative-ui-report)由 Thesys Engineering Team 撰写，分别讨论界面出现在哪里，以及 Agent 输出什么。
- [AG-UI.AI 的生态地图](https://ag-ui.ai/en/map)按交互、声明式渲染、嵌入应用、实现与集成四层分类。这是独立社区资源，与所列项目的维护者无隶属关系，不是 AG-UI 官方地图。

这些已有分类采用不同维度，都不是本图的“UI 化程度 × 额外 Token”坐标。

## 已有的组件与应用路线

下面列举开发者可使用的不同构件。同一个框架可以支持图中的多种方式，不对应固定坐标或固定 Token 成本。

| 项目 | 模型提供什么、开发者预置什么 | 交互与反馈 |
| --- | --- | --- |
| [AI SDK](https://ai-sdk.dev/docs/ai-sdk-ui/generative-user-interfaces) | 模型选择工具并提供参数，应用执行工具，再把结果映射到预先编写的 React 组件。 | [客户端工具](https://ai-sdk.dev/docs/ai-sdk-ui/chatbot-tool-usage)可以收集用户输入，返回工具结果并继续对话。 |
| [Tambo](https://docs.tambo.co/concepts/generative-interfaces) | 开发者注册 React 组件、用途描述与 props schema，Agent 选择组件并流式填充属性，也能更新页面上已放置的组件。 | 直接操作与自然语言可以作用于同一界面；需显式暴露[组件状态](https://docs.tambo.co/concepts/generative-interfaces/component-state)，模型才能看到用户编辑，普通 React state 默认不可见。 |
| [json-render](https://json-render.dev/docs/specs) | 模型生成 JSON UI spec；[catalog](https://json-render.dev/docs/catalog)定义可用组件与动作，[registry](https://json-render.dev/docs/registry)提供实际实现。 | 状态绑定和动作处理器支持表单等交互，应用决定执行什么，以及向模型回传什么。 |
| [OpenUI](https://github.com/thesysdev/openui) | 模型使用指定组件库输出 OpenUI Lang，渲染器逐步用这些组件构建界面。 | [动作回调与表单状态](https://www.openui.com/docs/openui-lang/interactivity)携带用户意图和字段值，也支持继续对话的动作。 |
| [CopilotKit](https://github.com/CopilotKit/CopilotKit) | 应用开发 SDK，支持生成式 UI、组件注册与共享状态，涵盖多种实现方式，而非单一界面格式。 | 支持用户交互，以及等待输入、确认或修改的流程。 |
| [assistant-ui](https://www.assistant-ui.com/docs/tools/generative-ui) | 前端组件与 runtime 将 AI 后端连接到界面，也支持自定义工具调用的呈现组件。 | 开发者可以在应用中组合对话、工具界面与用户输入。 |
| [Chainlit](https://docs.chainlit.io/get-started/overview) | Python 应用框架，组织消息、步骤和界面元素。 | [Ask User](https://docs.chainlit.io/advanced-features/ask-user)可在流程等待回应时收集文本、文件、动作选择和表单输入。 |
| [OpenIntelligentUI](https://github.com/CopilotKit/OpenIntelligentUI) | 原 OpenGenerativeUI 项目；支持原生组件与自定义交互界面，图中选取生成 HTML/CSS/JS 的用法。 | 本地控件在沙箱中运行，用户后续操作可以通过宿主桥接启动 Agent 的下一轮。 |

Thesys 当前将托管服务 **OpenUI Cloud** 标为原 **C1 API**。托管服务与 OpenUI 框架有关联，不宜当作两条彼此无关的实现路线。参见 [Thesys 官方介绍](https://www.thesys.dev/)。

## 协议与宿主是不同层次

| 项目 | 所在层次与职责 | 不由它决定的事情 |
| --- | --- | --- |
| [A2UI](https://a2ui.org/introduction/what-is-a2ui/) | 声明式 UI 描述协议：Agent 用 JSON 描述组件和数据，客户端通过组件目录渲染。 | 不包办应用的审阅流程，也不要求任意生成 HTML/JS。 |
| [AG-UI](https://docs.ag-ui.com/introduction) | Agent 与前端的连接协议，在 Agent 执行过程中传递事件、状态和交互。 | 不强制生成 HTML，也不限定应用采用聊天、组件树或某种工作台。 |
| [MCP Apps](https://github.com/modelcontextprotocol/ext-apps) | 工具提供交互式 HTML 资源，由支持的宿主加载到沙箱 iframe，并经宿主交换数据与动作。 | HTML 可以预先编写；协议本身不意味着每次生成代码或某种固定 Token 成本。 |

这些层次可以组合。例如 A2UI 不绑定单一传输方式，其[传输文档](https://a2ui.org/concepts/transports/)介绍了通过 AG-UI 传递消息的方式；组件渲染器也可以运行在嵌入应用内。提供 MCP 工具接口，不等于实现了 MCP Apps。

聊天、问答与材料审阅也可以出现在同一个应用里。聊天并非只能承载纯文本，消息还可以包含[文件与自定义元素](https://docs.chainlit.io/concepts/element)。RambleDesk 的设计问题是：**这次需要人做什么、应提供什么材料、反馈如何对应到材料，以及怎样把结果交回任务。** 它关注把已有交互思路用于这条工作流程，不声称为生成式 UI 发明了新的基础路线。

## 相关研究

以下使用正式发表年份，预印本会单独标明；链接指向论文或作者、机构提供的原始资料。设计联系是我们据此提出的理解，不是论文对 RambleDesk 的评价。

### Luminate: Structured Generation and Exploration of Design Space with Large Language Models for Human-AI Co-Creation

**Sangho Suh、Meng Chen、Bryan Min、Toby Jia-Jun Li、Haijun Xia，2024，CHI。** [作者上传的论文](https://arxiv.org/abs/2310.12953) · [机构 PDF](https://hci.ucsd.edu/papers/luminate.pdf) · [DOI](https://doi.org/10.1145/3613904.3642400)。预印本首发于 2023 年，v3 日期为 2024-03-13。

将多个候选的生成和设计空间组织起来，供人探索、评估与综合，并以 14 位专业写作者进行研究。相关之处是：确定方向前，可以比较和发展多个方案。它不是 RambleDesk 卡片继承或存储合同的来源，也不能证明本工作台复现了论文的研究结果。

### Wordcraft: a Human-AI Collaborative Editor for Story Writing

**Andy Coenen、Luke Davis、Daphne Ippolito、Emily Reif、Ann Yuan，2021，扩展摘要／预印本。** [论文](https://arxiv.org/abs/2107.07430) · [官方项目仓库](https://github.com/PAIR-code/wordcraft)。

把可编辑的故事与模型续写、候选表达、展开和选文改写结合起来。它为“请模型发展某个具体部分，同时由人继续塑造作品”提供参考。

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
