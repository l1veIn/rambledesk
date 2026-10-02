import type { WebReviewData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validWebReviewInput } from './input'
import { readWebReviewState } from './state'
import { hasWebReviewInput, validateWebReviewState } from '../../web-review/reviewModel'
import { webReviewInputFields } from '../../web-review/inputFields'
import { examples } from './examples'
const message: WorkbenchDefinition['submissionMessage'] = (spec, state) => validateWebReviewState(spec.data as WebReviewData, state?.type === 'web_review' ? state : null)
export const webReviewDefinition: WorkbenchDefinition = {
  type: 'web_review', version: 1, accepts: validWebReviewInput, decodeState: readWebReviewState,
  hasInput: (spec, state) => hasWebReviewInput(spec.data as WebReviewData, state?.type === 'web_review' ? state : null),
  complete: (spec, state) => message(spec, state) === null, submissionMessage: message,
  layout: { padded: false, interactivePreview: true, expanded: true }, fields: webReviewInputFields,
  loadView: () => import('./View.svelte'), examples,
  guide: {
    version: 1,
    steps: [
      {
        id: 'toolbar', target: '[data-web-review-toolbar]',
        title: ['浏览页面，再选择要批注的元素', 'Browse, then select an element'],
        body: ['浏览模式可以正常操作页面。切换到选择元素后，点击页面中的元素开始批注；这里也能切换桌面或手机视口，以及打开全屏评审。', 'Browse mode lets you use the page normally. Switch to Select elements and click an element to start a comment. You can also switch desktop or mobile viewports and open a full screen review.'],
      },
      {
        id: 'connection', target: '[data-web-review-status]',
        title: ['先确认元素选择是否已连接', 'Check the element selection connection'],
        body: ['连接成功后才能选择元素。页面未接入评审桥接或无法加载时，可以单独打开页面，并在右侧用截图和整体说明反馈。', 'Element selection needs a connected review bridge. If the page has no bridge or cannot load, open it separately and share screenshots and overall feedback on the right.'],
      },
      {
        id: 'surface', target: '[data-web-review-surface]',
        title: ['意见跟着具体元素保存', 'Keep feedback attached to the element'],
        body: ['选择元素后，在底部批注卡写下意见，也可以使用语音和附件。批注会保留当时的页面地址、视口和元素信息，便于 Agent 定位。', 'After selecting an element, write your feedback in the comment card at the bottom, or use voice and attachments. The comment preserves its page URL, viewport and element details so the agent can locate it.'],
      },
      {
        id: 'comments', target: '[data-tour="web-review-comments"]',
        title: ['集中回看，再继续补充', 'Review your comments together'],
        body: ['从这里查看所有批注。点击一条可以回到对应页面和元素，继续编辑意见；页面后续发生变化时，已经捕获的信息仍会保留。', 'Open the comment list here. Select a comment to return to its page and element and continue editing. Captured context stays available even if the page later changes.'],
      },
      {
        id: 'submit', target: '[data-feedback-actions]',
        title: ['批注和整体反馈一起提交', 'Send comments and overall feedback'],
        body: ['检查批注，在右侧补充整体说明，准备好后提交反馈。引导只说明操作，不会替你选择元素或创建批注；标题旁可以随时重看。', 'Check your comments, add any overall feedback on the right, then submit when ready. This guide explains the controls without selecting elements or creating comments. Reopen it beside the title at any time.'],
      },
    ],
  },
}
