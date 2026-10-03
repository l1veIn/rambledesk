import type { MediaReviewComment, WorkbenchState } from '../../../generated/feedback'
import { fields, list, object, text, uniqueId } from '../validation'

export type MediaReviewState = Extract<WorkbenchState, { type: 'media_review' }>
export const milliseconds = (value: unknown): value is number => Number.isInteger(value) && Number(value) >= 0 && Number(value) <= 86400000
export function readMediaReviewState(value: unknown): MediaReviewState | null {
  if (!object(value) || !fields(value, ['type', 'comments']) || value.type !== 'media_review' || !list(value.comments, 0, 500)) return null
  const ids = new Set<string>()
  if (!value.comments.every((comment) => object(comment) && fields(comment, ['id', 'start_ms', 'end_ms', 'body'])
    && uniqueId(comment.id, ids) && milliseconds(comment.start_ms)
    && (comment.end_ms === null || (milliseconds(comment.end_ms) && comment.end_ms > comment.start_ms))
    && text(comment.body, 4000, false))) return null
  return { type: 'media_review', comments: value.comments as MediaReviewComment[] }
}
