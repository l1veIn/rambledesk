import type { TableReviewData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint } from '../fields/value'
import { text } from '../definitions/validation'
import { cellIdentity, cellPosition } from './reviewModel'

export const tableReviewInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: (target) => target.destination.kind === 'workbench_field' && target.destination.workbenchType === 'table_review'
    && target.destination.version === 1 && ['comment_body', 'change_value'].includes(target.destination.field),
  text(state, target) {
    const destination = target.destination
    if (state.type !== 'table_review' || destination.kind !== 'workbench_field' || destination.workbenchType !== 'table_review'
      || destination.version !== 1) return null
    if (destination.field === 'comment_body') {
      const matches = state.comments.filter((comment) => comment.id === destination.entityId)
      return matches.length === 1 ? matches[0].body : null
    }
    const matches = state.changes.filter((change) => cellIdentity(change) === destination.entityId)
    return destination.field === 'change_value' && matches.length === 1 ? matches[0].value : null
  },
  removeAttachment(state, _id, removeText) {
    if (state.type !== 'table_review') return state
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
    const destination = target.destination, data = spec.data as TableReviewData
    if (spec.type !== 'table_review' || spec.version !== 1 || state.type !== 'table_review'
      || destination.kind !== 'workbench_field' || destination.workbenchType !== spec.type || destination.version !== spec.version
      || !['comment_body', 'change_value'].includes(destination.field) || destination.sourceVersion !== data.source_version)
      throw new Error('The original table has changed. Select the cell again.')
    const commentField = destination.field === 'comment_body'
    const entries = commentField ? state.comments.filter((comment) => comment.id === destination.entityId)
      : state.changes.filter((change) => cellIdentity(change) === destination.entityId)
    const entry = entries[0]
    const value = entry && ('body' in entry ? entry.body : entry.value)
    if (entries.length !== 1 || !entry || !cellPosition(data, entry) || !text(value, 4000, false)) throw new Error('This table field is unavailable.')
    return {
      value, limit: 4000, contract: fieldFingerprint({ type: spec.type, version: spec.version, data }),
      identity: fieldFingerprint({ cell: { row_id: entry.row_id, column_id: entry.column_id },
        ...(commentField ? { id: destination.entityId } : {}) }),
      replace: (next) => commentField
        ? { ...state, comments: state.comments.map((comment) => comment === entry ? { ...comment, body: next } : comment) }
        : { ...state, changes: state.changes.map((change) => change === entry ? { ...change, value: next } : change) },
    }
  },
}]
