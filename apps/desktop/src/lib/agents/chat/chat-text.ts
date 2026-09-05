import type { Locale } from '$lib/preferences'

const zh: Record<string, string> = {
  'You': '你', 'Agent': 'Agent', 'Thinking': '思考中', 'Reasoning': '思考过程',
  'Tool activity': '工具活动', 'Session status': '会话状态', 'Agent error': 'Agent 错误',
  'Pending': '待执行', 'Running': '执行中', 'Completed': '已完成', 'Failed': '失败',
  'No final result': '未收到最终结果', 'The tool did not report a final result before the turn stopped.': '轮次停止前，此工具未报告最终结果。',
  'Input': '输入', 'Output': '输出', 'Raw output': '原始输出', 'Locations': '位置',
  'Content truncated by the agent host': 'Agent 宿主已截断过长内容',
  'Quote in message': '引用到消息', 'Changed file': '文件变更', 'No line changes': '没有行变更',
  'Show more lines': '显示更多行', 'Showing': '当前显示', 'lines': '行',
  'Resource': '资源', 'Image output': '图片输出', 'Audio output': '音频输出',
  'Media preview unavailable': '无法预览此媒体', 'Terminal reference': '终端引用',
  'Unsupported content': '暂不支持的内容', 'Could not open the link': '无法打开链接',
  'Load earlier messages': '加载更早的消息', 'Loading earlier messages…': '正在加载更早的消息…',
  'Could not load earlier messages.': '无法加载更早的消息。',
  'Working…': '正在工作…', 'Finished working': '工作已结束', 'Worked for': '工作了',
  'Writing reply': '正在回复',
  'Turn cancelled': '本轮已取消', 'Turn interrupted': '本轮已中断', 'Work stopped': '工作已停止',
  'Copy reply': '复制回复', 'Copied': '已复制', 'Completed at': '完成于',
  'Could not copy the reply.': '无法复制回复。',
  'Earlier activity for this turn is not loaded.': '本轮更早的活动尚未加载。',
}

export function chatText(locale: Locale, text: string): string { return locale === 'zh-CN' ? (zh[text] ?? text) : text }
