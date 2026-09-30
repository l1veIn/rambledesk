import type { ParagraphMark, QuestionAnswer, ReviewAnnotation, TerminalTrialSession, WebReviewAnnotation, WorkbenchState } from '../../../generated/feedback'
import { record, text, nullableText, nullableInteger } from '../stateShape'

type WebReviewState = Extract<WorkbenchState, { type: 'web_review' }>

function webReviewAnnotation(value: unknown): value is WebReviewAnnotation {
  if (!record(value) || !text(value.id) || !text(value.page_url) || !text(value.body)
    || !record(value.viewport) || !record(value.element) || !record(value.element.rect)) return false
  const { element, viewport } = value
  const rect = element.rect as Record<string, unknown>
  return Number.isInteger(viewport.width) && Number.isInteger(viewport.height)
    && text(element.selector) && text(element.tag_name) && text(element.text)
    && ['x', 'y', 'width', 'height'].every((key) => Number.isInteger(rect[key]))
    && (value.screenshot_attachment_id == null || text(value.screenshot_attachment_id))
}

export function readWebReviewState(value: unknown): WebReviewState | null {
  return record(value) && value.type === 'web_review' && Array.isArray(value.annotations)
    && value.annotations.every(webReviewAnnotation) ? value as WebReviewState : null
}
