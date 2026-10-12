export const repoUrl = "https://github.com/l1veIn/rambledesk";
export const release = {
  version: "0.5.0",
  url: `${repoUrl}/releases/tag/v0.5.0`,
  mac: `${repoUrl}/releases/download/v0.5.0/RambleDesk_0.5.0_aarch64.dmg`,
  windows: `${repoUrl}/releases/download/v0.5.0/RambleDesk_0.5.0_x64-setup.exe`,
};
const docsRef = "v0.5.0";
export const links = {
  acp: `${repoUrl}/blob/${docsRef}/docs/ACP_MANAGED_SESSIONS.md`,
  adapters: `${repoUrl}/blob/${docsRef}/docs/COMPATIBILITY.md`,
  installation: `${repoUrl}/blob/${docsRef}/docs/INSTALLATION.md`,
  installationZh: `${repoUrl}/blob/${docsRef}/docs/INSTALLATION.zh-CN.md`,
  compatibility: `${repoUrl}/blob/${docsRef}/docs/DATA_COMPATIBILITY.md`,
  docs: `${repoUrl}/blob/main/README.md`,
  docsZh: `${repoUrl}/blob/main/README.zh-CN.md`,
  related: `${repoUrl}/blob/main/docs/INTERACTION_LANDSCAPE.md`,
  relatedZh: `${repoUrl}/blob/main/docs/INTERACTION_LANDSCAPE.zh-CN.md`,
  license: `${repoUrl}/blob/${docsRef}/LICENSE`,
};

interface Content {
  lang: string;
  path: string;
  alternatePath: string;
  alternateLabel: string;
  title: string;
  description: string;
  skip: string;
  nav: {
    label: string;
    workflow: string;
    start: string;
    download: string;
    menu: string;
    language: string;
  };
  hero: {
    eyebrow: string;
    lines: string[];
    body: string;
    download: string;
    watch: string;
    platforms: string;
    footnote: string;
  };
  workflow: {
    label: string;
    title: string;
    body: string;
    steps: { title: string; body: string }[];
  };
  workbenches: {
    label: string;
    title: string;
    body: string;
    previewNote: string;
    developmentLabel: string;
    play: string;
    pause: string;
    errorLabel: string;
    openVideo: string;
    items: {
      id: string;
      tab: string;
      title: string;
      body: string;
      alt: string;
      width: number;
      height: number;
      development: boolean;
    }[];
    catalogTitle: string;
    catalog: { name: string; development?: boolean }[];
    note: string;
    references: { label: string; href: string }[];
  };
  start: {
    label: string;
    title: string;
    body: string;
    steps: { title: string; body: string }[];
    acp: string;
    adapters: string;
    note: string;
  };
  download: {
    label: string;
    title: string;
    privacy: string;
    candidate: string;
    mac: string;
    windows: string;
    releaseNotes: string;
    installation: string;
    macNote: string;
    windowsNote: string;
    upgrade: string;
    upgradeGuide: string;
    voiceTitle: string;
    voiceNote: string;
    organizeTitle: string;
    organizeNote: string;
  };
  footer: { signature: string; docs: string; license: string };
}

export const locales = {
  en: {
    lang: "en",
    path: "/",
    alternatePath: "/zh/",
    alternateLabel: "中文",
    title: "RambleDesk — Work with agents, beyond the chat.",
    description:
      "Generative UI for working with agents. Review their work, mark what needs changing, and send feedback back to the task in a desktop workbench.",
    skip: "Skip to content",
    nav: {
      label: "Main navigation",
      workflow: "Workbenches",
      start: "Get started",
      download: "Download",
      menu: "Menu",
      language: "Read in Chinese",
    },
    hero: {
      eyebrow: "Generative UI for working with agents",
      lines: ["Work with agents.", "Beyond the chat."],
      body: "Read what it wrote. Try what it built. Point out what needs changing. RambleDesk brings your feedback and the material together, so your agent can continue the task.",
      download: "Get RambleDesk",
      watch: "See the workbenches",
      platforms: "macOS Apple Silicon · Windows x64",
      footnote: "Less composing a prompt. More showing what you mean.",
    },
    workflow: {
      label: "01 / How it works",
      title: "The right interface\nfor your part of the task.",
      body: "Used askUserQuestion? The idea is familiar: an agent asks, you answer, it continues. A workbench extends that approach to reviewing a document, marking an image, or trying a result.",
      steps: [
        {
          title: "The agent brings the material",
          body: "It explains what it has done, what needs your judgment, and how to try it.",
        },
        {
          title: "You respond where it matters",
          body: "Choose, annotate, write, or speak. Your feedback stays connected to the material you mean.",
        },
        {
          title: "The task moves forward",
          body: "Submit when you are ready. In sessions started inside RambleDesk, feedback returns to the same agent conversation.",
        },
      ],
    },
    workbenches: {
      label: "02 / Workbench examples",
      title: "Show what you mean.",
      body: "A few ways to take part, with the material in front of you. Play any six-second demo.",
      previewNote:
        "Recorded in the current development UI with demo material. Markdown document review and brainstorming are development previews, not included in the 0.5.0 download.",
      developmentLabel: "Development preview",
      play: "Play demo",
      pause: "Pause demo",
      errorLabel: "The demo could not play. You can open the video directly.",
      openVideo: "Open video",
      items: [
        {
          id: "document-review",
          tab: "Document review",
          title: "Document review: leave a note beside the text",
          body: "Select a passage, add a comment or suggested rewrite beside it, then keep reading.",
          alt: "Select text in a Markdown document and write feedback beside the original",
          width: 1600,
          height: 1334,
          development: true,
        },
        {
          id: "visual-feedback",
          tab: "Image annotation",
          title: "Image annotation: point to the change",
          body: "Mark a region and explain what to change, so your agent knows exactly where you mean.",
          alt: "Mark a region on an image and describe the change",
          width: 1600,
          height: 1000,
          development: false,
        },
        {
          id: "brainstorm",
          tab: "Brainstorming",
          title: "Brainstorming: lay out ideas, then choose a direction",
          body: "Edit cards, group ideas, and select directions to expand or combine. Your changes carry into the next round.",
          alt: "Edit idea cards, organize groups, and select directions on a brainstorm board",
          width: 1600,
          height: 1334,
          development: true,
        },
        {
          id: "free-feedback",
          tab: "Free feedback",
          title: "Free feedback: try it, then say what you noticed",
          body: "Follow the agent's steps, write or speak your observations, and attach material when it helps.",
          alt: "Follow the task steps and record observations in the feedback editor",
          width: 1600,
          height: 1000,
          development: false,
        },
      ],
      catalogTitle: "11 workbenches in the current source",
      catalog: [
        { name: "Questions" },
        { name: "Free feedback" },
        { name: "Markdown document review", development: true },
        { name: "Image annotation & canvas" },
        { name: "Web review" },
        { name: "Terminal tryout" },
        { name: "Drag-to-rank" },
        { name: "Brainstorming", development: true },
        { name: "Diff review" },
        { name: "Table review" },
        { name: "Audio & video review" },
      ],
      note: "RambleDesk is an experiment in applying generative UI to work, using prebuilt workbenches for different collaboration tasks.",
      references: [
        { label: "Explore all workbenches", href: links.docs },
        { label: "Interaction references & related work", href: links.related },
      ],
    },
    start: {
      label: "03 / Get started",
      title: "Keep the agent\nyou already use.",
      body: "Connect your coding agent in RambleDesk. Sign-in and model access stay with the agent.",
      steps: [
        {
          title: "Connect your agent",
          body: "Open Settings → Agents. Follow the prompts to install supported components or connect an agent you already have.",
        },
        {
          title: "Give it a task",
          body: "Start a session, choose your agent and project folder, and describe the goal.",
        },
        {
          title: "Try it. Send feedback.",
          body: "When a request arrives, review the work and submit your notes. Check delivery in the workbench as the conversation continues.",
        },
      ],
      acp: "Agent setup guide",
      adapters: "Connect an external agent",
      note:
        "Prefer your existing agent app or CLI? Use an external adapter. How the task resumes depends on the adapter; with Generic MCP, return to the agent to continue.",
    },
    download: {
      label: "RambleDesk 0.5.0",
      title: "Bring your next task.",
      privacy:
        "Feedback is stored locally. Submitting and optional AI tidying share the relevant content with your connected agent or model service.",
      candidate: "Stable release",
      mac: "Download for macOS",
      windows: "Download for Windows",
      releaseNotes: "Release notes",
      installation: "Installation guide",
      macNote:
        "Apple Silicon · ad-hoc signed, not notarized. See the installation guide if macOS blocks the first launch.",
      windowsNote:
        "Windows x64 · not Authenticode-signed. Verify the download source before allowing it through SmartScreen.",
      upgrade:
        "Upgrading from 0.4.0? Back up the complete data directory first. The old version cannot open an upgraded database.",
      upgradeGuide: "Backup and rollback guide",
      voiceTitle: "Write first. Speak when you want.",
      voiceNote:
        "Text needs no extra setup. For voice, download a local transcription model in Settings → Voice and allow microphone access.",
      organizeTitle: "AI tidying is optional",
      organizeNote:
        "It is off by default. Configure a model service in Settings → Post-processing to use it. Your original feedback is preserved.",
    },
    footer: {
      signature: "Rambling is all you need.",
      docs: "Documentation",
      license: "MIT license",
    },
  },
  zh: {
    lang: "zh-CN",
    path: "/zh/",
    alternatePath: "/",
    alternateLabel: "EN",
    title: "RambleDesk — 和 Agent 一起，把工作往前推。",
    description:
      "生成式 UI，面向与 Agent 协作的工作场景。在桌面工作台中审阅成果、指出问题、体验结果，把意见和对应材料一起交回任务。",
    skip: "跳到正文",
    nav: {
      label: "主导航",
      workflow: "工作台",
      start: "开始使用",
      download: "下载",
      menu: "菜单",
      language: "切换到英文",
    },
    hero: {
      eyebrow: "与 Agent 协作的生成式 UI 工作台",
      lines: ["和 Agent 一起，", "把工作往前推。"],
      body: "读读它写的内容，试试它做的东西，指出哪里需要改。RambleDesk 把你的意见和对应材料放在一起，交给 Agent 继续完成任务。",
      download: "下载 RambleDesk",
      watch: "看看工作台",
      platforms: "macOS Apple Silicon · Windows x64",
      footnote: "少一点组织 prompt，多一点直接表达。",
    },
    workflow: {
      label: "01 / 工作流程",
      title: "需要你参与时，\n就有合适的界面。",
      body: "用过 askUserQuestion？你已经熟悉这个过程：Agent 带着问题请你回答，再根据答案继续。工作台把同样的方式扩展到读文档、标图片和体验结果。",
      steps: [
        {
          title: "Agent 带着材料来",
          body: "说清做了什么、需要你判断什么，以及从哪里开始体验。",
        },
        {
          title: "你对着材料表达",
          body: "选择、批注、写字或说话。意见对应到具体内容，不必重新解释一遍上下文。",
        },
        {
          title: "反馈回到任务里",
          body: "准备好后统一提交。在 RambleDesk 内开始的会话中，反馈会交回原来的 Agent 对话。",
        },
      ],
    },
    workbenches: {
      label: "02 / 工作台示例",
      title: "对着材料，直接表达。",
      body: "看看几种具体的协作方式。切换工作台，点击播放查看 6 秒演示。",
      previewNote:
        "使用当前开发界面和演示材料录制。Markdown 文档审阅与头脑风暴标为开发预览，尚未包含在下载的 0.5.0 中。",
      developmentLabel: "开发预览",
      play: "播放演示",
      pause: "暂停演示",
      errorLabel: "演示暂时无法播放，可以直接打开视频查看。",
      openVideo: "打开视频",
      items: [
        {
          id: "document-review",
          tab: "文档审阅",
          title: "文档审阅：对着原文提出修改意见",
          body: "选中文字，在原文旁写下批注或建议改写，再继续阅读。",
          alt: "在 Markdown 文档中选中文字，在原文旁填写批注",
          width: 1600,
          height: 1334,
          development: true,
        },
        {
          id: "visual-feedback",
          tab: "图片标注",
          title: "图片标注：指出哪里需要调整",
          body: "圈出一个区域，直接说明改法，让 Agent 知道你指的是哪里。",
          alt: "在图片上圈出区域，说明需要调整的位置",
          width: 1600,
          height: 1000,
          development: false,
        },
        {
          id: "brainstorm",
          tab: "头脑风暴",
          title: "头脑风暴：把想法摆出来，再决定下一步",
          body: "改写卡片、分组和多选方向，再请 Agent 展开或组合。你的修改会保留到下一轮。",
          alt: "编辑想法卡片、调整分组并选择头脑风暴的方向",
          width: 1600,
          height: 1334,
          development: true,
        },
        {
          id: "free-feedback",
          tab: "自由反馈",
          title: "自由反馈：试过之后，把想法留下来",
          body: "跟着 Agent 给出的步骤体验结果，直接写下或说出观察，必要时附上材料。",
          alt: "跟着体验步骤，在反馈编辑器中记录观察和意见",
          width: 1600,
          height: 1000,
          development: false,
        },
      ],
      catalogTitle: "当前源码提供 11 种工作台",
      catalog: [
        { name: "问答" },
        { name: "自由反馈" },
        { name: "Markdown 文档审阅", development: true },
        { name: "图片标注与画布" },
        { name: "网页评审" },
        { name: "终端试用" },
        { name: "拖动排序" },
        { name: "头脑风暴", development: true },
        { name: "差异评审" },
        { name: "表格审阅" },
        { name: "音视频审阅" },
      ],
      note: "RambleDesk 是生成式 UI 在工作场景的应用尝试，使用预置工作台承接不同的协作任务。",
      references: [
        { label: "查看完整工作台说明", href: links.docsZh },
        { label: "交互参考与相关工作", href: links.relatedZh },
      ],
    },
    start: {
      label: "03 / 开始使用",
      title: "沿用你已经在用的 Agent。",
      body: "在 RambleDesk 中连接编程 Agent，登录与模型访问仍由 Agent 自身提供。",
      steps: [
        {
          title: "连接 Agent",
          body: "打开「设置 → Agents」，按提示安装支持的组件，或连接已有的 Agent。",
        },
        {
          title: "给它一个任务",
          body: "新建会话，选择 Agent 和项目目录，告诉它你的目标。",
        },
        {
          title: "体验并反馈",
          body: "收到反馈请求后，审阅成果、按步骤体验，再提交意见。送达状态在工作台中可见，对话接着往下走。",
        },
      ],
      acp: "Agent 使用指南",
      adapters: "接入外部 Agent",
      note:
        "也可以通过外部适配器，沿用你现有的 Agent 应用或 CLI。提交后的继续方式取决于适配器；通用 MCP 需要回到 Agent 继续。",
    },
    download: {
      label: "RambleDesk 0.5.0",
      title: "带上你的下一个任务。",
      privacy:
        "反馈保存在本机；提交和可选的 AI 整理会把相应内容交给你连接的 Agent 或模型服务。",
      candidate: "正式版",
      mac: "下载 macOS 版",
      windows: "下载 Windows 版",
      releaseNotes: "发布说明",
      installation: "安装指南",
      macNote:
        "Apple Silicon · 采用 ad-hoc 签名，尚未公证。若 macOS 阻止首次启动，请按安装指南操作。",
      windowsNote:
        "Windows x64 · 尚未做 Authenticode 签名。若 SmartScreen 拦截，请先确认下载来源。",
      upgrade:
        "从 0.4.0 升级前，请完整备份数据目录。旧版无法打开升级后的数据库。",
      upgradeGuide: "备份与回退说明",
      voiceTitle: "直接打字，也可以说话",
      voiceNote:
        "文字反馈无需额外配置。需要语音时，在「设置 → 语音」下载本地转写模型，并允许麦克风访问。",
      organizeTitle: "需要时，再用 AI 整理",
      organizeNote:
        "整理默认关闭。需要时在「设置 → 后处理」配置模型服务，原始反馈会保留。",
    },
    footer: {
      signature: "Rambling is all you need.",
      docs: "文档",
      license: "MIT 许可",
    },
  },
} satisfies Record<string, Content>;

export type SiteContent = Content;
