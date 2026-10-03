import type { TableReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validTableReviewInput } from './input'
import { readTableReviewState } from './state'
import { validateTableReviewState } from '../../table-review/reviewModel'
import { tableReviewInputFields } from '../../table-review/inputFields'
import { examples } from './examples'

const message: WorkbenchDefinition['submissionMessage'] = (spec, value) => {
  if (spec.type !== 'table_review' || spec.version !== 1 || !validTableReviewInput(spec.data)) return 'This table review is unavailable.'
  const state = value === null ? null : readTableReviewState(value)
  return value !== null && !state ? 'This table review draft is invalid.' : validateTableReviewState(spec.data as TableReviewData, state)
}
export const tableReviewDefinition: WorkbenchDefinition = {
  type: 'table_review', version: 1, accepts: validTableReviewInput, decodeState: readTableReviewState,
  hasInput: (_, state) => { const value = readTableReviewState(state); return !!value && value.changes.length + value.comments.length > 0 },
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: false, expanded: true }, fields: tableReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
  guide: { version: 1, steps: [
    { id: 'grid', target: '[data-tour="table-grid"]',
      title: ['按行列查看固定表格', 'Explore the fixed table'],
      body: ['列标题与行号帮助定位。点击单元格或用方向键移动，滚动可查看其他行列；本次请求的原始值保持固定。', 'Use column headings and row numbers to find a cell. Select it or move with arrow keys, and scroll to other rows and columns. Original values remain fixed for this request.'] },
    { id: 'suggestion', target: '[data-tour="table-selection"]',
      title: ['对单元格提出改值建议', 'Suggest a cell value'],
      body: ['选中单元格后按 Enter 或 F2 编辑建议，Enter 保存，Escape 取消。右侧对照原值和建议值，也可撤回建议；空字符串可以表示清空。', 'Press Enter or F2 on a selected cell to edit a suggestion. Enter saves and Escape cancels. Compare the original and suggested value on the right, or remove a suggestion. An empty string suggests clearing the cell.'] },
    { id: 'comments', target: '[data-tour="table-comments"]',
      title: ['给单元格留下批注', 'Comment on a cell'],
      body: ['添加批注或点击语音批注，使用共享输入框填写意见、录音或附加材料。批注列表保留单元格位置，点击一条即可回到对应位置。', 'Add a comment or choose Voice comment, then use the shared field to write, record, or attach material. Select a comment in the list to return to its cell.'] },
    { id: 'draft', target: '[data-tour="table-review-toolbar"]',
      title: ['全屏继续同一份草稿', 'Continue the same draft in full screen'],
      body: ['改值建议与批注保存到当前请求。全屏往返继续同一份草稿，重新打开可恢复；已提交的历史保留原表和意见，只允许查看。', 'Suggestions and comments belong to this request. Full screen continues the same draft, which can be restored later. Submitted history preserves the original table and feedback for viewing.'] },
    { id: 'submit', target: '[data-feedback-actions]',
      title: ['检查建议和批注，再统一提交', 'Check suggestions and comments, then send'],
      body: ['建议值需不同于原值，已有批注需填写意见；也可仅在反馈正文写整体说明。提交会把原始版本、行列身份、改值建议和批注交给 Agent，由 Agent 决定如何修改源文件。标题旁可以重看引导。', 'Suggestions must differ from original values, and existing comments need text. Overall feedback alone is also valid. Submission sends the source version, row and column identities, suggestions, and comments to the agent to handle source changes. Reopen the guide beside the title.'] },
  ] },
}
