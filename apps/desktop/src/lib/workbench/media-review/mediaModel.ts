import type { MediaReviewComment, MediaReviewData } from '../../generated/feedback'
import { readMediaReviewState, type MediaReviewState } from '../definitions/media_review/state'
import { text, uniqueId } from '../definitions/validation'

export type MediaCommentSort = 'time' | 'newest' | 'oldest'
export const emptyMediaReviewState = (): MediaReviewState => ({ type: 'media_review', comments: [] })
export function validMediaAnchor(data: MediaReviewData, comment: Pick<MediaReviewComment, 'start_ms' | 'end_ms'>): boolean {
  return Number.isInteger(comment.start_ms) && comment.start_ms >= 0 && comment.start_ms <= data.duration_ms
    && (comment.end_ms === null || (Number.isInteger(comment.end_ms) && comment.end_ms > comment.start_ms && comment.end_ms <= data.duration_ms))
}
export function validateMediaReviewState(data: MediaReviewData, value: MediaReviewState | null): string | null {
  if (value === null) return null
  const state = readMediaReviewState(value)
  if (!state || state.comments.some((comment) => !validMediaAnchor(data, comment))) return 'This media review draft is invalid.'
  return state.comments.some((comment) => !text(comment.body, 4000)) ? 'Write a comment or remove its empty marker before submitting.' : null
}
export function prepareMediaComment(data: MediaReviewData, state: MediaReviewState,
  anchor: Pick<MediaReviewComment, 'start_ms' | 'end_ms'>, id: () => string): { state: MediaReviewState; comment: MediaReviewComment } | null {
  if (!validMediaAnchor(data, anchor) || !readMediaReviewState(state)) return null
  const existing = state.comments.find((comment) => comment.start_ms === anchor.start_ms && comment.end_ms === anchor.end_ms)
  if (existing) return { state, comment: existing }
  if (state.comments.length >= 500) return null
  const comment = { id: id(), ...anchor, body: '' }
  if (!uniqueId(comment.id, new Set(state.comments.map((item) => item.id)))) return null
  return { state: { ...state, comments: [...state.comments, comment] }, comment }
}
export function sortedMediaComments(comments: readonly MediaReviewComment[], order: MediaCommentSort): MediaReviewComment[] {
  const sorted = [...comments]
  return order === 'newest' ? sorted.reverse() : order === 'time' ? sorted.sort((a, b) => a.start_ms - b.start_ms || (a.end_ms ?? a.start_ms) - (b.end_ms ?? b.start_ms)) : sorted
}
export function mediaTime(ms: number): string {
  const value = Math.max(0, Math.round(ms)), minutes = Math.floor(value / 60000), seconds = Math.floor(value % 60000 / 1000)
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}.${String(value % 1000).padStart(3, '0')}`
}
export const mediaAnchorLabel = (comment: Pick<MediaReviewComment, 'start_ms' | 'end_ms'>) =>
  comment.end_ms === null ? mediaTime(comment.start_ms) : `${mediaTime(comment.start_ms)} – ${mediaTime(comment.end_ms)}`
export const mediaDurationMismatch = (actualSeconds: number, requestedMs: number) =>
  !Number.isFinite(actualSeconds) || actualSeconds <= 0 || Math.abs(actualSeconds * 1000 - requestedMs) > Math.max(250, requestedMs * 0.01)
