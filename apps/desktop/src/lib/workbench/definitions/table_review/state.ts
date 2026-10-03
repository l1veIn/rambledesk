import type { TableReviewChange, TableReviewComment, WorkbenchState } from '../../../generated/feedback'
import { fields, list, object, text, uniqueId } from '../validation'

export type TableReviewState = Extract<WorkbenchState, { type: 'table_review' }>
const cell = (value: Record<string, unknown>) => uniqueId(value.row_id, new Set()) && uniqueId(value.column_id, new Set())
export function readTableReviewState(value: unknown): TableReviewState | null {
  if (!object(value) || !fields(value, ['type', 'changes', 'comments']) || value.type !== 'table_review'
    || !list(value.changes, 0, 20000) || !list(value.comments, 0, 500)) return null
  const cells = new Set<string>(), comments = new Set<string>()
  let total = 0
  if (!value.changes.every((change) => {
    if (!object(change) || !fields(change, ['row_id', 'column_id', 'value']) || !cell(change)
      || !text(change.value, 4000, false)) return false
    total += [...change.value].length
    if (total > 500000) return false
    const key = `${change.row_id}:${change.column_id}`
    if (cells.has(key)) return false
    cells.add(key)
    return true
  }) || !value.comments.every((comment) => object(comment) && fields(comment, ['id', 'row_id', 'column_id', 'body'])
    && uniqueId(comment.id, comments) && cell(comment) && text(comment.body, 4000, false))) return null
  return { type: 'table_review', changes: value.changes as TableReviewChange[], comments: value.comments as TableReviewComment[] }
}
