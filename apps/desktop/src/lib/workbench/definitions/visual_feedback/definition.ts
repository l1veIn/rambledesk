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
}
