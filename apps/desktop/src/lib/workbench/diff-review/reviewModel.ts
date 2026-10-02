import type { DiffReviewAnchor, DiffReviewComment, DiffReviewData } from '../../generated/feedback'
import type { DiffReviewState } from '../definitions/diff_review/state'
import { text } from '../definitions/validation'
import { parseReviewDiff, sameDiffAnchor, validDiffAnchor, type ReviewDiffHunk } from './diffModel'

export const emptyDiffReviewState = (): DiffReviewState => ({ type: 'diff_review', comments: [] })
export function validateDiffReviewState(data: DiffReviewData, state: DiffReviewState | null): string | null {
  if (!state) return null
  const parsed = new Map<string, ReviewDiffHunk[] | null>()
  for (const comment of state.comments) {
    const file = data.files.find((item) => item.id === comment.anchor.file_id)
    if (file && !parsed.has(file.id)) parsed.set(file.id, parseReviewDiff(file.diff))
    if (!file || !validDiffAnchor(file, comment.anchor, parsed.get(file.id))) return 'A comment no longer matches the original diff.'
    if (!text(comment.body, 4000)) return 'Write a comment or remove its empty draft before submitting.'
  }
  return null
}
export function prepareDiffReviewComment(data: DiffReviewData, state: DiffReviewState, anchor: DiffReviewAnchor,
  createId: () => string): { state: DiffReviewState; comment: DiffReviewComment } | null {
  const file = data.files.find((item) => item.id === anchor.file_id)
  if (!file || !validDiffAnchor(file, anchor)) return null
  const existing = state.comments.find((comment) => sameDiffAnchor(comment.anchor, anchor))
  if (existing) return { state, comment: existing }
  if (state.comments.length >= 500) return null
  const id = createId()
  if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(id) || state.comments.some((comment) => comment.id === id)) return null
  const comment: DiffReviewComment = { id, anchor: { ...anchor }, body: '' }
  return { state: { ...state, comments: [...state.comments, comment] }, comment }
}
