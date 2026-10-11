import type { Locale } from '../../preferences'

const zh: Record<string, string> = {
  'Brainstorm': '头脑风暴', 'Round {round}': '第 {round} 轮', 'Your ideas carry forward. New agent ideas never replace your cards.': '你的编辑会延续到下一轮；Agent 的新想法不会覆盖已有卡片。',
  'Diverge': '发散', 'Organize': '整理', 'Focus': '聚焦', 'Board view': '看板视图',
  'Add idea': '添加想法', 'Add group': '添加分组', 'Ideas': '想法', 'Ungrouped': '未分组', 'Selected ideas': '选中想法', 'Other ideas': '其他想法',
  '{count} selected': '已选 {count} 张', 'Clear selection': '清除选择', 'Move selected…': '移动选中卡片…', 'Move to group': '移至分组',
  'Park selected': '暂放选中卡片', 'Parked ({count})': '暂放（{count}）', 'Parked ideas': '暂放的想法', 'No parked ideas.': '暂无暂放的想法。',
  'No ideas here yet. Add one or move ideas into this group.': '这里还没有想法，可以新增或将卡片移入此组。',
  'No ideas selected. Select cards to bring them into focus.': '尚未选中想法。选择卡片后会在这里集中展示。',
  'Idea title': '想法标题', 'Idea details': '想法详情', 'Speak idea': '语音填写想法', 'Group': '分组', 'Group title': '分组名称',
  'Remove group': '移除分组', 'Cards return to Ungrouped when a group is removed.': '移除分组后，卡片会回到“未分组”。',
  'Group moved earlier': '分组已前移', 'Move group earlier': '前移分组', 'Move group later': '后移分组',
  'Untitled idea': '未命名想法', 'Untitled group': '未命名分组', 'Select idea': '选择想法', 'Edit idea': '编辑想法',
  'You': '你', 'Agent': 'Agent', 'New this round': '本轮新增', 'Earlier round': '之前轮次', 'Based on': '来源想法',
  'Done': '完成', 'Esc to return to the board. Your draft stays.': '按 Esc 返回看板，保留已输入草稿。',
  'Park idea': '暂放想法', 'Restore idea': '恢复想法', 'Attachments': '附件',
  'What should the agent do next?': '下一步请 Agent 做什么？', 'Keep exploring': '继续发散', 'Expand selected': '展开选中想法',
  'Combine selected': '组合选中想法', 'Proceed with selected': '按选中方向继续',
  'Ask for fresh alternatives. Existing cards stay.': '请 Agent 提出新方向，已有卡片保留。',
  'Develop the selected ideas into more concrete options.': '请 Agent 将选中想法展开为更具体的方案。',
  'Create new ideas that combine the selected cards.': '请 Agent 将选中卡片组合为新想法。',
  'Continue the original task using the selected direction.': '请 Agent 按选中方向继续原任务。',
  'Instructions for the next step (optional)': '下一步补充说明（可选）', 'Speak next-step instructions': '语音填写下一步说明',
  'Review and send this round': '检查并提交本轮', 'This sets the next step. The agent starts after you submit the round.': '这里只设定下一步，提交本轮后才会交给 Agent。',
  'Choose what the agent should do next.': '请选择下一步请 Agent 做什么。',
  'Select at least one idea.': '请至少选择一个想法。', 'Select at least two ideas to combine.': '请至少选择两个想法进行组合。',
  'Give every idea and group a title before sending.': '提交前请填写每张卡片和分组的标题。',
  'This brainstorm draft is invalid.': '这份头脑风暴草稿无效。', 'Keep inherited cards. Park ideas you do not need.': '请保留继承的卡片；不需要的想法可以暂放。',
  'The origin of an idea cannot be changed.': '不能更改想法的来源。', 'Idea limit reached (200).': '已达到 200 张卡片上限。', 'Group limit reached (30).': '已达到 30 个分组上限。',
  'Full screen board': '全屏看板', 'Read-only board': '只读看板', 'Add details, constraints, or examples…': '补充细节、限制或例子…',
  'Remove new idea': '移除新想法', 'Remove this new idea?': '移除这个新想法？', 'Cancel': '取消',
  'This new card and its text will be removed. Inherited cards can only be parked.': '将移除这张新卡片及其文字。继承的卡片只能暂放。',
}
export function brainstormText(locale: Locale, source: string, values: Record<string, string | number> = {}) {
  return (locale === 'zh-CN' ? zh[source] ?? source : source).replace(/\{(\w+)\}/g, (match, key) => String(values[key] ?? match))
}
