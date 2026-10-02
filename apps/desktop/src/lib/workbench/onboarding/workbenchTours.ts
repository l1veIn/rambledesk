import type { Locale } from '../../preferences'

export type WorkbenchTourId = 'ramble' | 'document_review' | 'web_review' | 'terminal'

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
  web_review: [
    {
      id: 'toolbar', target: '[data-web-review-toolbar]',
      title: ['浏览页面，再选择要批注的元素', 'Browse, then select an element'],
      body: ['浏览模式可以正常操作页面。切换到选择元素后，点击页面中的元素开始批注；这里也能切换桌面或手机视口，以及打开全屏评审。', 'Browse mode lets you use the page normally. Switch to Select elements and click an element to start a comment. You can also switch desktop or mobile viewports and open a full screen review.'],
    },
    {
      id: 'connection', target: '[data-web-review-status]',
      title: ['先确认元素选择是否已连接', 'Check the element selection connection'],
      body: ['连接成功后才能选择元素。页面未接入评审桥接或无法加载时，可以单独打开页面，并在右侧用截图和整体说明反馈。', 'Element selection needs a connected review bridge. If the page has no bridge or cannot load, open it separately and share screenshots and overall feedback on the right.'],
    },
    {
      id: 'surface', target: '[data-web-review-surface]',
      title: ['意见跟着具体元素保存', 'Keep feedback attached to the element'],
      body: ['选择元素后，在底部批注卡写下意见，也可以使用语音和附件。批注会保留当时的页面地址、视口和元素信息，便于 Agent 定位。', 'After selecting an element, write your feedback in the comment card at the bottom, or use voice and attachments. The comment preserves its page URL, viewport and element details so the agent can locate it.'],
    },
    {
      id: 'comments', target: '[data-tour="web-review-comments"]',
      title: ['集中回看，再继续补充', 'Review your comments together'],
      body: ['从这里查看所有批注。点击一条可以回到对应页面和元素，继续编辑意见；页面后续发生变化时，已经捕获的信息仍会保留。', 'Open the comment list here. Select a comment to return to its page and element and continue editing. Captured context stays available even if the page later changes.'],
    },
    {
      id: 'submit', target: '[data-feedback-actions]',
      title: ['批注和整体反馈一起提交', 'Send comments and overall feedback'],
      body: ['检查批注，在右侧补充整体说明，准备好后提交反馈。引导只说明操作，不会替你选择元素或创建批注；标题旁可以随时重看。', 'Check your comments, add any overall feedback on the right, then submit when ready. This guide explains the controls without selecting elements or creating comments. Reopen it beside the title at any time.'],
    },
  ],
  terminal: [
    {
      id: 'directory', target: '[data-tour="terminal-directory"]',
      title: ['确认这次试用的工作目录', 'Check the trial directory'],
      body: ['终端会在这里显示的目录中运行。先阅读请求中的体验说明和命令，再开始这次 CLI 试用。', 'The terminal runs in the directory shown here. Read the request instructions and suggested commands before starting the CLI trial.'],
    },
    {
      id: 'toolbar', target: '[data-terminal-toolbar]',
      title: ['主动启动，按需停止或重开', 'Start, stop or restart when ready'],
      body: ['点击开始终端才会创建试用会话。停止或退出后可以重新开始，各轮记录都会保留；也能打开全屏试用，或通过样式按钮选择预置外观。', 'Start terminal creates the trial session. After stopping or exiting, restart to begin another trial while keeping earlier records. You can also open a full screen trial or choose a preset terminal appearance.'],
    },
    {
      id: 'surface', target: '[data-terminal-surface]',
      title: ['在真实终端里体验 CLI', 'Try the CLI in a real terminal'],
      body: ['启动后在这里输入命令，检查帮助、交互提示和错误信息，也可以试用全屏文字界面。普通工作台和全屏页签继续同一个终端会话。', 'Once started, enter commands here to check help, prompts and errors, or try a full screen text interface. The normal workbench and full screen tab continue the same terminal session.'],
    },
    {
      id: 'quote', target: '[data-tour="terminal-quote"]',
      title: ['选中输出，把证据引用到反馈中', 'Quote selected output into your feedback'],
      body: ['在终端中选中相关输出，再点击引用选中输出，把这段内容放到右侧正文，接着说明观察或问题。没有选中内容时，这个按钮会暂时禁用。', 'Select relevant terminal output, then use Quote selected output to add it to the feedback on the right. Explain your observation or issue alongside it. The button is disabled until output is selected.'],
    },
    {
      id: 'submit', target: '[data-feedback-actions]',
      title: ['说明体验结果，连同记录提交', 'Send your experience and trial records'],
      body: ['在右侧写下体验结果并提交。提交会收尾仍在运行的终端，并保存试用输出和最后的画面；引导本身不会启动终端或执行命令。', 'Write your experience on the right and submit when ready. Submission finishes any running terminal and preserves trial output and the final screen. This guide never starts a terminal or runs commands.'],
    },
  ],
}

/** Only workbenches with an authored guide participate in first-use onboarding. */
export function getWorkbenchTour(type: string | null | undefined, locale: Locale): WorkbenchTour | null {
  if (type !== 'ramble' && type !== 'document_review' && type !== 'web_review' && type !== 'terminal') return null
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
