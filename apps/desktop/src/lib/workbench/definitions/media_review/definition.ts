import type { MediaReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validMediaReviewInput } from './input'
import { readMediaReviewState } from './state'
import { validateMediaReviewState } from '../../media-review/mediaModel'
import { mediaReviewInputFields } from '../../media-review/inputFields'
import { createMediaReviewController } from '../../media-review/mediaController'
import { examples } from './examples'

const message: WorkbenchDefinition['submissionMessage'] = (spec, value) => {
  if (spec.type !== 'media_review' || spec.version !== 1 || !validMediaReviewInput(spec.data)) return 'This media review is unavailable.'
  const state = value === null ? null : readMediaReviewState(value)
  return value !== null && !state ? 'This media review draft is invalid.' : validateMediaReviewState(spec.data as MediaReviewData, state)
}
export const mediaReviewDefinition: WorkbenchDefinition = {
  type: 'media_review', version: 1, accepts: validMediaReviewInput, decodeState: readMediaReviewState,
  hasInput: (_, state) => (readMediaReviewState(state)?.comments.length ?? 0) > 0,
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: false, expanded: true }, fields: mediaReviewInputFields,
  createController: createMediaReviewController, loadView: () => import('./View.svelte'), examples,
  guide: {
    version: 1,
    steps: [
      {
        id: 'player', target: '[data-tour="media-player"]',
        title: ['主动播放原始音视频', 'Play the original media'],
        body: ['播放器使用这次请求保存的原始材料，不会自动播放。可以调节音量、播放或暂停；不支持的格式或时长明显不符会显示提示，并保留草稿。全屏评审使用同一份材料和意见。', 'The player uses the original attachment saved with this request and never starts automatically. Play, pause, or adjust volume. Unsupported media or a substantial duration mismatch shows a message while keeping your draft. Full screen review uses the same source and feedback.'],
      },
      {
        id: 'timeline', target: '[data-tour="media-timeline"]',
        title: ['沿时间轴定位意见', 'Locate feedback on the timeline'],
        body: ['拖动时间轴定位；已有批注以时间标记显示。点击标记会暂停播放并回到对应位置。时间标记沿用 Agent 提供的固定时长，不会随播放器重写。', 'Seek with the timeline. Existing comments appear as time markers; selecting one pauses playback and returns to its location. Anchors use the fixed duration supplied by the agent and are never rewritten by the player.'],
      },
      {
        id: 'selection', target: '[data-tour="media-selection"]',
        title: ['批注时间点或一个区间', 'Comment on a point or a range'],
        body: ['可以在当前时间点添加批注，或标记开始、结束后添加区间批注。时间轴获得焦点时按 I 和 O 设置两端；也可以拖动区间滑杆调整。开始批注会暂停播放。', 'Add a comment at the current time, or mark an in and out point to comment on a range. With the timeline focused, I and O set its ends; the range sliders also adjust them. Starting a comment pauses playback.'],
      },
      {
        id: 'comments', target: '[data-tour="media-comments"]',
        title: ['排序、定位和补充批注', 'Sort, locate, and develop comments'],
        body: ['按时间、最新或最早排列批注，点击一条回到对应时间。批注正文使用共享编辑器，可以输入、语音填写或添加附件；这些动作不会改变时间位置。已完成的反馈可回看和播放，不能修改。', 'Sort by time, newest, or oldest, and select a comment to seek to its anchor. Comment bodies use the shared editor for typing, voice, and attachments without changing the location. Completed feedback can be viewed and played without editing.'],
      },
      {
        id: 'submit', target: '[data-feedback-actions]',
        title: ['填写意见后提交', 'Send your feedback'],
        body: ['提交前填写每条已有批注；右侧可补充整体说明，也可以仅提交整体说明。反馈返回时间点、区间和意见，不需要选择审批结论。标题旁可以重新查看引导。', 'Fill in every existing comment before sending. Add overall notes on the right, or send overall notes alone. Feedback returns time points, ranges, and comments without requiring an approval decision. Reopen this guide beside the title.'],
      },
    ],
  },
}
