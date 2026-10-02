import type { DiffReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validDiffReviewInput } from './input'
import { readDiffReviewState } from './state'
import { validateDiffReviewState } from '../../diff-review/reviewModel'
import { diffReviewInputFields } from '../../diff-review/inputFields'
import { examples } from './examples'

const message: WorkbenchDefinition['submissionMessage'] = (spec, value) => {
  if (spec.type !== 'diff_review' || spec.version !== 1 || !validDiffReviewInput(spec.data)) return 'This diff review is unavailable.'
  const state = value === null ? null : readDiffReviewState(value)
  return value !== null && !state ? 'This diff review draft is invalid.' : validateDiffReviewState(spec.data as DiffReviewData, state)
}
export const diffReviewDefinition: WorkbenchDefinition = {
  type: 'diff_review', version: 1, accepts: validDiffReviewInput, decodeState: readDiffReviewState,
  hasInput: (_, state) => (readDiffReviewState(state)?.comments.length ?? 0) > 0,
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: false, expanded: true }, fields: diffReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
  guide: {
    version: 1,
    steps: [
      {
        id: 'files', target: '[data-tour="diff-files"]',
        title: ['按文件浏览这次改动', 'Browse the changes by file'],
        body: ['点击文件名切换 diff，旁边的数字表示新增和删除的行数。本次请求的差异内容保持固定，文件路径用于定位评审意见。', 'Select a file to switch diffs. The counts beside its name show added and deleted lines. The request preserves a fixed diff, and file paths locate your review comments.'],
      },
      {
        id: 'lines', target: '[data-diff-source]',
        title: ['区分修改前后，再选择行号', 'Choose a line before or after the change'],
        body: ['左列行号对应修改前，右列对应修改后；红色表示删除，绿色表示新增。点击对应侧的行号定位意见，按住 Shift 再点击另一行可选择同一个改动块、同一侧的行范围。', 'The left line-number column is before the change, and the right is after it. Red marks deletions and green marks additions. Click a line number to anchor feedback. Shift-click another line to select a range within the same hunk and side.'],
      },
      {
        id: 'comment', target: '[data-tour="diff-selection"]',
        title: ['为选中范围或整个改动块批注', 'Comment on a range or a whole hunk'],
        body: ['选中行后点击添加批注，或按 C 开始写意见。针对整个改动块，可以使用 @@ 标题旁的批注按钮；无需先选择行。Shift 加上下方向键也能调整行范围。', 'After selecting a line or range, use Add comment or press C to write feedback. To comment on a whole hunk, use the comment button beside its @@ header without selecting lines. Shift with the up or down arrow keys also adjusts a range.'],
      },
      {
        id: 'comments', target: '[data-tour="diff-comments"]',
        title: ['从批注回到对应改动', 'Return to a change from its comment'],
        body: ['批注集中在这里。点击一条会切换到对应文件和位置，可以继续填写、使用语音和附件，或删除批注。全屏评审继续同一份草稿。', 'Your comments are collected here. Select one to return to its file and location, continue writing, use voice and attachments, or delete it. Full screen review continues the same draft.'],
      },
      {
        id: 'submit', target: '[data-feedback-actions]',
        title: ['检查位置和意见，再提交', 'Check locations and feedback, then send'],
        body: ['提交前填写已有批注的意见，也可以在右侧补充整体说明；仅提交整体说明同样可以。工作台会把固定 diff 的位置和意见发送给 Agent，不要求选择审批结论。标题旁可以重看引导。', 'Fill in existing comments before submitting, and add overall feedback on the right if needed. Overall feedback alone is also valid. The workbench sends locations in the fixed diff and your comments to the agent without requiring an approval decision. Reopen this guide beside the title.'],
      },
    ],
  },
}
