export const releaseUrl = 'https://github.com/l1veIn/rambledesk/releases'
export const repoUrl = 'https://github.com/l1veIn/rambledesk'

export const locales = {
  en: {
    lang: 'en',
    path: '/',
    alternatePath: '/zh/',
    alternateLabel: '简体中文',
    title: 'RambleDesk',
    description:
      'A local human-feedback workbench for coding agents. Receive structured requests, ramble with voice, screenshots, notes, and files, then return an immutable feedback package.',
    nav: {
      product: 'Product',
      loop: 'Loop',
      adapters: 'Adapters',
      download: 'Download',
    },
    hero: {
      eyebrow: 'Local-first human feedback for coding agents',
      headline: 'Rambling is all you need.',
      subhead:
        'RambleDesk turns messy human experience into durable feedback packages your agent can read, cite, and continue from.',
      primaryCta: 'Download RambleDesk',
      secondaryCta: 'Watch the loop',
      liveBadge: 'Local archive online',
      requestTitle: 'Homepage motion review',
      requestBody: 'Test the opening animation, capture issues, and ramble freely.',
      packageTitle: 'Feedback Package',
      packageBody: 'feedback.md + manifest.json + attachments',
    },
    workflowKicker: 'The loop',
    workflowTitle: 'From human ramble to agent continuation.',
    workflowIntro:
      'The site animation follows the product contract: requests persist first, humans answer freely, and the final artifact is a portable package instead of another chat message.',
    workflowCode: '02 / Human loop',
    workflow: [
      {
        label: 'Request',
        title: 'Agent asks clearly',
        body: 'The host sends context, actions, and evidence needs as a persistent Feedback Request.',
      },
      {
        label: 'Ramble',
        title: 'Human answers freely',
        body: 'Speak, type, paste, attach files, and capture annotated screenshots without reshaping your thought process first.',
      },
      {
        label: 'Package',
        title: 'Evidence gets sealed',
        body: 'RambleDesk publishes feedback.md, uncooked.md, manifest.json, attachments, hashes, and paths as one immutable package.',
      },
      {
        label: 'Continue',
        title: 'Agent resumes work',
        body: 'The original host reads the package and continues iteration with concrete human evidence.',
      },
    ],
    proofKicker: 'Built for serious local work',
    proofTitle: 'It feels cinematic because the contract is concrete.',
    surfaceIntro:
      'The page treats RambleDesk like a local operations deck: the request enters on the left, evidence is captured in the center, and a sealed package leaves through a verified rail.',
    surface: {
      sectionCode: '01 / Local contract',
      deckTitle: 'Feedback OS',
      deckStatus: 'vault synced',
      deckMeta: ['loopback', 'token guard', 'SQLite'],
      captureTitle: 'Capture stream',
      captureItems: ['voice', 'annotation', 'files'],
      vaultTitle: 'Evidence vault',
      vaultFiles: ['feedback.md', 'uncooked.md', 'manifest.json', 'attachments/'],
    },
    proof: [
      {
        value: 'local',
        label: 'Loopback listener, token guard, SQLite, and package files stay on your machine.',
      },
      {
        value: 'multimodal',
        label: 'Voice, screenshots, annotations, pasted context, and ordinary files all enter the same draft.',
      },
      {
        value: 'host-neutral',
        label: 'Generic MCP plus native adapters let Codex, Claude, Pi, OpenCode, and others call the same workbench.',
      },
    ],
    featureKicker: 'Workbench anatomy',
    featureTitle: 'A workbench for the messy middle.',
    features: [
      'Structured task brief with executable actions',
      'Ramble mode with local speech capture',
      'Screenshot capture and annotation workflow',
      'Feedback Cooking with original evidence preserved',
      'Immutable package output for agent continuation',
      'Adapter setup for common coding hosts',
    ],
    adaptersKicker: 'Host adapters',
    adaptersCode: '04 / Adapter dock',
    adaptersTitle: 'Installed where coding agents already work.',
    adapters: ['Codex', 'Claude Code', 'Cursor', 'Gemini CLI', 'Grok', 'OpenCode', 'Reasonix', 'Pi', 'DeepSeek Harness'],
    ctaTitle: 'Wake the workbench when the agent needs your eyes.',
    ctaBody:
      'Open RambleDesk, enable your host adapter, and let the next hard product judgment become a sealed feedback package.',
    ctaButton: 'Get the release',
  },
  zh: {
    lang: 'zh-CN',
    path: '/zh/',
    alternatePath: '/',
    alternateLabel: 'English',
    title: 'RambleDesk',
    description:
      'RambleDesk 是面向 coding agent 的本地人类反馈工作台：接收结构化请求，用语音、截图、文字和文件自由 ramble，再交回不可变反馈包。',
    nav: {
      product: '产品',
      loop: '闭环',
      adapters: '适配器',
      download: '下载',
    },
    hero: {
      eyebrow: '面向 Coding Agent 的本地人类反馈工作台',
      headline: 'Rambling is all you need.',
      subhead:
        'RambleDesk 把人类真实体验、截图、语音和胡言乱语封存成 agent 能读取、引用并继续工作的反馈包。',
      primaryCta: '下载 RambleDesk',
      secondaryCta: '观看闭环',
      liveBadge: '本地档案库在线',
      requestTitle: '主页动效验收',
      requestBody: '测试开屏动画，截图标注问题，然后自由 ramble。',
      packageTitle: 'Feedback Package',
      packageBody: 'feedback.md + manifest.json + attachments',
    },
    workflowKicker: '核心闭环',
    workflowTitle: '从人类 ramble 到 agent 继续迭代。',
    workflowIntro:
      '主页动效遵循产品合同：请求先持久化，人类自由表达，最终产物是可携带的反馈包，而不是又一段聊天记录。',
    workflowCode: '02 / 人类闭环',
    workflow: [
      {
        label: 'Request',
        title: 'Agent 清楚发问',
        body: '宿主发送背景、动作清单和证据要求，形成持久化 Feedback Request。',
      },
      {
        label: 'Ramble',
        title: '人类自由回答',
        body: '语音、文字、粘贴、文件、截图批注都进入同一个草稿，不需要先整理成提示词。',
      },
      {
        label: 'Package',
        title: '证据被封存',
        body: 'RambleDesk 发布 feedback.md、uncooked.md、manifest.json、附件、hash 和路径。',
      },
      {
        label: 'Continue',
        title: 'Agent 继续工作',
        body: '原宿主读取反馈包，用真实人类证据继续实现和修正。',
      },
    ],
    proofKicker: '认真服务本地工作',
    proofTitle: '画面可以电影化，因为产品合同很具体。',
    surfaceIntro:
      '页面把 RambleDesk 当成一张本地操作台来设计：请求从左侧进入，中间采集真实证据，最终沿着可验证轨道输出封存包。',
    surface: {
      sectionCode: '01 / 本地合同',
      deckTitle: 'Feedback OS',
      deckStatus: '档案库已同步',
      deckMeta: ['loopback', 'token guard', 'SQLite'],
      captureTitle: '采集流',
      captureItems: ['语音', '批注', '文件'],
      vaultTitle: '证据库',
      vaultFiles: ['feedback.md', 'uncooked.md', 'manifest.json', 'attachments/'],
    },
    proof: [
      {
        value: 'local',
        label: 'Loopback、token guard、SQLite 和反馈包文件都留在本机。',
      },
      {
        value: 'multimodal',
        label: '语音、截图批注、粘贴上下文和普通文件进入同一个反馈草稿。',
      },
      {
        value: 'host-neutral',
        label: 'Generic MCP 和原生适配器让 Codex、Claude、Pi、OpenCode 等宿主接入同一工作台。',
      },
    ],
    featureKicker: '工作台解剖',
    featureTitle: '为最混乱的体验过程而生。',
    features: [
      '带可执行动作的结构化任务说明',
      '本地语音采集的 Ramble 模式',
      '截图、批注、附件和粘贴上下文',
      '保留原始证据的 Feedback Cooking',
      '供 agent 继续工作的不可变反馈包',
      '常见 coding host 的适配器安装流程',
    ],
    adaptersKicker: '宿主适配器',
    adaptersCode: '04 / 适配器船坞',
    adaptersTitle: '安装在 coding agent 已经工作的地方。',
    adapters: ['Codex', 'Claude Code', 'Cursor', 'Gemini CLI', 'Grok', 'OpenCode', 'Reasonix', 'Pi', 'DeepSeek Harness'],
    ctaTitle: '当 agent 需要你的眼睛时，唤醒工作台。',
    ctaBody:
      '打开 RambleDesk，启用宿主适配器，让下一次困难的产品判断变成一份封存的反馈包。',
    ctaButton: '获取发布版',
  },
} as const

export type LocaleKey = keyof typeof locales
export type SiteContent = (typeof locales)[LocaleKey]

export const assets = {
  heroFallback: '/assets/hero-workbench-cinema.png',
  assistant: '/assets/rambelle-motion-cutout.png',
  sceneArchive: '/assets/scene-archive.webp',
  sceneCryo: '/assets/scene-cryo.webp',
  sceneGate: '/assets/scene-gate.webp',
  pattern: '/assets/rambelle-vault-pattern.webp',
}
