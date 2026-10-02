import { t } from '../../i18n'
const zh: Record<string, string> = {
  'Terminal trial': '终端试用', 'Start terminal': '开始试用', 'Stop terminal': '停止终端',
  'Quote selected output': '引用选中输出', 'Full screen trial': '全屏试用',
  'Saved trial': '已保存的试用记录', 'Restart terminal': '重新启动终端',
  'Recorded trials': '已保存的试用次数',
  'The 16 trial limit was reached. Submit feedback to keep the recorded trials.': '已达到 16 次试用上限，请提交反馈以保存这些试用记录。',
  'Start the terminal to try the CLI.': '开始试用后，即可在终端中操作 CLI。',
  'Running': '运行中', 'Exited': '已退出', 'Stopped': '已停止', 'Connecting…': '正在连接…',
  'Trial output is saved with your feedback.': '试用记录将与反馈一起保存。',
  'Earlier output was truncated. The latest screen is preserved.': '较早的输出已截断，当前画面仍保留。',
  'Saved terminal screen': '保存的终端画面', 'Terminal transcript': '终端输出记录',
  'No trial recorded yet.': '还没有试用记录。', 'Terminal unavailable': '终端暂时不可用',
  'Retry connection': '重试连接', 'Exit code': '退出码',
  'The terminal session was lost. Saved output is preserved; you can submit it with your feedback.': '终端会话已丢失，保存的输出仍保留，可以随反馈一起提交。',
}
export function terminalText(locale: string, source: string): string { return locale === 'zh-CN' ? zh[source] ?? t('zh-CN', source) : source }
