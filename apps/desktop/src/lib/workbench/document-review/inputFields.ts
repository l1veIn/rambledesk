import type { DocumentReviewData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint, validFieldText } from '../fields/value'

export const documentReviewInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: (target) => target.destination.kind === 'review_annotation',
  text(state, target) {
    const destination = target.destination
    if (destination.kind !== 'review_annotation' || state.type !== 'document_review') return null
    const annotations = state.annotations.filter((annotation) => annotation.id === destination.annotationId)
    return annotations.length === 1 && (destination.field === 'body' || annotations[0].kind === 'suggestion')
      ? annotations[0][destination.field] : null
  },
  removeAttachment(state, _attachmentId, remove) {
    if (state.type !== 'document_review') return state
    let changed = false
    const annotations = state.annotations.map((annotation) => {
      const body = remove(annotation.body), replacement = annotation.replacement === null ? null : remove(annotation.replacement)
      if (body === annotation.body && replacement === annotation.replacement) return annotation
      changed = true
      return { ...annotation, body, replacement }
    })
    return changed ? { ...state, annotations } : state
  },
  sameIdentity(capturedIdentity, currentIdentity, target) {
    if (capturedIdentity === currentIdentity) return true
    if (target.destination.kind !== 'review_annotation' || target.destination.field !== 'body') return false
    try {
      const captured: unknown = JSON.parse(capturedIdentity)
      return !!captured && typeof captured === 'object' && !Array.isArray(captured)
        && 'kind' in captured && captured.kind === 'comment'
        && fieldFingerprint({ ...captured, kind: 'suggestion' }) === currentIdentity
    } catch { return false }
  },
  read({ spec, state, target }) {
    const destination = target.destination
    if (destination.kind !== 'review_annotation' || spec.type !== 'document_review' || state.type !== 'document_review') throw new Error('This input field is unavailable.')
    const data = spec.data as DocumentReviewData
    if (data.source_version !== destination.sourceVersion) throw new Error('The original document version has changed. Your input has been preserved.')
    const annotations = state.annotations.filter((item) => item.id === destination.annotationId)
    if (annotations.length !== 1) throw new Error('The target comment no longer exists or has an ambiguous id.')
    const annotation = annotations[0]
    const paragraph = data.paragraphs.find((item) => item.id === annotation.paragraph_id)
    if (!paragraph) throw new Error('The target comment refers to an unavailable paragraph.')
    if (annotation.start !== null || annotation.end !== null || annotation.quote !== null) {
      const text = [...paragraph.text]
      if (annotation.start === null || annotation.end === null || !Number.isInteger(annotation.start) || !Number.isInteger(annotation.end)
        || annotation.start < 0 || annotation.end <= annotation.start || annotation.end > text.length
        || text.slice(annotation.start, annotation.end).join('') !== annotation.quote) throw new Error('The target comment no longer matches the original text.')
    }
    if ((annotation.kind === 'comment' && annotation.replacement !== null)
      || (annotation.kind === 'suggestion' && !validFieldText(annotation.replacement, 8000))
      || !validFieldText(annotation.body, 4000) || (destination.field === 'replacement' && annotation.kind !== 'suggestion')) {
      throw new Error('This comment cannot receive input in the selected field.')
    }
    return {
      value: annotation[destination.field]!, limit: destination.field === 'body' ? 4000 : 8000,
      contract: fieldFingerprint({ type: spec.type, version: spec.version, sourceVersion: data.source_version,
        paragraphId: paragraph.id, paragraphText: paragraph.text }),
      identity: fieldFingerprint({ id: annotation.id, paragraphId: annotation.paragraph_id, kind: annotation.kind,
        start: annotation.start, end: annotation.end, quote: annotation.quote }),
      replace: (value) => ({ ...state, annotations: state.annotations.map((item) => item === annotation ? { ...item, [destination.field]: value } : item) }),
    }
  },
}]
