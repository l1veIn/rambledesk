import type { Locale } from '../../preferences'

export type WorkbenchTourId = 'ramble' | 'document_review'

export type WorkbenchTourStep = {
  id: string
  target: string
  title: string
  body: string
}

export type WorkbenchTour = {
  id: WorkbenchTourId
  version: number
  steps: WorkbenchTourStep[]
  labels: {
    dialog: string
    skip: string
    back: string
    next: string
    done: string
    step: (index: number, total: number) => string
  }
  replayLabel: string
}

type LocalizedStep = Omit<WorkbenchTourStep, 'title' | 'body'> & {
  title: [string, string]
  body: [string, string]
}

const steps: Record<WorkbenchTourId, LocalizedStep[]> = {
  ramble: [
    {
      id: 'context', target: '[data-tour="request-context"]',
      title: ['先了解这次要反馈什么', 'Start with the context'],
      body: ['这里是本次请求的情况说明和参考材料。先了解目标，再按自己的想法给出反馈。', 'The request context and reference materials explain what needs your feedback. Start here, then share your thoughts.'],
    },
    {
      id: 'actions', target: '[data-tour="ramble-actions"]',
      title: ['点选体验项，围绕具体操作反馈', 'Start with an action to try'],
      body: ['如果请求列出了需要体验的操作，点击一项就会把它引用到右侧反馈正文。接着写下你的观察、问题或建议，也可以直接说出来。', 'When a request lists actions to try, click one to quote it in the feedback on the right. Then type or speak your observations, questions or suggestions.'],
    },
    {
      id: 'input', target: '[data-tour="feedback-input"]',
      title: ['打字、说话，都在这里', 'Type or speak your feedback'],
      body: ['这里是默认输入区。工具栏支持语音、截图、粘贴和附件；说出来的文字会带上小麦克风标记。', 'This is your default input area. Its toolbar supports voice, screenshots, pasting and attachments. A small microphone marks dictated text.'],
    },
    {
      id: 'tidy', target: '[data-tour="input-console"]',
      title: ['说完后，让我帮你整理', 'Let me tidy up when you are ready'],
      body: ['有待整理的语音文字时，我身边会出现整理按钮，一次整理各输入区里的语音内容，并回填原处。', 'When dictated text needs tidying, a button appears beside me. It tidies voice text across input areas and puts each result back where it belongs.'],
    },
    {
      id: 'submit', target: '[data-feedback-actions]',
      title: ['确认后，再提交反馈', 'Review, then send'],
      body: ['检查正文和附件，准备好后提交反馈。想再看一遍引导，随时可以从工作台标题旁重新打开。', 'Check your feedback and attachments, then submit when ready. You can reopen this guide beside the workbench title at any time.'],
    },
  ],
  document_review: [
    {
      id: 'source', target: '[data-review-text]',
      title: ['原稿留在这里，意见写在旁边', 'Keep the original, add your feedback'],
      body: ['原稿保持不变。审阅时可以选中一段中的文字，针对这段文字批注或提出建议改写。', 'The original stays unchanged. Select text within a paragraph to comment on a passage or suggest new wording.'],
    },
    {
      id: 'comment', target: '[data-tour="review-comment"]',
      title: ['一段一条批注，想到什么就续写', 'Keep each paragraph’s feedback together'],
      body: ['段落旁的批注和麦克风按钮都能开始批注。再次打开会接着原批注写，收起后保留一行摘要，方便回看。', 'The comment and microphone buttons start a paragraph note. Reopening it lets you continue the same note; collapsing it leaves a one-line preview.'],
    },
    {
      id: 'delete', target: '[data-tour="review-delete"]',
      title: ['不需要的段落，用删除划掉', 'Strike through a paragraph to remove it'],
      body: ['删除按钮会给整段加上删除线，表达你的删减意见；再次点击可以恢复。原稿内容始终保留。', 'The delete button strikes through the whole paragraph to suggest removing it. Use it again to restore the paragraph. The source text is always preserved.'],
    },
    {
      id: 'verdict', target: '[data-tour="review-verdict"]',
      title: ['最后，给出一个审阅结论', 'Finish with a review decision'],
      body: ['审阅完后，明确选择原稿可用或需要修改。具体修改意见留在批注里，整体说明可以补充在右侧。', 'When you finish, choose whether the draft is ready or needs changes. Keep specific edits in comments and add any overall feedback on the right.'],
    },
    {
      id: 'submit', target: '[data-feedback-actions]',
      title: ['批注、删减和结论一起提交', 'Send the complete review'],
      body: ['准备好后，提交这次审阅。批注、删除标记、审阅结论和补充说明会一起发送，再由 Agent 根据反馈修改原稿。', 'Submit when ready. Your comments, removal marks, decision and overall feedback are sent together so the agent can revise the original.'],
    },
  ],
}

/** Only workbenches with an authored guide participate in first-use onboarding. */
export function getWorkbenchTour(type: string | null | undefined, locale: Locale): WorkbenchTour | null {
  if (type !== 'ramble' && type !== 'document_review') return null
  const zh = locale === 'zh-CN'
  const language = zh ? 0 : 1
  return {
    id: type,
    version: 1,
    steps: steps[type].map((step) => ({ ...step, title: step.title[language], body: step.body[language] })),
    labels: {
      dialog: zh ? '工作台使用引导' : 'Workbench guide',
      skip: zh ? '跳过引导' : 'Skip guide',
      back: zh ? '上一步' : 'Back',
      next: zh ? '下一步' : 'Next',
      done: zh ? '开始使用' : 'Get started',
      step: (index, total) => zh ? `第 ${index + 1} / ${total} 步` : `Step ${index + 1} of ${total}`,
    },
    replayLabel: zh ? '使用引导' : 'Show guide',
  }
}
