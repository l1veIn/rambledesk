import type { Locale } from '$lib/preferences'

const zh: Record<string, string> = {
  'Message the agent…': '向 Agent 发送消息…',
  'Message the agent': '向 Agent 发送消息',
  'Send message': '发送消息', 'Cancel current turn': '取消当前轮次',
  'Enter to send · Shift+Enter for a new line': 'Enter 发送 · Shift+Enter 换行',
  'Ctrl/⌘+Enter to send · Enter for a new line': 'Ctrl/⌘+Enter 发送 · Enter 换行',
  'Draft saved while the agent works': 'Agent 工作时可以继续编辑草稿',
  'Could not send. Your draft is preserved.': '发送失败，草稿已保留。',
  'Could not cancel the current turn.': '未能取消当前轮次。',
  'Could not add the attachment.': '未能添加附件。',
  'Attachments are not supported by this session.': '此会话暂不支持附件。',
  'Add attachment': '添加附件', 'Remove attachment': '移除附件',
  'Mention a file': '提及文件', 'Files': '文件', 'Searching…': '正在搜索…',
  'No matching files': '没有匹配的文件', 'File search failed': '文件搜索失败',
}

export function composerText(locale: Locale, text: string): string {
  return locale === 'zh-CN' ? (zh[text] ?? text) : text
}
