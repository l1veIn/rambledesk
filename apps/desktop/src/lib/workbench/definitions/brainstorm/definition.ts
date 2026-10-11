import type { WorkbenchDefinition } from '../contracts'
import { validBrainstormInput } from './input'
import { readBrainstormState } from './state'
import { hasBrainstormInput, validateBrainstormState } from '../../brainstorm/brainstormModel'
import { brainstormInputFields } from '../../brainstorm/inputFields'
import { brainstormExamples } from './examples'

const message: WorkbenchDefinition['submissionMessage'] = (spec, value) => {
  if (spec.type !== 'brainstorm' || spec.version !== 1 || !validBrainstormInput(spec.data)) return 'This brainstorm round is unavailable.'
  const state = readBrainstormState(value)
  return value !== null && !state ? 'This brainstorm draft is invalid.' : validateBrainstormState(spec.data, state)
}
export const brainstormDefinition: WorkbenchDefinition = {
  type: 'brainstorm', version: 1, accepts: validBrainstormInput, decodeState: readBrainstormState,
  hasInput: (spec, value) => validBrainstormInput(spec.data) && hasBrainstormInput(spec.data, readBrainstormState(value)),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: false, expanded: true, feedbackPlacement: 'drawer' },
  fields: brainstormInputFields, loadView: () => import('./View.svelte'), examples: brainstormExamples,
  guide: {
    version: 1,
    steps: [
      { id: 'phase', target: '[data-tour="brainstorm-phase"]', title: ['同一看板，随时切换思考方式', 'One board, different ways to think'],
        body: ['发散、整理和聚焦可以自由切换，不是必须依次完成的步骤。卡片、分组和选择始终保留。', 'Switch freely between Diverge, Organize, and Focus. These are views, not required steps; your cards, groups, and selections stay.'] },
      { id: 'cards', target: '[data-tour="brainstorm-cards"]', title: ['直接修改，或加上你的想法', 'Edit ideas or add your own'],
        body: ['打开卡片编辑标题和详情，可以语音填写并添加附件。输入会进入草稿，关闭卡片不会提交本轮；Agent 的新提案作为新卡片出现。', 'Open a card to edit its title and details, with voice and attachments. Closing the editor keeps the draft without submitting the round. New agent proposals appear as new cards.'] },
      { id: 'selection', target: '[data-tour="brainstorm-selection"]', title: ['分组与多选由你决定', 'Choose and group ideas yourself'],
        body: ['选择一张或多张卡片，可一起移动分组或暂放。整理视图支持新增、改名、排列和移除分组；移除组不会删除卡片。', 'Select one or more cards to move them to a group or park them. Organize lets you add, rename, reorder, and remove groups without deleting their cards.'] },
      { id: 'parked', target: '[data-tour="brainstorm-parked"]', title: ['不需要的想法先暂放', 'Keep unused ideas parked'],
        body: ['暂放会取消选择，但保留卡片文字和来源。之后可以恢复；下一轮也会保留这些决定，不会重新塞回候选列表。', 'Parking clears selection but preserves the text and origin. Restore an idea later; the next round keeps your parked choices.'] },
      { id: 'next', target: '[data-tour="brainstorm-next"]', title: ['明确下一步，再提交本轮', 'Choose the next step, then send'],
        body: ['可以继续发散、展开一个方向、组合多个方向，或按选中方向继续原任务。按钮只设定意图；“检查并提交本轮”会打开整体意见区，最终提交才交给 Agent。', 'Explore alternatives, expand an idea, combine ideas, or continue the original task with the selected direction. Choosing an action sets intent; Review and send opens overall feedback, and only submission hands the round to the agent.'] },
    ],
  },
}
