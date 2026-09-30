import type { WebReviewData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint, validFieldText } from '../fields/value'

export const webReviewInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: (target) => target.destination.kind === 'web_review_annotation',
  text(state, target) {
    const destination = target.destination
    if (destination.kind !== 'web_review_annotation' || state.type !== 'web_review') return null
    const annotations = state.annotations.filter((annotation) => annotation.id === destination.annotationId)
    return annotations.length === 1 ? annotations[0].body : null
  },
  removeAttachment(state, attachmentId, remove) {
    if (state.type !== 'web_review') return state
    let changed = false
    const annotations = state.annotations.map((annotation) => {
      const body = remove(annotation.body), screenshotRemoved = annotation.screenshot_attachment_id === attachmentId
      if (body === annotation.body && !screenshotRemoved) return annotation
      changed = true
      const next = { ...annotation, body }
      if (screenshotRemoved) delete next.screenshot_attachment_id
      return next
    })
    return changed ? { ...state, annotations } : state
  },
  read({ spec, state, target }) {
    const destination = target.destination
    if (destination.kind !== 'web_review_annotation' || spec.type !== 'web_review' || state.type !== 'web_review') throw new Error('This webpage comment is unavailable.')
    const annotations = state.annotations.filter((item) => item.id === destination.annotationId)
    if (annotations.length !== 1 || !validFieldText(annotations[0].body, 4000)) throw new Error('The target comment no longer exists or cannot receive input.')
    const annotation = annotations[0]
    return {
      value: annotation.body, limit: 4000,
      contract: fieldFingerprint({ type: spec.type, version: spec.version, data: spec.data as WebReviewData }),
      identity: fieldFingerprint({ id: annotation.id, page_url: annotation.page_url, viewport: annotation.viewport, element: annotation.element }),
      replace: (value) => ({ ...state, annotations: state.annotations.map((item) => item === annotation ? { ...item, body: value } : item) }),
    }
  },
}]
