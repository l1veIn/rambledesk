import type { Locale } from '../../preferences'

const zh: Record<string, string> = {
  'Media review': '音视频审阅', 'Full screen review': '全屏审阅', 'Audio': '音频', 'Video': '视频',
  'Loading media…': '正在加载媒体…', 'Retry loading': '重新加载',
  'This browser cannot play the media. Your draft is preserved. Ask for a supported media file in a new request.': '当前浏览器无法播放这份媒体。草稿已保留，请让 Agent 在新请求中提供受支持的媒体文件。',
  'The media duration differs from the request. Existing timestamps are preserved. Ask the Agent to check the material and create a corrected request.': '实际媒体时长与请求不符。已有时间位置保持不变，请让 Agent 核对材料并创建正确的新请求。',
  'The selected time is outside the playable media. The comment anchor is unchanged.': '选择的位置超出可播放媒体时长，批注位置保持不变。',
  'The original media attachment is unavailable.': '原始媒体附件不可用。', 'The media is no longer available.': '这份媒体已不可用。',
  'Media must be between 1 byte and 20 MiB.': '媒体文件大小必须在 1 字节至 20 MiB 之间。',
  'Seek media': '定位播放位置', 'Comments': '批注', 'Sort comments': '批注排序',
  'By time': '按时间', 'Newest first': '最新在前', 'Oldest first': '最早在前',
  'Comment at current time': '批注当前时间点', 'Mark in': '设为区间起点', 'Mark out': '设为区间终点',
  'Add range comment': '添加区间批注', 'Clear range': '清除区间', 'Range start': '区间起点', 'Range end': '区间终点',
  'Choose an end after the start.': '请选择晚于起点的终点。', 'Comment limit reached.': '批注已达到数量上限。',
  'No comments yet. You can also submit only overall feedback.': '还没有批注，也可以仅提交整体意见。',
  'Write your comment…': '填写意见…', 'Attachments': '附件', 'Comment': '批注意见', 'Speak comment': '口述批注意见',
  'Delete comment': '删除批注', 'Close comment': '收起批注',
  'Playback starts only when you choose to play. Comments pause playback.': '点击播放后才会开始，添加批注会暂停播放。',
  'The original media has changed. Select the comment again.': '原始媒体已变更，请重新选择批注。',
  'This media review draft is invalid.': '这份媒体审阅草稿无效。',
  'Write a comment or remove its empty marker before submitting.': '提交前请填写批注意见，或删除空白时间标记。',
}
export function mediaReviewText(locale: Locale, source: string): string { return locale === 'zh-CN' ? zh[source] ?? source : source }
