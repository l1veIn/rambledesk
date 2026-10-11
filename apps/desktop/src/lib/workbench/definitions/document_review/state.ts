import type { ReviewAnnotation, WorkbenchState } from '../../../generated/feedback'
import { record, text, nullableText, nullableInteger } from '../stateShape'

type DocumentReviewState = Extract<WorkbenchState, { type: 'document_review' }>

function reviewAnnotation(value: unknown): value is ReviewAnnotation {
  return record(value) && text(value.id) && text(value.body)
    && (value.kind === 'comment' || value.kind === 'suggestion')
    && nullableInteger(value.start) && nullableInteger(value.end)
    && nullableText(value.quote) && nullableText(value.replacement)
}

export function readDocumentReviewState(value: unknown): DocumentReviewState | null {
  if (!(record(value) && value.type === 'document_review'
    && (value.verdict === null || value.verdict === 'ready' || value.verdict === 'changes_requested')
    && Array.isArray(value.annotations) && value.annotations.every(reviewAnnotation))) return null
  return value as DocumentReviewState
}
