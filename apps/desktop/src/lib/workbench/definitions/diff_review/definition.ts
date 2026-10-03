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
    version: 2,
    steps: [
      {
        id: 'files', target: '[data-tour="diff-files"]',
        title: ['按文件浏览这次改动', 'Browse the changes by file'],
        body: ['左侧按目录显示文件，可以输入路径筛选。点击文件切换 diff，数字表示新增和删除的行数；已查看标记只记录本次浏览进度，不改变反馈或评审结论。', 'The sidebar groups files by directory and lets you filter paths. Select a file to switch diffs; counts show additions and deletions. Viewed only tracks this visit and does not change feedback or make a review decision.'],
      },
      {
        id: 'lines', target: '[data-diff-source]',
        title: ['区分修改前后，再选择行号', 'Choose a line before or after the change'],
        body: ['顶部可以切换统一与并排视图、开启自动换行。旧版行号对应修改前，新版对应修改后；红色表示删除，绿色表示新增。点击对应侧行号定位意见，Shift 点击选择同一个改动块、同一侧的行范围。', 'Switch between unified and split views or wrap lines in the toolbar. Old line numbers are before the change; new ones are after it. Red marks deletions and green marks additions. Select a side to anchor feedback, or Shift-click a range within one hunk and side.'],
      },
      {
        id: 'comment', target: '[data-tour="diff-selection"]',
        title: ['为选中范围或整个改动块批注', 'Comment on a range or a whole hunk'],
        body: ['行号旁的加号可以直接添加行内批注；也可以选中范围后点击添加批注或按 C。@@ 标题旁的按钮批注整个改动块；Shift 加上下方向键调整行范围。', 'Use the plus beside a line number to add an inline comment, or select a range and use Add comment or C. The button beside an @@ header comments on the entire hunk. Shift with the up or down arrow keys adjusts a range.'],
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
