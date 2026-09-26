import { t } from '../../i18n'

const zh: Record<string, string> = {
  'Document review': '文稿审阅', 'Original': '原稿',
  'Original text is preserved': '原稿保持不变', 'Select text to comment or suggest a rewrite.': '选中文字，添加批注或建议改写。',
  'Comment': '批注', 'Suggest rewrite': '建议改写', 'Paragraph comment': '整段批注',
  'Not marked': '未标记', 'Keep': '保留', 'Revise': '修改', 'Remove': '删除',
  'Mark paragraph': '标记段落', 'Comment on paragraph': '批注段落', 'Paragraph': '段落',
  'Delete paragraph': '删除段落', 'Restore paragraph': '恢复段落', 'Paragraphs deleted': '已删除段落',
  'Continue comment': '继续批注', 'Write your comment…': '填写批注意见…',
  'Attachments': '附件',
  'One comment per paragraph. Reopen it to add more feedback.': '每段保留一条批注，重新展开即可继续补充。',
  'Selection': '所选文字', 'Clear selection': '清除选区',
  'Your comment': '批注意见', 'Explain what to change and why…': '说明哪里需要调整，以及原因…',
  'Suggested wording': '建议措辞', 'An empty suggestion means delete this text.': '建议措辞留空，表示建议删除这段文字。',
  'Delete comment': '删除批注',
  'Collapse comment': '收起批注', 'Comments are saved with your feedback.': '批注随反馈自动保存。',
  'Delete this comment?': '删除这条批注？',
  'Cancel': '取消',
  'This comment and its contents will be deleted. This cannot be undone.': '这条批注及其中的内容将被删除，无法撤销。',
  'Review decision': '审阅结论', 'Ready as written': '原稿可用', 'Changes requested': '需要修改',
  'Choose a decision, then submit with your overall feedback.': '选择审阅结论，再与整体意见一起提交。',
  'Finish or remove empty comments.': '请补全空白批注，或删除它。',
  'Choose a review decision.': '请选择审阅结论。', 'Select text within one paragraph. Use paragraph comments for broader feedback.': '请选择同一段内的文字；较大范围的意见可使用整段批注。',
  'Selected passage': '所选片段', 'Whole paragraph': '整段', 'Review progress': '审阅进度',
  'Paragraphs marked': '已标记段落', 'comments': '条批注', 'Read only': '只读',
  'Comment limit reached.': '批注已达上限。', 'Needs attention': '待补全',
}

export function reviewText(locale: string, source: string): string { return locale === 'zh-CN' ? zh[source] ?? t('zh-CN', source) : source }
