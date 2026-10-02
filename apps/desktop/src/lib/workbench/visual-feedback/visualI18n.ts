import { t } from '../../i18n'
const zh: Record<string, string> = {
  'Visual feedback': '视觉反馈', 'Select': '选择', 'Pen': '自由笔', 'Arrow': '箭头', 'Rectangle': '矩形', 'Text': '文字',
  'Color': '颜色', 'Line width': '线宽', 'Text size': '字号', 'Undo': '撤销', 'Redo': '重做', 'Delete annotation': '删除标注',
  'Zoom out': '缩小', 'Zoom in': '放大', 'Fit canvas': '适合窗口', 'Full screen canvas': '全屏画布',
  'Draw on the canvas, or select an annotation to add a comment.': '在画布上简单描绘，或选择标注补充意见。',
  'Click the canvas to place text.': '点击画布放置文字。', 'Text on canvas': '画布文字', 'Your comment': '标注意见',
  'Speak annotation text': '为画布文字录音', 'Speak comment': '为标注意见录音', 'Annotations': '标注',
  'No annotations yet.': '还没有标注。', 'Original image unavailable': '无法读取原图', 'Retry': '重试',
  'Loading original image…': '正在读取原图…', 'Canvas': '画布', 'Untitled text': '文字',
  'The annotation limit was reached. Delete a mark to continue.': '已达到标注上限，请删除部分标注后继续。',
  'The original stays unchanged. A PNG with your annotations is submitted with feedback.': '原图保持不变，提交时会生成包含标注的 PNG。',
  'Saved visual feedback': '已保存的视觉反馈',
}
export const visualText = (locale: string, source: string) => locale === 'zh-CN' ? zh[source] ?? t('zh-CN', source) : source
