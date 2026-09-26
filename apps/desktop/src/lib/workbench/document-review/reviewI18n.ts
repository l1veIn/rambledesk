import { t } from '../../i18n'

const zh: Record<string, string> = {
  'Document review': '文稿审阅', 'Original': '原稿', 'All comments': '全部批注',
  'Original text is preserved': '原稿保持不变', 'Select text to comment or suggest a rewrite.': '选中文字，添加批注或建议改写。',
  'Comment': '批注', 'Suggest rewrite': '建议改写', 'Paragraph comment': '整段批注',
  'Not marked': '未标记', 'Keep': '保留', 'Revise': '修改', 'Remove': '删除',
  'Mark paragraph': '标记段落', 'Comment on paragraph': '批注段落', 'Paragraph': '段落',
  'Selection': '所选文字', 'Clear selection': '清除选区', 'Open': '待处理', 'Resolved': '已解决', 'All': '全部',
  'Your comment': '批注意见', 'Explain what to change and why…': '说明哪里需要调整，以及原因…',
  'Suggested wording': '建议措辞', 'An empty suggestion means delete this text.': '建议措辞留空，表示建议删除这段文字。',
  'Delete comment': '删除批注', 'Resolve': '标为已解决', 'Reopen': '重新打开', 'Locate in original': '定位原文',
  'Close comment': '关闭批注', 'Comments are saved with your feedback.': '批注随反馈自动保存。',
  'Review decision': '审阅结论', 'Ready as written': '原稿可用', 'Changes requested': '需要修改',
  'Choose a decision, then submit with your overall feedback.': '选择审阅结论，再与整体意见一起提交。',
  'No comments yet': '还没有批注', 'Select a passage or use a paragraph’s comment button to begin.': '选中一段文字，或使用段落旁的批注按钮开始审阅。',
  'No comments in this view': '此筛选下没有批注', 'View original': '查看原稿',
  'Finish or remove empty comments.': '请补全空白批注，或删除它。',
  'Choose a review decision.': '请选择审阅结论。', 'Select text within one paragraph. Use paragraph comments for broader feedback.': '请选择同一段内的文字；较大范围的意见可使用整段批注。',
  'Selected passage': '所选片段', 'Whole paragraph': '整段', 'Review progress': '审阅进度',
  'Paragraphs marked': '已标记段落', 'comments': '条批注', 'Read only': '只读',
  'Comment limit reached.': '批注已达上限。', 'Needs attention': '待补全',
}

export function reviewText(locale: string, source: string): string { return locale === 'zh-CN' ? zh[source] ?? t('zh-CN', source) : source }
