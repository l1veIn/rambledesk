import type { Locale } from '$lib/preferences'

export type SessionStarterId = 'brainstorm' | 'optimize' | 'develop'

export interface SessionStarter {
  id: SessionStarterId
  title: string
  description: string
  prompt: string
}

const firstRound: Record<Locale, string> = {
  'zh-CN': '第一轮先确认方向，不要直接开始实现或扫描整个仓库。如需了解项目，最多快速查看 README 和顶层目录。请先创建一次 RambleDesk 反馈请求并等待我回复，只使用当前需要的工作台。',
  en: 'Keep the first round focused on agreeing a direction. Do not start implementing or scan the whole repository. If project context is needed, take only a quick look at the README and top-level directory. Create one RambleDesk feedback request, then wait for my reply. Use only the workbench needed for this step.',
}

const starters: Record<Locale, readonly SessionStarter[]> = {
  'zh-CN': [
    {
      id: 'brainstorm', title: '头脑风暴', description: '从几个关键问题开始，把想法说清楚。',
      prompt: '我想一起头脑风暴，探索一个想法或新的方向。请先用 RambleDesk 的逐项问答（questions）工作台，问我最多 3 个必要问题，了解主题、目标和约束。收到回复后，再一起比较可能的方向。',
    },
    {
      id: 'optimize', title: '系统优化', description: '先找一个值得改善的点，再决定怎么改。',
      prompt: '我想改善这个项目的体验、结构或性能。请先做一次很小范围的了解，提出 1–2 个初步观察，然后用 RambleDesk 的自由反馈（ramble）工作台，让我补充当前最困扰的地方并确定优先级。收到回复后，再深入诊断选定的问题。',
    },
    {
      id: 'develop', title: '代码开发', description: '先确认要做什么，再分步实现。',
      prompt: '我想为这个项目开发一个功能或修复一个问题。请先用 RambleDesk 的逐项问答（questions）工作台，问我最多 3 个必要问题，明确要做什么、预期行为和验收方式。收到回复后，再确定一个小步实现计划。',
    },
  ],
  en: [
    {
      id: 'brainstorm', title: 'Brainstorm', description: 'A few key questions to shape your idea.',
      prompt: 'I want to brainstorm an idea or a new direction together. Start with a RambleDesk questions workbench and ask at most 3 essential questions about the topic, goal, and constraints. After my reply, help me compare possible directions.',
    },
    {
      id: 'optimize', title: 'Improve a system', description: 'Find one useful improvement, then decide how.',
      prompt: 'I want to improve this project’s experience, structure, or performance. Take a very small initial look and share 1–2 tentative observations in a RambleDesk ramble workbench. Invite me to describe the main pain points and agree on a priority. After my reply, investigate the selected problem in depth.',
    },
    {
      id: 'develop', title: 'Develop code', description: 'Agree on the task, then build it step by step.',
      prompt: 'I want to build a feature or fix a problem in this project. Start with a RambleDesk questions workbench and ask at most 3 essential questions about the task, expected behavior, and acceptance criteria. After my reply, agree on a small implementation plan.',
    },
  ],
}

export function getSessionStarters(locale: Locale): SessionStarter[] {
  return starters[locale].map(starter => ({ ...starter, prompt: `${starter.prompt}\n\n${firstRound[locale]}` }))
}
