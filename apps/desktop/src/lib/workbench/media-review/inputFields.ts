import type { MediaReviewData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint, validFieldText } from '../fields/value'
import { validMediaAnchor } from './mediaModel'

export const mediaReviewInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: (target) => target.destination.kind === 'workbench_field' && target.destination.workbenchType === 'media_review'
    && target.destination.version === 1 && target.destination.field === 'comment_body',
  text(state, target) {
    const field = target.destination
    if (state.type !== 'media_review' || field.kind !== 'workbench_field' || field.workbenchType !== 'media_review'
      || field.version !== 1 || field.field !== 'comment_body') return null
    const matches = state.comments.filter((comment) => comment.id === field.entityId)
    return matches.length === 1 ? matches[0].body : null
  },
  removeAttachment(state, _id, removeText) {
    if (state.type !== 'media_review') return state
    let changed = false
    const comments = state.comments.map((comment) => {
      const body = removeText(comment.body)
      if (body === comment.body) return comment
      changed = true; return { ...comment, body }
    })
    return changed ? { ...state, comments } : state
  },
  read({ spec, state, target }) {
    const field = target.destination, data = spec.data as MediaReviewData
    if (spec.type !== 'media_review' || spec.version !== 1 || state.type !== 'media_review'
      || field.kind !== 'workbench_field' || field.workbenchType !== spec.type || field.version !== spec.version
      || field.field !== 'comment_body' || field.sourceVersion !== data.source_version) throw new Error('The original media has changed. Select the comment again.')
    const matches = state.comments.filter((comment) => comment.id === field.entityId), comment = matches[0]
    if (matches.length !== 1 || !validMediaAnchor(data, comment) || !validFieldText(comment.body, 4000)) throw new Error('This media comment is unavailable.')
    return { value: comment.body, limit: 4000, contract: fieldFingerprint({ type: spec.type, version: spec.version, data }),
      identity: fieldFingerprint({ id: comment.id, start_ms: comment.start_ms, end_ms: comment.end_ms }),
      replace: (body) => ({ ...state, comments: state.comments.map((item) => item === comment ? { ...item, body } : item) }) }
  },
}]
