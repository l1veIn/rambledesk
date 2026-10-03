import type { MediaReviewData } from '../../../generated/feedback'
import type { WorkbenchExample } from '../contracts'
import previewVideo from './previewVideo.json'

// A one-second PCM tone is a real player fixture, independent of mutable paths or remote URLs.
function tone() {
  const bytes = new Uint8Array(44 + 8000), header = new DataView(bytes.buffer)
  const write = (offset: number, value: string) => [...value].forEach((char, index) => bytes[offset + index] = char.charCodeAt(0))
  write(0, 'RIFF'); header.setUint32(4, bytes.length - 8, true); write(8, 'WAVE'); write(12, 'fmt ')
  header.setUint32(16, 16, true); header.setUint16(20, 1, true); header.setUint16(22, 1, true)
  header.setUint32(24, 8000, true); header.setUint32(28, 8000, true); header.setUint16(32, 1, true); header.setUint16(34, 8, true)
  write(36, 'data'); header.setUint32(40, 8000, true)
  for (let i = 0; i < 8000; i++) bytes[44 + i] = Math.round(128 + 30 * Math.sin(i * 2 * Math.PI * 440 / 8000))
  return btoa(String.fromCharCode(...bytes))
}
export const examples: readonly WorkbenchExample[] = [
  {
    key: 'media_review-audio', order: 11, title: '音频审阅', markdown: '主动播放短音频，在时间点或区间留下意见；整体意见写在右侧反馈正文。',
    spec: { type: 'media_review', version: 1, data: { title: '试听一秒提示音', source_version: 'tone-preview-v1',
      media_kind: 'audio', media_file_name: 'preview-tone.wav', duration_ms: 1000 } satisfies MediaReviewData },
    attachments: [{ id: 'preview-tone', name: 'preview-tone.wav', mimeType: 'audio/wav', contentsBase64: tone() }],
  },
  {
    key: 'media_review-video', order: 12, title: '视频审阅', markdown: '主动播放真实 WebM 测试视频，拖动时间轴定位，在时间点或区间批注。',
    spec: { type: 'media_review', version: 1, data: { title: '检查一秒运动图案', source_version: 'motion-preview-v1',
      media_kind: 'video', media_file_name: 'preview-motion.webm', duration_ms: 1000 } satisfies MediaReviewData },
    attachments: [{ id: 'preview-motion', name: 'preview-motion.webm', mimeType: 'video/webm', contentsBase64: previewVideo.contentsBase64 }],
  },
]
