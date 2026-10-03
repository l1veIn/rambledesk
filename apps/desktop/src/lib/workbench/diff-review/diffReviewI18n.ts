import type { Locale } from '../../preferences'

const chinese: Record<string, string> = {
  'Diff review': '差异评审',
  'The original diff is preserved. Add comments; write overall feedback on the right.': '差异原文保持不变。针对改动添加批注，整体意见写在右侧。',
  'Full screen review': '全屏评审',
  'Comment on {side} line {line}': '批注{side}第 {line} 行',
  Old: '旧版', New: '新版',
  '{count} changed files': '{count} 个修改文件',
  '{viewed}/{total} viewed': '已查看 {viewed}/{total}',
  'Diff layout': '差异布局', Unified: '统一视图', Split: '并排视图',
  'Wrap lines': '自动换行', 'Filter files': '筛选文件',
  'No matching files.': '没有匹配的文件。', Viewed: '已查看', Before: '修改前', After: '修改后',
  Files: '文件', Comments: '批注', Attachments: '附件',
  'No comments yet. You can also submit only overall feedback.': '还没有批注。也可以只在右侧提交整体反馈。',
  'Choose an old or new line number. Shift-click or Shift + ↑/↓ selects a range within one hunk and side. Press C to comment.': '选择旧版或新版行号。Shift 点击或 Shift + ↑/↓ 可选择同一分块、同一版本的范围，按 C 添加批注。',
  'Old line {line}': '旧版第 {line} 行',
  'New line {line}': '新版第 {line} 行',
  'Comment on hunk {number}': '批注第 {number} 个分块',
  'Add comment': '添加批注',
  'Clear selection': '清除选择',
  'Previous lines': '上一页差异',
  'Next lines': '下一页差异',
  'Showing {first}–{last} of {total}': '显示第 {first}–{last} 条，共 {total} 条',
  'Write your comment…': '填写批注…',
  'Comment': '批注内容',
  'Speak comment': '为批注录音',
  'Delete comment': '删除批注',
  'Close comment': '收起批注',
  'Comment limit reached.': '批注数量已达到上限。',
  'A comment no longer matches the original diff.': '有批注无法对应差异原文，请检查其锚点。',
  'Write a comment or remove its empty draft before submitting.': '请填写批注，或删除空白批注草稿后再提交。',
}
export function diffReviewText(locale: Locale, source: string, values: Record<string, string | number> = {}): string {
  const text = locale === 'zh-CN' ? chinese[source] ?? source : source
  return text.replace(/\{(\w+)\}/g, (match, key: string) => key in values ? String(values[key]) : match)
}
