import type { VisualFeedbackData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { visualFeedbackState, validVisualState } from '../../visual-feedback/visualModel'
import { createVisualWorkbenchController } from '../../visual-feedback/visualController'
import { visualAnnotationField } from './fields'
import { validVisualFeedbackInput } from './input'
import { text } from '../validation'
import backgroundPng from '../../../../../../../playground/workbenches/materials/visual-background.png?inline'

const complete: WorkbenchDefinition['complete'] = (spec, state) => validVisualFeedbackInput(spec.data)
  && (state === null || (state.type === 'visual_feedback' && validVisualState(spec.data as VisualFeedbackData, state)
    && state.annotations.every((mark) => mark.kind !== 'text' || text(mark.text, 2000))))
export const visualFeedbackDefinition: WorkbenchDefinition = {
  type: 'visual_feedback', version: 1, accepts: validVisualFeedbackInput, decodeState: visualFeedbackState,
  hasInput: (spec, state) => validVisualFeedbackInput(spec.data) && state?.type === 'visual_feedback'
    && validVisualState(spec.data as VisualFeedbackData, state) && state.annotations.length > 0,
  complete, submissionMessage: (spec, state) => complete(spec, state) ? null : '请完成或删除无效的标注后提交。',
  layout: { padded: true, interactivePreview: false, expanded: true }, fields: [visualAnnotationField],
  createController: createVisualWorkbenchController, loadView: () => import('./View.svelte'),
  examples: [{ order: 8, title: '视觉反馈 · 简单描绘', markdown: '用自由笔、箭头、框和文字描绘一个简单的页面布局，选择标注后补充意见。原生空白画布也可以作为视觉指引。',
    spec: { type: 'visual_feedback', version: 1, data: { title: '画出你期望的页面布局', source_version: 'layout-1',
      width: 960, height: 600, image_file_name: null } satisfies VisualFeedbackData } },
    { key: 'visual_feedback-image', order: 8.1, title: '视觉反馈 · 原图标注', markdown: '在固定的原图上画箭头、框或文字并补充意见。提交后检查包含原图与标注的 PNG，以及只读历史恢复。',
      spec: { type: 'visual_feedback', version: 1, data: { title: '标出这个页面应调整的位置', source_version: 'image-layout-1',
        width: 640, height: 400, image_file_name: 'visual-background.png' } satisfies VisualFeedbackData },
      attachments: [{ name: 'visual-background.png', mimeType: 'image/png', contentsBase64: backgroundPng.replace(/^data:image\/png;base64,/, '') }] }],
  guide: {
    version: 1,
    steps: [
      {
        id: 'tools', target: '[data-tour="visual-tools"]',
        title: ['选择工具，把想法画出来', 'Choose a tool to sketch your idea'],
        body: ['使用自由笔、箭头、矩形或文字表达视觉指引；颜色、线宽和文字大小也在这里调整。撤销和重做可以回退操作，选择工具用于选中已有标注。', 'Use the pen, arrows, rectangles or text to share visual guidance. Adjust colors, line widths and text sizes here. Undo and redo let you revisit edits; Select picks an existing annotation.'],
      },
      {
        id: 'canvas', target: '[data-tour="visual-canvas"]',
        title: ['在原图或空白画布上标记', 'Mark the image or blank canvas'],
        body: ['拖动绘制线条、箭头或矩形；文字工具点击后可以编辑文字。图片背景保持不变，标注单独保存；引导不会替你绘制。', 'Drag to draw a line, arrow or rectangle. With the text tool, click to place editable text. The background image stays unchanged and annotations are saved separately. This guide does not draw for you.'],
      },
      {
        id: 'view', target: '[data-tour="visual-view-controls"]',
        title: ['缩放或全屏，继续同一份草稿', 'Zoom or go full screen with the same draft'],
        body: ['放大、缩小或适合窗口只改变显示，标注位置保持不变。需要更多空间时打开全屏画布，返回后继续同一份草稿。', 'Zoom in, zoom out or fit the canvas without changing annotation positions. Open the full screen canvas when you need more room, then return to the same draft.'],
      },
      {
        id: 'annotations', target: '[data-tour="visual-annotations"]',
        title: ['选择标注，补充具体意见', 'Select an annotation to add feedback'],
        body: ['这里列出已有标注。点击一条可以编辑文字、补充意见，也能使用语音和附件；删除标注使用上方工具栏。只有整体想法时，也可以直接写在右侧正文。', 'Your annotations appear here. Select one to edit its text or add a comment, using voice and attachments if helpful. Delete the selected annotation from the toolbar above. You can also write only overall feedback on the right.'],
      },
      {
        id: 'submit', target: '[data-feedback-actions]',
        title: ['把图片和意见一起提交', 'Send the image and your feedback'],
        body: ['检查标注与整体说明后提交。工作台会生成带标注的 PNG，连同结构化意见发送给 Agent；只写整体说明时也会附上当前画布。标题旁可以随时重看引导。', 'Check your annotations and overall feedback, then submit. The workbench creates an annotated PNG and sends it with structured comments to the agent. Overall feedback alone also includes the current canvas. Reopen this guide beside the title at any time.'],
      },
    ],
  },
}
