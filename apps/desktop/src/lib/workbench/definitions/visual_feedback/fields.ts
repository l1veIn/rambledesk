import type { VisualFeedbackData } from '../../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../../fields/contracts'
import { fieldFingerprint } from '../../fields/value'
import { validVisualFeedbackInput } from './input'

export const visualAnnotationField: WorkbenchFieldAdapter = {
  accepts: (target) => target.destination.kind === 'workbench_field' && target.destination.workbenchType === 'visual_feedback'
    && target.destination.version === 1 && ['text', 'body'].includes(target.destination.field),
  read: ({ spec, state, target }) => {
    const destination = target.destination
    if (spec.type !== 'visual_feedback' || spec.version !== 1 || !validVisualFeedbackInput(spec.data)
      || state.type !== 'visual_feedback' || destination.kind !== 'workbench_field'
      || destination.workbenchType !== 'visual_feedback' || destination.version !== 1
      || destination.sourceVersion !== (spec.data as VisualFeedbackData).source_version) throw new Error('The canvas has changed. Select the annotation again.')
    const annotation = state.annotations.find((mark) => mark.id === destination.entityId)
    const field = destination.field
    if (!annotation || (field !== 'body' && field !== 'text') || (field === 'text' && annotation.kind !== 'text')) throw new Error('This annotation is no longer available.')
    return { value: annotation[field], limit: field === 'text' ? 2000 : 4000,
      contract: fieldFingerprint(spec.data), identity: fieldFingerprint([annotation.id, annotation.kind, annotation.points, field]),
      replace: (value) => ({ ...state, composite_attachment_id: null,
        annotations: state.annotations.map((mark) => mark.id === annotation.id ? { ...mark, [field]: value } : mark) }) }
  },
  text: (state, target) => {
    const destination = target.destination
    if (state.type !== 'visual_feedback' || destination.kind !== 'workbench_field' || !['text', 'body'].includes(destination.field)) return null
    const annotation = state.annotations.find((mark) => mark.id === destination.entityId)
    return annotation ? annotation[destination.field as 'text' | 'body'] : null
  },
  removeAttachment: (state, id, removeText) => state.type === 'visual_feedback' ? { ...state,
    composite_attachment_id: state.composite_attachment_id === id ? null : state.composite_attachment_id,
    annotations: state.annotations.map((mark) => ({ ...mark, text: removeText(mark.text), body: removeText(mark.body) })) } : state,
}
