const messages: Record<string, string> = {
  'Table review': '表格审阅', 'Full screen review': '全屏审阅', 'Original value': '原值', 'Suggested value': '建议值',
  'Suggest value': '建议改值', 'Remove suggestion': '撤回建议', 'Save suggestion': '保存建议', 'Cancel editing': '取消编辑',
  'Comments': '批注', 'Add comment': '添加批注', 'Voice comment': '语音批注', 'Voice suggestion': '语音建议',
  'Comment': '意见', 'Record comment': '录制批注', 'Record suggestion': '录制建议', 'Delete comment': '删除批注',
  'No comments yet. Select a cell to leave feedback.': '还没有批注。选择单元格留下意见。',
  'No suggestion for this cell.': '此单元格尚无改值建议。', 'Empty': '空值', 'Draft comment': '批注草稿',
  'Write your feedback…': '填写你的意见…', 'Suggestions': '改值建议', 'Rows': '行', 'Columns': '列',
  'Arrow keys move · Enter/F2 edit · Escape cancels': '方向键移动 · Enter/F2 编辑 · Escape 取消',
  'A suggestion no longer matches the original table.': '一条改值建议已无法对应原始表格。',
  'A comment no longer matches the original table.': '一条批注已无法对应原始表格。',
  'Change the suggested value or remove the unchanged suggestion before submitting.': '提交前修改建议值，或撤回与原值相同的建议。',
  'Write a comment or remove its empty draft before submitting.': '提交前填写已有批注的意见，或删除空白草稿。',
}
export const tableReviewText = (locale: string, source: string) => locale === 'zh-CN' ? messages[source] ?? source : source
