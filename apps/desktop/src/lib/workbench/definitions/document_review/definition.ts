import type { DocumentReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validDocumentReviewInput } from './input'
import { readDocumentReviewState } from './state'
import { hasReviewInput, validateReviewState } from '../../document-review/reviewModel'
import { documentReviewInputFields } from '../../document-review/inputFields'
import { examples } from './examples'
const message: WorkbenchDefinition['submissionMessage'] = (spec, state) => validateReviewState(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null)
export const documentReviewDefinition: WorkbenchDefinition = {
  type: 'document_review', version: 1, accepts: validDocumentReviewInput, decodeState: readDocumentReviewState,
  hasInput: (spec, state) => hasReviewInput(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: true, interactivePreview: false, expanded: false }, fields: documentReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
  guide: {
    version: 1,
    steps: [
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
  },
}
