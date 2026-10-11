import type { DocumentReviewData, ReviewAnnotation, WorkbenchState } from '../../generated/feedback'
import { text, uniqueId } from '../definitions/validation'

export type DocumentReviewState = Extract<WorkbenchState, { type: 'document_review' }>
export type ReviewAnchor = Pick<ReviewAnnotation, 'start' | 'end' | 'quote'>

export function emptyReviewState(): DocumentReviewState {
  return { type: 'document_review', verdict: null, annotations: [] }
}

export function hasReviewInput(_data: DocumentReviewData, state: DocumentReviewState | null): boolean {
  return !!state && (state.verdict === 'ready' || state.verdict === 'changes_requested'
    || (Array.isArray(state.annotations) && state.annotations.some((item) =>
      (typeof item?.body === 'string' && item.body.trim()) || (item?.kind === 'suggestion' && typeof item.replacement === 'string'))))
}

export function validAnchor(source: string[], anchor: ReviewAnchor): boolean {
  if (anchor.start === null && anchor.end === null && anchor.quote === null) return true
  return Number.isInteger(anchor.start) && Number.isInteger(anchor.end)
    && anchor.start !== null && anchor.end !== null && anchor.start >= 0
    && anchor.end > anchor.start && anchor.end <= source.length
    && source.slice(anchor.start, anchor.end).join('') === anchor.quote
}

/** Submission validation. Incomplete comments remain valid editable drafts. */
export function validateReviewState(data: DocumentReviewData, state: DocumentReviewState | null): string | null {
  if (!state || !['ready', 'changes_requested'].includes(state.verdict ?? '')) return 'Choose a review decision.'
  if (!Array.isArray(state.annotations) || state.annotations.length > 500) return 'Invalid review state.'
  const ids = new Set<string>(), source = [...data.markdown]
  for (const annotation of state.annotations) {
    if (!annotation || !uniqueId(annotation.id, ids)) return 'Invalid or duplicate comment id.'
    if (!text(annotation.body, 4000, annotation.kind === 'comment')) return 'Finish or remove empty comments.'
    if (annotation.kind === 'suggestion') {
      if (!text(annotation.replacement, 8000, false)) return 'Enter the suggested wording.'
      if (annotation.replacement === (annotation.quote ?? data.markdown) && !text(annotation.body, 4000, true)) return 'Change the wording or add a reason before finishing.'
    } else if (annotation.kind !== 'comment' || annotation.replacement !== null) return 'Invalid comment kind.'
    if (!validAnchor(source, annotation)) return 'A comment no longer matches the original text.'
  }
  return null
}

/** New selections create separate notes; reopening a card edits its stable ID. */
export function prepareAnnotation(
  data: DocumentReviewData, state: DocumentReviewState, anchor: ReviewAnchor,
  kind: ReviewAnnotation['kind'], idFactory: () => string, deletion = false,
): { annotation: ReviewAnnotation; state: DocumentReviewState } | null {
  if (state.annotations.length >= 500 || !validAnchor([...data.markdown], anchor)) return null
  const source = anchor.quote ?? data.markdown
  if (kind === 'suggestion' && !deletion && [...source].length > 8000) return null
  const annotation: ReviewAnnotation = { ...anchor, id: idFactory(), kind, body: '',
    replacement: kind === 'suggestion' ? (deletion ? '' : source) : null }
  return { annotation, state: { ...state, annotations: [...state.annotations, annotation] } }
}
