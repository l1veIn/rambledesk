import type { ParagraphMark, QuestionAnswer, ReviewAnnotation, TerminalTrialSession, WebReviewAnnotation, WorkbenchState } from './generated/feedback'

type QuestionsState = Extract<WorkbenchState, { type: 'questions' }>
type SingleChoiceState = Extract<WorkbenchState, { type: 'single_choice' }>
type DocumentReviewState = Extract<WorkbenchState, { type: 'document_review' }>
type WebReviewState = Extract<WorkbenchState, { type: 'web_review' }>
type TerminalState = Extract<WorkbenchState, { type: 'terminal' }>

const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value)
const text = (value: unknown): value is string => typeof value === 'string'
const nullableText = (value: unknown): value is string | null => value === null || text(value)
const nullableInteger = (value: unknown): value is number | null => value === null || Number.isInteger(value)

function questionAnswer(value: unknown): value is QuestionAnswer {
  return record(value) && text(value.id) && text(value.value) && text(value.label)
    && typeof value.wasCustom === 'boolean'
}

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

// Draft decoding checks editable shape only. Empty text, incomplete anchors and
// unknown fields must survive loading; submission policy validates their meaning.
export function readQuestionsState(value: unknown): QuestionsState | null {
  return record(value) && value.type === 'questions'
    && Array.isArray(value.answers) && value.answers.every(questionAnswer)
    ? value as QuestionsState : null
}

/** Historical single_choice requests retain their original wire state. */
export function readSingleChoiceState(value: unknown): SingleChoiceState | null {
  return record(value) && value.type === 'single_choice' && nullableText(value.selected_option_id)
    ? value as SingleChoiceState : null
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

function terminalSession(value: unknown): value is TerminalTrialSession {
  return record(value) && text(value.id) && text(value.cwd) && text(value.shell)
    && Number.isInteger(value.cols) && Number.isInteger(value.rows)
    && ['running', 'exited', 'stopped'].includes(value.status as string) && nullableInteger(value.exit_code)
    && text(value.output) && text(value.screen) && typeof value.truncated === 'boolean'
}

export function readTerminalState(value: unknown): TerminalState | null {
  return record(value) && value.type === 'terminal' && Array.isArray(value.sessions) && value.sessions.every(terminalSession)
    ? value as TerminalState : null
}
