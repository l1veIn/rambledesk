import type { BrainstormData } from '../../../generated/feedback'
import type { WorkbenchExample } from '../contracts'

// In-memory preview of a server-resolved second round, not Agent request input.
const firstRequest = 'be715c00-83a1-4b8d-8d53-181208b7ca01'
const secondRequest = 'be715c00-83a1-4b8d-8d53-181208b7ca02'

function board(english: boolean): BrainstormData {
  const text = (zh: string, en: string) => english ? en : zh
  return {
    title: text('让新用户完成第一次有用的反馈', 'Help a new user give their first useful feedback'),
    source_version: secondRequest,
    board_id: firstRequest,
    round: 2,
    inherit_from_request_id: firstRequest,
    initial_board: {
      phase: 'organize',
      groups: [
        { id: 'first-action', title: text('先做一件小事', 'One small first action') },
        { id: 'confidence', title: text('知道反馈去了哪里', 'Know where feedback goes') },
      ],
      selected_card_ids: ['small-task', 'visible-return'],
      cards: [
        {
          id: 'small-task', title: text('从自己的截图开始，而不是填演示表单', 'Start with their own screenshot, not a demo form'),
          body: text('上一轮把“试一个示例”改成了“带一张自己的截图”。让人先圈出一个实际问题，再补一句解释。', 'In the previous round, “try a sample” became “bring your own screenshot.” Mark one real problem, then explain it in a sentence.'),
          group_id: 'first-action', source_card_ids: [], origin: 'agent', origin_request_id: firstRequest, parked: false,
        },
        {
          id: 'visible-return', title: text('提交后看见 Agent 如何接着做', 'Show how the agent continues after submission'),
          body: text('在原会话看到这次反馈已送达，并能回看圈画和解释。不要把“提交成功”当成 Agent 已完成修改。', 'Show delivery in the original conversation and keep the annotation available. Submission does not mean the agent has finished the change.'),
          group_id: 'confidence', source_card_ids: [], origin: 'agent', origin_request_id: firstRequest, parked: false,
        },
        {
          id: 'human-example', title: text('先告诉我怎样的一句意见就够了', 'Show me what one useful sentence looks like'),
          body: text('人补充的想法：在空白处放一条具体例子，例如“这个按钮看不出下一步会发生什么”。', 'A human-added idea: give a concrete example near an empty field, such as “This button does not tell me what happens next.”'),
          group_id: 'first-action', source_card_ids: [], origin: 'human', origin_request_id: firstRequest, parked: false,
        },
        {
          id: 'keep-draft', title: text('离开后还能回到没写完的意见', 'Let people return to unfinished feedback'),
          body: text('明确显示自动保存状态，重新打开时回到已有内容；先不要求用户了解资料库或文件路径。', 'Make draft status clear and restore the existing content on return, without asking users to understand storage paths.'),
          group_id: 'confidence', source_card_ids: [], origin: 'agent', origin_request_id: firstRequest, parked: false,
        },
        {
          id: 'full-tour', title: text('第一次展示所有工作台', 'Tour every workbench on the first visit'),
          body: text('上一轮已暂放：步骤太多，会打断第一次真实反馈。保留卡片，之后可以恢复讨论。', 'Parked in the previous round: too many steps interrupt the first real feedback. Keep the card available for a later discussion.'),
          group_id: null, source_card_ids: [], origin: 'agent', origin_request_id: firstRequest, parked: true,
        },
        {
          id: 'one-loop', title: text('把第一张截图和后续处理连成一次短体验', 'Connect the first screenshot to one short feedback loop'),
          body: text('基于上轮选中的两个方向追加：圈出问题、提交一句解释，再回原会话看 Agent 的下一步。请继续修改或暂放这个新提案。', 'A new proposal based on the two selected directions: mark a problem, submit one sentence, then see the next step in the original conversation. Edit or park this proposal as needed.'),
          group_id: 'first-action', source_card_ids: ['small-task', 'visible-return'], origin: 'agent', origin_request_id: secondRequest, parked: false,
        },
      ],
    },
  }
}

export const brainstormExamples: readonly WorkbenchExample[] = [
  {
    key: 'brainstorm', order: 13, title: '头脑风暴',
    markdown: '这是第二轮看板的内存演示。上轮改过的文字、人补充的卡片、分组、两个选择和暂放项均已保留；Agent 只追加了一张新卡。继续整理后，选择希望 Agent 下一步做什么，再统一提交。预览不调用 Agent，也不证明真实跨轮保存。',
    spec: { type: 'brainstorm', version: 1, data: board(false) },
  },
  {
    key: 'brainstorm-en', order: 13.1, title: 'Brainstorming',
    markdown: 'This in-memory second-round example keeps the previous edits, a human-added card, groups, two selections, and a parked idea. The agent has added one new card. Adjust the board, choose what the agent should do next, then submit. This preview does not call an agent or establish real cross-round persistence.',
    spec: { type: 'brainstorm', version: 1, data: board(true) },
  },
]
