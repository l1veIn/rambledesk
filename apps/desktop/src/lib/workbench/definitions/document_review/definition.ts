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
  layout: { padded: true, interactivePreview: false, expanded: false, feedbackPlacement: 'drawer' }, fields: documentReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
  guide: {
    version: 2,
    steps: [
      {
        id: 'source', target: '[data-review-text]',
        title: ['原稿留在这里，意见写在旁边', 'Keep the original, add your feedback'],
        body: ['Markdown 原文保持不变。选区可跨标题、段落、列表、表格和代码，批注会定位到准确的原文。', 'The Markdown source stays unchanged. Select across headings, paragraphs, lists, tables and code; comments keep an exact source anchor.'],
      },
      {
        id: 'comment', target: '[data-tour="review-comment"]',
        title: ['每个选区都能添加独立批注', 'Add independent comments to each selection'],
        body: ['选中文字后，在原文旁添加批注或建议修改；语音位于编辑框工具栏。同处可有多条批注，点击编号或高亮继续编辑。', 'Select text to comment or suggest a change beside the source. Voice is in each editor toolbar. Reopen numbered notes or highlights, including several notes on the same passage.'],
      },
      {
        id: 'suggestion', target: '[data-review-text]',
        title: ['改写或删减，都作为建议保留', 'Keep rewrites and deletions as suggestions'],
        body: ['建议修改会显示差异，留空替换文本表示删除，理由可选。完成批注或按 Ctrl/⌘+Enter 后继续阅读；Esc 收起并保留草稿。原文始终不变。', 'Suggestions show a diff; an empty replacement means deletion, with an optional reason. Done or Ctrl/⌘+Enter finishes the note; Escape collapses it and keeps the draft. The original stays unchanged.'],
      },
      {
        id: 'verdict', target: '[data-tour="review-verdict"]',
        title: ['最后，给出一个审阅结论', 'Finish with a review decision'],
        body: ['审阅完后，选择原稿可用或需要修改。顶部“整体意见与提交”可补充说明；完成单条批注不会提交整轮反馈。', 'Choose whether the draft is ready or needs changes. Open Overall feedback and submit at the top for additional notes. Finishing one comment does not submit the review.'],
      },
      {
        id: 'submit', target: '[data-workbench-display-controls]',
        title: ['批注、删减和结论一起提交', 'Send the complete review'],
        body: ['准备好后，提交这次审阅。批注、改写建议、审阅结论和补充说明会一起发送，再由 Agent 根据反馈修改原文。', 'Submit when ready. Your comments, suggestions, decision and overall feedback are sent together so the agent can revise the original.'],
      },
    ],
  },
}
