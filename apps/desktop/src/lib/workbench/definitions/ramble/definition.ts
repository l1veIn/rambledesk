import type { WorkbenchDefinition } from '../contracts'
import { validRambleInput } from './input'
import { examples } from './examples'
export const rambleDefinition: WorkbenchDefinition = {
  type: 'ramble', version: 1, accepts: validRambleInput, decodeState: () => null,
  hasInput: () => false, complete: () => true, supportsApproval: true,
  layout: { padded: true, interactivePreview: false, expanded: false },
  loadView: () => import('./View.svelte'), examples,
  guide: {
    version: 1,
    steps: [
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
  },
}
