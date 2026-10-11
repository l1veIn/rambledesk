<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/social/readme-header-dark.webp" />
    <source media="(prefers-color-scheme: light)" srcset="docs/social/readme-header-light.webp" />
    <img src="docs/social/readme-header-light.webp" alt="RambleDesk" width="640" />
  </picture>
</p>

<p align="center"><strong>A new interface for working with agents</strong></p>

<p align="center">
  <a href="README.md">English</a> · <a href="README.zh-CN.md">简体中文</a>
</p>

**RambleDesk wants to move collaboration beyond the chat box.**

RambleDesk is a desktop workbench for working with coding agents. When you need to try their work, point out a problem, or decide what happens next, it gives you a place to do that.

Leave suggestions beside a document, mark a problem in an image, or write or speak your thoughts. RambleDesk sends your feedback together with the content it refers to, so the agent can use it to continue the task.

[Download](https://github.com/l1veIn/rambledesk/releases/latest) · [Get started](#get-started) · [Documentation](docs/README.md)

## What do people do when working with coding agents?

Agents can write code, research a topic, and run tools on their own. But moving a task forward still often needs your involvement:

- **Add context they don't have.** Explain the business background, clarify a requirement, or answer a question.
- **Choose a direction.** Compare approaches, express a preference, or set priorities.
- **Review what they wrote.** Read a plan, a draft, or code, and suggest changes in the relevant places.
- **Try it yourself.** Open a page, use an interface, or run a CLI, and note what works and where you get stuck.
- **Check the result.** Verify table values, details in an image, or a passage in audio or video.

If you've used `askUserQuestion`, you already know one kind of workbench interaction: the agent presents a question and options, asks you to answer, and continues based on your response. RambleDesk's **questions workbench** follows that familiar pattern and extends it to other tasks. Reviewing a document brings up the text and annotation controls; trying a page brings up the page and a place for feedback.

**A workbench prepares the material and controls for a task that needs your input, then collects your response for the agent.** You don't have to turn every judgment into a chat message. Answers, annotations, and attachments return to the agent with your feedback. In sessions started inside RambleDesk, submitting feedback queues the next turn in the original conversation.

### Which workbenches are available?

RambleDesk provides eleven workbenches in the current source:

| Workbench | What you can do |
| --- | --- |
| Questions | Answer questions one by one, choose options, and add explanations, much like `askUserQuestion` |
| Free feedback | Follow review steps and record feedback with text, voice, and attachments |
| Document review | Review Markdown and annotate selected text, cross-paragraph ranges, or the whole document |
| Image annotation and canvas | Draw arrows, boxes, and text on an image or blank canvas to show where and what to change |
| Web review | View a page and leave comments on specific elements |
| Terminal trials | Use a real CLI, quote its output, and record your experience |
| Drag sorting | Reorder options to express priorities or preferences |
| Brainstorming | Develop idea cards, group and select directions, and carry your edits into the next round |
| Diff review | Review code changes and comment on lines, ranges, or hunks |
| Table review | Suggest cell values and add comments |
| Audio and video review | Play the material and leave feedback on a timestamp or segment |

Annotations and proposed rewrites are saved as feedback and suggestions. Actions in a web page or terminal have their usual effects. See the [workbench playground](playground/workbenches/README.md) for more examples.

## Workbench examples

See a few ways to work with an agent.

### Document review: suggest changes beside the original text

Select a passage, add a comment or suggested rewrite beside it, then keep reading.

![Document review: selecting text and writing a comment beside the original](docs/screenshots/en/document-review.gif)

### Image annotation: point out what needs changing

Mark an area and explain the adjustment so the agent knows what you're referring to.

![Image annotation: marking a region and explaining what should change](docs/screenshots/en/visual-feedback.gif)

### Brainstorming: shape ideas and choose what comes next

Edit and group idea cards, select directions, then ask the agent to expand, combine, or continue with your choices. Your edits carry into the next round.

![Brainstorming: editing idea cards, grouping them, and selecting directions](docs/screenshots/en/brainstorm.gif)

### Free feedback: try it, then leave your thoughts

Follow the agent's steps to try the result. Write or speak your observations, and attach supporting material when needed.

![Free feedback: following review steps and recording observations](docs/screenshots/en/free-feedback.gif)

You can also add text, voice, and attachments. Optional AI cleanup helps organize your wording while preserving your original feedback. Unfinished feedback is saved as a draft.

## Get started

Download **0.5.0** for **Windows x64** or **macOS Apple Silicon** from [GitHub Releases](https://github.com/l1veIn/rambledesk/releases).

### Start a task inside RambleDesk

1. **Connect an agent.** Open **Settings → Agents**. Follow the prompts to install supported components or connect an existing agent such as Claude Code, Codex CLI, or Gemini CLI. Sign-in and model access belong to the agent itself.
2. **Give it a task.** Start a session, choose an agent and project folder, and describe your goal.
3. **Try it and respond.** When a feedback request arrives, review the result, follow its steps, and submit your comments. Feedback returns to the original conversation so the task can continue.

### Connect your existing agent app or CLI

Open **Settings → External adapters** and follow the [integration guide](docs/COMPATIBILITY.md). How the task continues after delivery depends on the adapter; Generic MCP requires returning to the agent manually.

### Voice and AI cleanup

Text feedback needs no extra setup. For speech, download a local transcription model in **Settings → Voice** and allow microphone access. AI cleanup is optional and needs a separately configured model service.

### Installation and upgrades

The Windows installer is not Authenticode-signed; macOS builds are not notarized. See the [installation guide](docs/INSTALLATION.md).

Before upgrading from 0.4.0, make a complete backup: **0.4.0 cannot open the upgraded database**. See [upgrade and rollback guidance](docs/DATA_COMPATIBILITY.md).

## Why this design?

From ChatGPT to today's coding agents, interfaces have moved into IDEs, terminals, and browsers. We still tend to put our requests, questions, and feedback into a chat box.

As agents take on more work independently, the moments that need human involvement change: trying the result, adding context the agent lacks, or choosing between possible directions.

RambleDesk organizes interaction around those moments. It brings the current work, the questions that need your judgment, and your feedback together, giving each review a clear focus and a path back to the original task.

It suits a workflow where the agent works independently and you step in at meaningful points to try the result and give feedback.

## RambleDesk on the Generative UI map

```mermaid
---
config:
  quadrantChart:
    chartWidth: 800
    chartHeight: 640
    pointLabelFontSize: 16
    xAxisLabelFontSize: 16
    yAxisLabelFontSize: 16
  themeVariables:
    quadrant1Fill: transparent
    quadrant2Fill: transparent
    quadrant3Fill: transparent
    quadrant4Fill: transparent
---
quadrantChart
    title UI richness × extra token usage · AG-UI: connection layer
    x-axis "Text chat" --> "Direct interaction"
    y-axis "Fewer extra tokens" --> "More extra tokens"
    "Classic chat": [0.10, 0.06]
    "askUserQuestion": [0.29, 0.12]
    "Chainlit": [0.18, 0.28]
    "AI SDK": [0.33, 0.28]
    "assistant-ui": [0.49, 0.28]
    "CopilotKit": [0.65, 0.28]
    "RambleDesk": [0.81, 0.28] radius: 7, color: #0ea5e9
    "MCP Apps": [0.95, 0.28]
    "Tambo": [0.40, 0.65]
    "A2UI": [0.57, 0.65]
    "json-render": [0.74, 0.65]
    "OpenUI": [0.90, 0.65]
    "OpenIntelligentUI": [0.71, 0.90]
```

**RambleDesk applies the established prebuilt UI approach to work such as review, annotation, and hands-on trials.**

Positions illustrate selected uses, not measured token costs; a project may span several positions. For an existing overview, see [CopilotKit's Generative UI Spectrum](https://www.copilotkit.ai/generative-ui-spectrum). See [related resources](docs/INTERACTION_LANDSCAPE.md) for project sources and more community maps.

## Run from source

Install Git, Node.js, pnpm, Rust, and your platform's Tauri dependencies, then run:

```bash
git clone https://github.com/l1veIn/rambledesk.git
cd rambledesk
pnpm install --frozen-lockfile
pnpm dev
```

See [development setup](docs/DEVELOPMENT.md), [agent setup and recovery](docs/ACP_MANAGED_SESSIONS.md), the [workbench playground](playground/workbenches/README.md), or the [documentation index](docs/README.md).

## Interaction references

- **Review and annotation:** recent refinements draw on [Plannotator](https://plannotator.ai/) and [Agentation](https://www.agentation.com/).
- **Related feedback workflows:** [MCP Feedback Enhanced](https://github.com/Minidoracat/mcp-feedback-enhanced) is a close counterpart identified during later comparison.
- **Brainstorming:** [Miro](https://help.miro.com/hc/en-us/articles/4409706795410-Clustering) and [FigJam](https://help.figma.com/hc/en-us/articles/18711926790423-Sort-and-summarize-stickies-with-FigJam-AI) demonstrate established card-grouping patterns; [Luminate](https://arxiv.org/abs/2310.12953) and [Wordcraft](https://arxiv.org/abs/2107.07430) explore human–AI creative work.

See [sources and design connections](docs/INTERACTION_LANDSCAPE.md#interaction-references-and-related-products) for what informed these refinements and what is related work.

## Thanks

- [Codeg](https://github.com/xintaofei/codeg): ACP integration, agent conversations, settings, and appearance references
- [Snow Shot](https://github.com/mg-chao/snow-shot): the screenshot stack
- [RepoChan](https://github.com/l1veIn/repochan-mono): brand and character assets
- [Kotone](https://github.com/l1veIn/kotone): the local speech stack this workbench grew from

## License

[MIT](LICENSE). Adapted code and other third-party components retain their respective licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

<p align="center">
  <img src="docs/social/rambelle-chibi-footer.webp" alt="Rambelle hands over a feedback folio" width="400" />
</p>
