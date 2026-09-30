import { t } from '../../i18n'

const zh: Record<string, string> = {
  'Web review': '网页评审', 'Browse': '浏览', 'Select elements': '选择元素', 'Refresh page': '刷新页面',
  'Desktop viewport': '桌面视口', 'Mobile viewport': '移动视口', 'Review comments': '评审记录',
  'Your comment': '批注意见', 'Explain what to change and why…': '说明哪里需要调整，以及原因…',
  'Delete comment': '删除批注', 'Collapse comment': '收起批注', 'Write your comment…': '填写批注意见…',
  'Attachments': '附件', 'Read only': '只读', 'No element comments yet.': '还没有元素批注。',
  'Choose Select elements, then click an element to comment.': '切换到选择元素，点击网页中的元素即可批注。',
  'Click a comment to return to its element.': '点击评审记录，可回到对应元素继续批注。',
  'Comments are saved with your overall feedback.': '元素批注与整体反馈一起保存、提交。',
  'Finish or remove empty comments.': '请补全空白批注，或删除它。', 'Comment limit reached.': '批注已达上限。',
  'Saved page context': '评审时的页面信息', 'Open page': '打开网页', 'Close review comments': '关闭评审记录',
  'Connecting…': '正在连接…', 'Element selection connected': '元素选择已连接', 'Preview only': '仅可预览',
  'Page loading…': '网页加载中…', 'Page unavailable': '网页不可用',
  'This page needs the RambleDesk review bridge to select elements.': '此网页需要接入 RambleDesk 评审桥接脚本，才能选择元素。',
  'You can browse the preview and add screenshots or overall feedback.': '可以浏览预览，在反馈正文中添加截图或整体意见。',
  'Open the page separately and add screenshots or overall feedback.': '可以另行打开网页，在反馈正文中添加截图或整体意见。',
  'Submitted comments keep their captured context.': '已提交的批注保留评审时的页面信息。',
  'Delete this comment?': '删除这条批注？', 'Cancel': '取消',
  'This comment and its contents will be deleted. This cannot be undone.': '这条批注及其中的内容将被删除，无法撤销。',
  'Recorded viewport': '记录的视口', 'Element': '元素', 'Page': '页面',
  'The saved element is unavailable. Its captured context is preserved.': '原元素暂时无法定位，评审时的页面信息已保留。',
}

export function webReviewText(locale: string, source: string): string { return locale === 'zh-CN' ? zh[source] ?? t('zh-CN', source) : source }
