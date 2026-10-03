import type { DiffReviewAnchor, DiffReviewComment, WorkbenchState } from '../../../generated/feedback'
import { fields, list, object, text, uniqueId } from '../validation'

export type DiffReviewState = Extract<WorkbenchState, { type: 'diff_review' }>
const line = (value: unknown) => Number.isInteger(value) && Number(value) >= 1 && Number(value) <= 0xffffffff
function anchor(value: unknown): value is DiffReviewAnchor {
  return object(value) && fields(value, ['file_id', 'hunk_index', 'side', 'start_line', 'end_line'])
    && uniqueId(value.file_id, new Set()) && Number.isInteger(value.hunk_index) && Number(value.hunk_index) >= 0
    && Number(value.hunk_index) <= 0xffffffff && (value.side === 'old' || value.side === 'new')
    && ((value.start_line === null && value.end_line === null)
      || (line(value.start_line) && line(value.end_line) && Number(value.start_line) <= Number(value.end_line)))
}
export function readDiffReviewState(value: unknown): DiffReviewState | null {
  if (!object(value) || !fields(value, ['type', 'comments']) || value.type !== 'diff_review' || !list(value.comments, 0, 500)) return null
  const ids = new Set<string>()
  if (!value.comments.every((comment) => object(comment) && fields(comment, ['id', 'anchor', 'body'])
    && uniqueId(comment.id, ids) && anchor(comment.anchor) && text(comment.body, 4000, false))) return null
  return { type: 'diff_review', comments: value.comments as DiffReviewComment[] }
}
