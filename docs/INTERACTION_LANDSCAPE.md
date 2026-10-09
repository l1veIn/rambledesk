# Interaction approaches and related research

Sources checked: **2026-10-09**.

[English](INTERACTION_LANDSCAPE.md) · [简体中文](INTERACTION_LANDSCAPE.zh-CN.md) · [Back to README](../README.md)

Interfaces for working with agents can be understood from two angles: how people participate in a task, and how developers build that interaction. This overview groups approaches by their main focus to explain RambleDesk's place.

## How to read the two-axis map

The README uses Mermaid `quadrantChart` to show five illustrative points. Coordinates serve layout only; they are not measurements, a fixed cost ordering, or proportional values:

- **X axis: degree of UI interaction**, from text conversation through visual presentation to direct manipulation of materials or controls.
- **Y axis: additional token consumption**, from low to high, representing model input and output added to provide the interface for the same underlying task.

The five points represent these implementation routes and design goals:

- **Classic chat:** does not require the model to generate a UI, making it the zero baseline for additional UI overhead; the conversation itself still consumes tokens.
- **`askUserQuestion`:** the application implements the interface in advance, and the agent supplies parameters such as questions and options.
- **RambleDesk prebuilt workbenches:** the agent supplies workbench parameters. The lower-right position expresses RambleDesk's design goal: direct interaction with less need to generate interface descriptions on demand.
- **Dynamic A2UI interfaces:** the model generates component and data JSON for client rendering; overhead varies with description size and update frequency.
- **Dynamically generated full HTML/JS:** the model generates interface code for the interaction, illustrated toward the upper right. HTML interfaces written in advance and reused do not belong to this route.

The relative cost of dynamic A2UI and HTML/JS can change with the implementation; their plotted positions do not establish a fixed ordering.

Tool schemas, input-protocol instructions, generated parameters, and UI descriptions can all add overhead; prebuilt interfaces are not free of it. The map excludes the underlying task's context, reasoning, code generation, and answers. **It is a design illustration, not a benchmark, and does not represent total end-to-end token consumption or actual prices.**

A protocol name does not determine the vertical position: [AG-UI](https://docs.ag-ui.com/introduction) carries events and state without requiring HTML generation, and [MCP Apps](https://github.com/modelcontextprotocol/ext-apps) can load prebuilt interfaces. The position depends on the implementation and the content passed to the model.

## What each approach addresses

| Approach | Main focus | Problem addressed | Relationship to workbenches |
| --- | --- | --- | --- |
| Classic chat | Interaction pattern | Organizes a conversation through messages, which can also contain images, files, and custom elements. See Chainlit's [Message](https://docs.chainlit.io/concepts/message) and [Element](https://docs.chainlit.io/concepts/element). | A conversation can contain or accompany a workbench; chat is not limited to plain text. |
| [RambleDesk](../README.md) | Desktop application and interaction pattern | Prepares materials, ways to interact, and a result to return for human participation: answering, reviewing, annotating, or trying something. | Focuses on the person's task and on bringing answers, annotations, and attachments back to the agent's work. |
| [A2UI](https://a2ui.org/introduction/what-is-a2ui/) | Declarative UI protocol | Agents send JSON describing components and data; clients render it using their own component catalogs. | Describes an interface; the application still designs its materials, review actions, and result structure. |
| [AG-UI](https://docs.ag-ui.com/introduction) | Agent-to-frontend protocol | Connects agent execution and user interfaces through events carrying state and user interactions. | Connects the interaction process without prescribing chat or a particular workbench. |
| [MCP Apps](https://github.com/modelcontextprotocol/ext-apps) | Embedded tool interfaces | MCP tools declare HTML UI resources that supported hosts load in sandboxed iframes, with data exchanged through the host. | Gives tools a way to bring forms, charts, or other interactive interfaces into a host. |
| [CopilotKit](https://github.com/CopilotKit/CopilotKit) | Application SDK | Provides generative UI, shared state, and flows that wait for user input, confirmation, or edits. | Helps developers build applications with human participation; particular workbenches still require product design. |
| [assistant-ui](https://www.assistant-ui.com/docs/) | Frontend components and runtimes | Builds AI interfaces from components, connects backends through runtimes, and supports [generative UI](https://www.assistant-ui.com/docs/tools/generative-ui). | Supplies interface building blocks for combining conversation with dedicated interactions. |
| [Chainlit](https://docs.chainlit.io/get-started/overview) | Python AI application framework | Organizes messages, steps, and UI elements; [Ask User](https://docs.chainlit.io/advanced-features/ask-user) collects text, files, action choices, and form input. | Supports waiting for a person's response and collecting it through an input suited to the task. |

## How these concerns fit together

Chat, questions, and material review can coexist in one application. Interactions such as `askUserQuestion` collect answers around a question; workbenches extend that idea to documents, images, websites, terminals, and other materials that need human judgment or action.

UI descriptions and interaction connections can also work together. For example, A2UI is transport-agnostic, and its [official transport documentation](https://a2ui.org/concepts/transports/) describes carrying messages through AG-UI. SDKs, component libraries, and application frameworks help developers turn these capabilities into products.

RambleDesk's design questions are: **What does the person need to do, what material do they need, how does their feedback refer to it, and how does the result return to the task?** The table relates these questions to other projects' concerns; it does not claim RambleDesk uses the listed protocols or frameworks. Providing an MCP interface does not itself mean implementing MCP Apps. See [compatibility and integrations](COMPATIBILITY.md) for actual connection options.

## Related research

The dates below are formal publication years. Links point to papers or original sources provided by their authors or institutions. The design connections are our interpretation, not evaluations of RambleDesk by these papers.

### Principles of Mixed-Initiative User Interfaces

**Eric Horvitz, 1999, CHI.** [Paper and institutional page](https://www.microsoft.com/en-us/research/publication/principles-mixed-initiative-user-interfaces/)

Discusses combining automation with direct manipulation, including when to interrupt, how to clarify uncertain goals, and how people can invoke, stop, and refine services. It suggests keeping clear, low-cost ways to participate and correct results while an agent works independently.

### Guidelines for Human-AI Interaction

**Saleema Amershi et al., 2019, CHI.** [Paper](https://www.microsoft.com/en-us/research/wp-content/uploads/2019/01/Guidelines-for-Human-AI-Interaction-camera-ready.pdf)

Proposes and evaluates 18 guidelines covering capability communication, relevant context, correction, and granular feedback. These suggest making the review subject, feedback target, and consequences of submission clear.

### AI Chains: Transparent and Controllable Human-AI Interaction by Chaining Large Language Model Prompts

**Tongshuang Wu, Michael Terry, and Carrie J. Cai, 2022, CHI.** [Paper](https://arxiv.org/pdf/2110.01691)

Decomposes LLM tasks into connected steps whose intermediate results people can inspect and modify, evaluating this prompt-chain interaction in a 20-person study. This suggests accepting feedback on concrete intermediate work.

### Grounded Copilot: How Programmers Interact with Code-Generating Models

**Shraddha Barke, Michael B. James, and Nadia Polikarpova, 2023, OOPSLA1.** [Paper](https://shraddhabarke.github.io/raw/copilot.pdf)

Observes 20 programmers using early Copilot, distinguishing acceleration when the direction is known from exploration when it is uncertain. It records validation through code examination, execution, static analysis, and documentation, suggesting room for comparison, understanding, and correction.

### Validating AI-Generated Code with Live Programming

**Kasra Ferdowsi et al., 2024, CHI.** [Paper](https://www.cs.cornell.edu/~lerner/papers/leap_chi24.pdf)

Combines code generation with live runtime-value displays and studies validation support with 17 participants in a particular live programming environment. It suggests providing inspectable results and execution evidence.

These studies concern general design principles, particular tools, and bounded experimental settings. They offer design references for this project. RambleDesk's efficiency, accuracy, and use with modern agents carrying out long tasks still require their own evaluation.
