import type { ParagraphMark, ReviewAnnotation, WorkbenchState } from '../../../generated/feedback'
import { record, text, nullableText, nullableInteger } from '../stateShape'

type DocumentReviewState = Extract<WorkbenchState, { type: 'document_review' }>

function reviewAnnotation(value: unknown): value is ReviewAnnotation {
  return record(value) && text(value.id) && text(value.paragraph_id) && text(value.body)
    && (value.kind === 'comment' || value.kind === 'suggestion')
    && nullableInteger(value.start) && nullableInteger(value.end)
    && nullableText(value.quote) && nullableText(value.replacement)
}

function paragraphMark(value: unknown): value is ParagraphMark {
  return record(value) && text(value.paragraph_id)
    && (value.decision === 'keep' || value.decision === 'revise' || value.decision === 'remove')
}

export function readDocumentReviewState(value: unknown): DocumentReviewState | null {
  if (!(record(value) && value.type === 'document_review'
    && (value.verdict === null || value.verdict === 'ready' || value.verdict === 'changes_requested')
    && Array.isArray(value.annotations) && value.annotations.every(reviewAnnotation)
    && Array.isArray(value.paragraph_marks) && value.paragraph_marks.every(paragraphMark))) return null
  const state = value as DocumentReviewState
  if (!state.annotations.some((annotation) => 'status' in annotation)) return state
  // Former open/resolved flags are retired; every saved note remains editable.
  return { ...state, annotations: state.annotations.map((annotation) => {
    const { status: _legacyStatus, ...note } = annotation as ReviewAnnotation & { status?: unknown }
    return note
  }) }
}
