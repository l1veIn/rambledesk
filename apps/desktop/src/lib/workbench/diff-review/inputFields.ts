import type { DiffReviewData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint, validFieldText } from '../fields/value'
import { validDiffAnchor } from './diffModel'

export const diffReviewInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: (target) => target.destination.kind === 'workbench_field' && target.destination.workbenchType === 'diff_review'
    && target.destination.version === 1 && target.destination.field === 'comment_body',
  text(state, target) {
    const destination = target.destination
    if (state.type !== 'diff_review' || destination.kind !== 'workbench_field' || destination.workbenchType !== 'diff_review'
      || destination.version !== 1 || destination.field !== 'comment_body') return null
    const comments = state.comments.filter((comment) => comment.id === destination.entityId)
    return comments.length === 1 ? comments[0].body : null
  },
  removeAttachment(state, _id, removeText) {
    if (state.type !== 'diff_review') return state
    let changed = false
    const comments = state.comments.map((comment) => {
      const body = removeText(comment.body)
      if (body === comment.body) return comment
      changed = true
      return { ...comment, body }
    })
    return changed ? { ...state, comments } : state
  },
  read({ spec, state, target }) {
    const destination = target.destination, data = spec.data as DiffReviewData
    if (spec.type !== 'diff_review' || spec.version !== 1 || state.type !== 'diff_review'
      || destination.kind !== 'workbench_field' || destination.workbenchType !== spec.type || destination.version !== spec.version
      || destination.field !== 'comment_body' || destination.sourceVersion !== data.source_version) throw new Error('The original diff has changed. Select the comment again.')
    const comments = state.comments.filter((comment) => comment.id === destination.entityId)
    const comment = comments[0], file = comment && data.files.find((item) => item.id === comment.anchor.file_id)
    if (comments.length !== 1 || !file || !validDiffAnchor(file, comment.anchor) || !validFieldText(comment.body, 4000)) throw new Error('This diff comment is unavailable.')
    return {
      value: comment.body, limit: 4000,
      contract: fieldFingerprint({ type: spec.type, version: spec.version, data }),
      identity: fieldFingerprint({ id: comment.id, anchor: comment.anchor }),
      replace: (body) => ({ ...state, comments: state.comments.map((item) => item === comment ? { ...item, body } : item) }),
    }
  },
}]
