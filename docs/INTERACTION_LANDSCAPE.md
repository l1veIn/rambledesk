# Generative UI approaches and related research

Map sources checked: **2026-10-11**. Interaction and brainstorming references checked: **2026-10-12**.

[English](INTERACTION_LANDSCAPE.md) · [简体中文](INTERACTION_LANDSCAPE.zh-CN.md) · [Back to README](../README.md)

Generative UI lets a model help decide which interface to show, what content to put in it, or how to compose it. It includes selecting and filling prebuilt components, generating a structured component tree, and generating interface code. It does not necessarily mean generating HTML or JavaScript from scratch. The [AI SDK guide](https://ai-sdk.dev/docs/ai-sdk-ui/generative-user-interfaces), for example, connects tool results to React components written by the developer.

**Prebuilt templates and registered components are established approaches, not an invention of RambleDesk.** RambleDesk is an application-level experiment in using this approach for work: an agent chooses a prepared workbench, supplies the material, and receives the person's answers, annotations, or suggestions. It is a desktop workbench application, not a general-purpose UI framework. This overview does not claim that RambleDesk integrates the projects listed below; actual connection options are documented in [compatibility and integrations](COMPATIBILITY.md).

## Interaction references and related products

The list separates references used in recent interaction refinements from related products identified during later comparison. Discovering a similar project after development began does not make it the original inspiration for RambleDesk.

| Reference | Existing interaction | Relationship to this work |
| --- | --- | --- |
| [Plannotator](https://plannotator.ai/) · [official repository](https://github.com/backnotprop/plannotator) | Select text in plans or Markdown, comment or propose replacements, review code diffs, and send structured feedback to the agent. | A reference for the recent document-review work: making the displayed selection match the feedback target and keeping comments close to the original. RambleDesk's Markdown anchors and feedback contract remain its own implementation. |
| [Agentation](https://www.agentation.com/) · [interaction guide](https://www.agentation.com/features) · [official repository](https://github.com/benjitaylor/agentation) | Annotate page elements or text, include identifying context, and deliver feedback by copying it or through MCP. Its guide documents keyboard and cancellation behavior. | A reference for the recent web-review refinements around leaving selection mode, preserving drafts, and recovering focus. This does not mean RambleDesk embeds Agentation. |
| [MCP Feedback Enhanced](https://github.com/Minidoracat/mcp-feedback-enhanced) | An MCP tool opens a feedback interface, collects choices, text, or images, and returns human input to the agent. | A directly related feedback workflow identified during comparison, not a claim about RambleDesk's original inspiration or a dependency. |
| [Miro clustering](https://help.miro.com/hc/en-us/articles/4409706795410-Clustering) · [FigJam sorting and summarizing](https://help.figma.com/hc/en-us/articles/18711926790423-Sort-and-summarize-stickies-with-FigJam-AI) | Established whiteboard products let people select and group notes; FigJam can create grouped copies that people adjust, and Miro can cluster selected notes. | Existing interaction patterns relevant to the brainstorm board's editable cards, groups, and selection. They do not establish RambleDesk's cross-round inheritance or attachment guarantees. |

These are interaction references and comparisons. Code reuse and third-party dependencies are documented separately in [third-party notices](../THIRD_PARTY_NOTICES.md).

## How to read the two-axis map

The README's “RambleDesk on the Generative UI map” labels selected uses of projects and interaction patterns: Classic chat, askUserQuestion, Chainlit, AI SDK, assistant-ui, CopilotKit, RambleDesk, MCP Apps, Tambo, A2UI, json-render, OpenUI, and OpenIntelligentUI. AG-UI appears in the title as a connection layer, not as a plotted point; this does not imply that every listed project uses it.

- **X axis: degree of UI interaction**, from text conversation through visual presentation to direct manipulation of materials or controls.
- **Y axis: additional token consumption**, representing model input and output added to provide the interface for the same underlying task.

**Positions illustrate selected uses, not measured costs or permanent product rankings.** One project can support several approaches. Reusing an interface can provide rich interactions without generating its code each time, while schemas, parameters, materials, and returned feedback still have costs. The map excludes the underlying task's ordinary context, reasoning, code generation, and answers; it is not an end-to-end token benchmark or a price comparison.

## Existing community maps

- [CopilotKit's Generative UI Spectrum](https://www.copilotkit.ai/generative-ui-spectrum) organizes Controlled, Declarative, and Open-ended UI by developer control and agent freedom.
- [OpenUI's State of Generative UI in 2026](https://www.openui.com/blog/state-of-generative-ui-report), by the Thesys Engineering Team, separates where an interface appears from what the agent emits.
- [AG-UI.AI's ecosystem map](https://ag-ui.ai/en/map) groups interaction, declarative rendering, embedded applications, and implementation into four layers. It is an independent community resource, unaffiliated with the projects' maintainers, not an official AG-UI map.

These are existing classifications with different dimensions; none uses this map's UI-interaction × additional-token axes.

## Existing component and application approaches

These examples show different developer-facing building blocks. A single framework can support several map patterns; none has one permanent position or token cost.

| Project | What the model provides and the developer prepares | Interaction and feedback |
| --- | --- | --- |
| [AI SDK](https://ai-sdk.dev/docs/ai-sdk-ui/generative-user-interfaces) | The model selects tools and supplies arguments; the application executes tools and maps their results to prewritten React components. | [Client-side tools](https://ai-sdk.dev/docs/ai-sdk-ui/chatbot-tool-usage) can collect user input and return tool outputs to continue the conversation. |
| [Tambo](https://docs.tambo.co/concepts/generative-interfaces) | Developers register React components, descriptions, and prop schemas; the agent selects components and streams their props. It can also update pre-placed components. | Direct editing and conversation can work on the same interface. [Component state](https://docs.tambo.co/concepts/generative-interfaces/component-state) must be explicitly exposed for the model to see user edits; ordinary React state is private by default. |
| [json-render](https://json-render.dev/docs/specs) | The model generates a JSON UI spec. A [catalog](https://json-render.dev/docs/catalog) defines allowed components and actions; a [registry](https://json-render.dev/docs/registry) supplies their implementations. | State bindings and action handlers support forms and other interactions. The application decides what to execute and what to send back to the model. |
| [OpenUI](https://github.com/thesysdev/openui) | The model emits OpenUI Lang using a defined component library; the renderer progressively builds the interface from those components. | [Action callbacks and form state](https://www.openui.com/docs/openui-lang/interactivity) carry user intent and field values, including actions that continue the conversation. |
| [CopilotKit](https://github.com/CopilotKit/CopilotKit) | An application SDK supporting generative UI, component registration, and shared state. It offers several implementation approaches rather than one UI format. | Supports user interaction and flows that wait for input, confirmation, or edits. |
| [assistant-ui](https://www.assistant-ui.com/docs/tools/generative-ui) | Frontend components and runtimes connect AI backends to interfaces, including custom renderers for tool calls. | Developers can combine conversation, tool interfaces, and user input in their application. |
| [Chainlit](https://docs.chainlit.io/get-started/overview) | A Python application framework organizing messages, steps, and UI elements. | [Ask User](https://docs.chainlit.io/advanced-features/ask-user) collects text, files, action choices, and form input while a workflow waits for a response. |
| [OpenIntelligentUI](https://github.com/CopilotKit/OpenIntelligentUI) | Formerly OpenGenerativeUI; supports native components and custom interactive interfaces. The map selects its HTML/CSS/JS generation mode. | Local controls run in a sandbox; user follow-ups can start another agent turn through the host bridge. |

Thesys now describes its hosted **OpenUI Cloud** as the former **C1 API**. The hosted service and the OpenUI framework are related, not two unrelated implementation routes. See the [official Thesys overview](https://www.thesys.dev/).

## Protocols and hosting are different layers

| Project | Layer and purpose | What it does not determine |
| --- | --- | --- |
| [A2UI](https://a2ui.org/introduction/what-is-a2ui/) | A declarative UI protocol: agents describe components and data in JSON, and clients render them using component catalogs. | It does not supply every application's review workflow or require arbitrary HTML/JS generation. |
| [AG-UI](https://docs.ag-ui.com/introduction) | An agent-to-frontend protocol carrying events, state, and interaction across the agent execution lifecycle. | It does not require HTML generation or prescribe chat, a component tree, or a particular workbench. |
| [MCP Apps](https://github.com/modelcontextprotocol/ext-apps) | A way for tools to expose interactive HTML resources that supported hosts load in sandboxed iframes, with data and actions exchanged through the host. | HTML can be prebuilt. The protocol alone does not imply on-demand code generation or a particular token cost. |

These layers can work together. A2UI is transport-agnostic, and its [transport documentation](https://a2ui.org/concepts/transports/) describes carrying messages through AG-UI. Component renderers can run inside an embedded app. Providing an MCP tool interface does not by itself mean implementing MCP Apps.

Chat, questions, and material review can also coexist. Chat is not inherently text-only: messages can contain [files and custom elements](https://docs.chainlit.io/concepts/element). RambleDesk's design questions are: **What does the person need to do, what material do they need, how does their feedback refer to it, and how does the result return to the task?** Its focus is applying existing interaction ideas to that workflow, not claiming a new foundation for generative UI.

## Related research

The dates below are formal publication years unless explicitly marked as a preprint. Links point to papers or original sources provided by their authors or institutions. The design connections are our interpretation, not evaluations of RambleDesk by these papers.

### Luminate: Structured Generation and Exploration of Design Space with Large Language Models for Human-AI Co-Creation

**Sangho Suh, Meng Chen, Bryan Min, Toby Jia-Jun Li, and Haijun Xia, 2024, CHI.** [Author-uploaded paper](https://arxiv.org/abs/2310.12953) · [Institutional PDF](https://hci.ucsd.edu/papers/luminate.pdf) · [DOI](https://doi.org/10.1145/3613904.3642400). The preprint first appeared in 2023; v3 is dated March 13, 2024.

Structures the generation of multiple alternatives so people can explore, evaluate, and synthesize a design space; the study involved 14 professional writers. This is relevant to comparing and developing several directions before committing to one. It is not a source for RambleDesk's card-inheritance or storage contract, nor evidence that this workbench reproduces the study's results.

### Wordcraft: a Human-AI Collaborative Editor for Story Writing

**Andy Coenen, Luke Davis, Daphne Ippolito, Emily Reif, and Ann Yuan, 2021, extended abstract / preprint.** [Paper](https://arxiv.org/abs/2107.07430) · [Official project repository](https://github.com/PAIR-code/wordcraft).

Combines an editable story with model-assisted continuation, alternatives, elaboration, and rewriting of selected text. It provides a reference for asking the model to develop a particular part while the person continues shaping the work.

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
