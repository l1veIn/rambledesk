import { feedbackExamples } from "./feedback-example";

export const repoUrl = "https://github.com/l1veIn/rambledesk";
export const release = {
  version: "0.4.0-rc.3",
  url: `${repoUrl}/releases/tag/v0.4.0-rc.3`,
  mac: `${repoUrl}/releases/download/v0.4.0-rc.3/RambleDesk_0.4.0-rc.3_aarch64.dmg`,
  windows: `${repoUrl}/releases/download/v0.4.0-rc.3/RambleDesk_0.4.0-rc.3_x64-setup.exe`,
};
const docsRef = "v0.4.0-rc.3";
export const links = {
  acp: `${repoUrl}/blob/${docsRef}/docs/ACP_MANAGED_SESSIONS.md`,
  adapters: `${repoUrl}/blob/${docsRef}/docs/COMPATIBILITY.md`,
  docs: `${repoUrl}/blob/${docsRef}/README.md`,
  license: `${repoUrl}/blob/${docsRef}/LICENSE`,
  earlyDemo: `${repoUrl}/releases/download/v0.3.2/rambledesk-demo-10s.gif`,
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
  request: {
    label: string;
    title: string;
    body: string;
    example: string;
    status: string;
    subject: string;
    happened: string;
    progress: string;
    ask: string;
    action: string;
    note: string;
  };
  demo: {
    label: string;
    title: string;
    body: string;
    recording: string;
    recordingNote: string;
  };
  start: {
    label: string;
    title: string;
    body: string;
    steps: { title: string; body: string }[];
    acp: string;
    adapters: string;
    voice: string;
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
    title: "RambleDesk — Your attention. For what matters.",
    description:
      "A local feedback workbench for coding agents. Get a clear request, speak and capture what you see, then return a lasting feedback package. Connect your own agent with ACP.",
    skip: "Skip to content",
    nav: {
      label: "Main navigation",
      workflow: "How it works",
      start: "Get started",
      download: "Download",
      menu: "Menu",
      language: "Read in Chinese",
    },
    hero: {
      eyebrow: "A local feedback workbench for coding agents",
      lines: ["Your attention.", "For what matters."],
      body: "Your agent writes a clear request. You speak, capture, and edit. Send your feedback so it can continue.",
      download: "Get RambleDesk",
      watch: "Explore the workflow",
      platforms: "macOS Apple Silicon · Windows x64",
      footnote: "Less prompt-writing. More judgment.",
    },
    request: {
      label: "01 / A clear request",
      title: "Know what needs\nyour eyes.",
      body: "Your agent explains what changed and what to try. Open the workbench and know where to begin.",
      example: "Example request",
      status: "Ready for your review",
      subject: feedbackExamples.en.caseTitle,
      happened: feedbackExamples.en.happened,
      progress: feedbackExamples.en.happenedBody,
      ask: feedbackExamples.en.experience,
      action: feedbackExamples.en.experienceBody,
      note: "Two clear fields. One place to begin.",
    },
    demo: {
      label: "02 / Your experience, in your words",
      title: "See it. Say it.",
      body: "Speak, take screenshots, or edit directly. Tidy it up if you want. Your original feedback stays.",
      recording: "Watch an earlier workflow demo ↗",
      recordingNote:
        "v0.3.2 · 10-second GIF · earlier interface; opens an animation",
    },
    start: {
      label: "03 / Your agent, connected",
      title: "Keep the agent\nyou already use.",
      body: "Connect a local agent through ACP. Your agent provides its own login and model access.",
      steps: [
        {
          title: "Connect your agent",
          body: "Check that it works on your machine. Open Settings → Agents and install a connection component if prompted.",
        },
        {
          title: "Give it a task",
          body: "Choose your agent and project. Tell it what you want to achieve.",
        },
        {
          title: "Try it. Send feedback.",
          body: "Follow the request, speak or capture what you see, then submit and check delivery in the workbench.",
        },
      ],
      acp: "ACP setup guide",
      adapters: "Use an external agent",
      voice:
        "Voice input needs a local transcription model. Download it once in Settings → Voice.",
    },
    download: {
      label: "Made for the human in the loop",
      title: "Your next piece of feedback?\nJust say it.",
      privacy:
        "Feedback is stored locally. Submitting and optional tidying share the relevant content with your connected agent or model service.",
      candidate: "Release candidate",
      mac: "Get it for macOS",
      windows: "Get it for Windows",
      releaseNotes: "Release notes",
      installation: "Before your first launch",
      macNote:
        "Open the DMG and drag RambleDesk into Applications. This build is ad-hoc signed and not notarized. After verifying the download source, use System Settings → Privacy & Security → Open Anyway if macOS blocks it.",
      windowsNote:
        "Run the x64 installer. It is not Authenticode signed. If SmartScreen blocks it, verify the source, then choose More info → Run anyway.",
      voiceTitle: "Voice, ready when you are",
      voiceNote:
        "Download a local transcription model in Settings → Voice and allow microphone access.",
      organizeTitle: "Tidying is optional",
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
    title: "RambleDesk — 把注意力，留给判断。",
    description:
      "面向 Coding Agent 的本地反馈工作台。Agent 写清体验单，你说话、截图、直接编辑，提交不可变反馈包。通过 ACP 沿用你本机的 Agent。",
    skip: "跳到正文",
    nav: {
      label: "主导航",
      workflow: "工作流程",
      start: "开始使用",
      download: "下载",
      menu: "菜单",
      language: "切换到英文",
    },
    hero: {
      eyebrow: "面向 Coding Agent 的本地反馈工作台",
      lines: ["把注意力，", "留给判断。"],
      body: "Agent 写清体验单。你说话、截图、直接编辑，提交反馈，让它继续。",
      download: "下载 RambleDesk",
      watch: "了解反馈流程",
      platforms: "macOS Apple Silicon · Windows x64",
      footnote: "少一点组织 prompt，多一点体验和判断。",
    },
    request: {
      label: "01 / 体验单",
      title: "先把需要你看的，\n写清楚。",
      body: "Agent 先交代进展，再提出体验请求。你打开工作台，就知道从哪里开始。",
      example: "体验单示例",
      status: "待体验",
      subject: feedbackExamples.zh.caseTitle,
      happened: feedbackExamples.zh.happened,
      progress: feedbackExamples.zh.happenedBody,
      ask: feedbackExamples.zh.experience,
      action: feedbackExamples.zh.experienceBody,
      note: "两件事写清楚，体验就有了起点。",
    },
    demo: {
      label: "02 / 直接表达",
      title: "看到什么，就说什么。",
      body: "说话、截图、直接编辑。需要时再整理，原始反馈会保留。",
      recording: "观看早期版本流程演示 ↗",
      recordingNote: "v0.3.2 · 10 秒 GIF · 界面为早期版本，打开后播放动画",
    },
    start: {
      label: "03 / 开始使用",
      title: "沿用你已经在用的 Agent。",
      body: "通过 ACP 连接本机 Agent，登录与模型访问由 Agent 自身提供。",
      steps: [
        {
          title: "连接 Agent",
          body: "确认本机可用，在「设置 → Agents」按提示连接，必要时安装连接组件。",
        },
        { title: "开始任务", body: "选择 Agent 和项目，告诉它你的目标。" },
        {
          title: "体验并反馈",
          body: "收到体验单，说话、截图或编辑，提交后在工作台查看送达状态。",
        },
      ],
      acp: "ACP 使用指南",
      adapters: "沿用外部 Agent",
      voice: "第一次使用语音，需要在「设置 → 语音」下载本地转写模型。",
    },
    download: {
      label: "把人的注意力放在心上",
      title: "下一次反馈，\n直接说出来。",
      privacy:
        "反馈保存在本机；提交和可选整理会把相应内容交给你连接的 Agent 或模型服务。",
      candidate: "候选版",
      mac: "下载 macOS 版",
      windows: "下载 Windows 版",
      releaseNotes: "发布说明",
      installation: "首次安装与使用说明",
      macNote:
        "打开 DMG，将 RambleDesk 拖入「应用程序」。当前版本采用 ad-hoc 签名，尚未公证。确认下载来源后，若 macOS 阻止启动，可在「系统设置 → 隐私与安全性」选择「仍要打开」。",
      windowsNote:
        "运行 x64 安装程序。当前安装包未做 Authenticode 签名；若 SmartScreen 拦截，确认来源后，选择「更多信息 → 仍要运行」。",
      voiceTitle: "第一次说话之前",
      voiceNote: "在「设置 → 语音」下载本地转写模型，并允许麦克风访问。",
      organizeTitle: "需要时，再整理",
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
