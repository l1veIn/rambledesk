import { t } from '../../i18n'

const zh: Record<string, string> = {
  'Change the wording or add a reason before finishing.': '请修改措辞或填写理由，再完成这条建议。',
  'Suggest change': '建议修改', 'Reason (optional)': '理由（可选）', 'Change preview': '修改预览',
  'No wording change yet.': '尚未修改措辞。', 'Done': '完成批注', 'Ctrl/⌘ + Enter': 'Ctrl/⌘ + Enter',
  'Draft only · Esc to continue reading': '仅保存草稿 · Esc 继续阅读',
  'Select text, leave a note, and keep reading.': '选中文字，留下意见，继续阅读。',
  'Notes stay beside the text. Select a highlight or a numbered note to reopen it.': '批注留在原文旁。点击高亮或编号，可继续编辑。',
  'Markdown document review': 'Markdown 文档审阅', 'Whole document': '全文', 'Comments': '批注',
  'Comment on document': '全文批注', 'Speak comment on document': '语音批注全文',
  'Suggest deletion': '建议删除', 'Locate in original': '定位原文',
  'Select text across paragraphs to comment or suggest a rewrite. Each selection can have its own comments.': '可跨段选中文字，添加批注或建议改写；每次选区都可以添加独立批注。',
  'Some content is shown as Markdown source to preserve exact selection anchors.': '部分内容以 Markdown 原文显示，确保选区位置准确。',
  'Select text without image placeholders. You can also comment on the whole document.': '请选择不含图片占位的文字，也可使用全文批注。',
  'Select a shorter passage to suggest wording (8,000 characters maximum).': '请缩小选区后建议改写（最多 8,000 字符）。',
  'Original': '原文', 'Original text is preserved': '原文保持不变',
  'Comment': '批注', 'Suggest rewrite': '建议改写', 'Speak comment': '语音批注',
  'Write your comment…': '填写批注意见…', 'Attachments': '附件', 'Clear selection': '清除选区',
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
  'Choose a review decision.': '请选择审阅结论。', 'Selected passage': '所选片段',
  'Comment limit reached.': '批注已达上限。',
}

export function reviewText(locale: string, source: string): string { return locale === 'zh-CN' ? zh[source] ?? t('zh-CN', source) : source }
