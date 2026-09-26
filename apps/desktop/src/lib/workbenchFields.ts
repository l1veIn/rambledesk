import type { DocumentReviewData, QuestionsData } from './generated/feedback'
import { decodeFeedbackDraftEnvelope, type FeedbackDraftSnapshot } from './feedbackDraftDocument'
import { decodeWorkbenchState, resolveWorkbenchPolicy } from './workbenchPolicy'
import type { InputTarget, InputWriteWorkspace } from './domain/inputTarget'

/** One validated field from one complete draft; replace preserves the envelope. */
export type WorkbenchField = ReturnType<typeof readWorkbenchField>

export const validFieldText = (value: unknown, limit: number): value is string => typeof value === 'string'
  && !value.includes('\0') && [...value].length <= limit

/** Property order is not part of a workbench contract's identity. */
function fingerprint(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(fingerprint).join(',')}]`
  if (value && typeof value === 'object') return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b))
    .map(([key, item]) => `${JSON.stringify(key)}:${fingerprint(item)}`).join(',')}}`
  return JSON.stringify(value) ?? 'undefined'
}

/** Adding suggested wording keeps the existing comment body and its captured input valid. */
export function sameWorkbenchFieldIdentity(capturedIdentity: string, currentIdentity: string, target: InputTarget): boolean {
  if (capturedIdentity === currentIdentity) return true
  if (target.destination.kind !== 'review_annotation' || target.destination.field !== 'body') return false
  try {
    const captured: unknown = JSON.parse(capturedIdentity)
    if (!captured || typeof captured !== 'object' || Array.isArray(captured)
      || !('kind' in captured) || captured.kind !== 'comment') return false
    return fingerprint({ ...captured, kind: 'suggestion' }) === currentIdentity
  } catch { return false }
}

export function readWorkbenchField(workspace: InputWriteWorkspace, target: InputTarget) {
  if (workspace.request.request_id !== target.requestId) throw new Error('The input request is no longer open.')
  if (workspace.request.status === 'completed' || workspace.request.status === 'cancelled') throw new Error('This request is closed. The draft is read-only.')
  if (!resolveWorkbenchPolicy(workspace.workbench)) throw new Error('This workbench is not supported. The draft is read-only.')
  const envelope = decodeFeedbackDraftEnvelope(workspace.draft.document_json)
  const state = decodeWorkbenchState(envelope?.workbenchState)
  if (!envelope) throw new Error('The draft is unavailable.')
  const destination = target.destination
  const snapshot = (workbenchState: unknown): FeedbackDraftSnapshot => ({
    documentJson: JSON.stringify({ ...envelope, workbenchState }), bodyMarkdown: workspace.draft.body_markdown,
  })
  if (destination.kind === 'question_answer') {
    if (workspace.workbench?.type !== 'questions' || state?.type !== 'questions') throw new Error('This answer is unavailable.')
    const question = (workspace.workbench.data as QuestionsData).questions.find((item) => item.id === destination.questionId)
    const answers = state.answers.filter((item) => item.id === destination.questionId)
    if (!question?.allowOther || answers.length !== 1 || answers[0].wasCustom !== true || !validFieldText(answers[0].value, 4000)) {
      throw new Error('The custom answer was cleared or changed. Your input has been preserved.')
    }
    const answer = answers[0]
    const contract = fingerprint({ type: workspace.workbench.type, version: workspace.workbench.version, question })
    return { value: answer.value, limit: 4000, contract, identity: fingerprint({ id: answer.id, wasCustom: true }),
      replace: (value: string) => snapshot({ ...state, answers: state.answers.map((item) => {
        if (item !== answer) return item
        const next = { ...item, value, label: value }
        delete next.index
        return next
      }) }),
    }
  }
  if (destination.kind !== 'review_annotation' || workspace.workbench?.type !== 'document_review' || state?.type !== 'document_review') {
    throw new Error('This input field is unavailable.')
  }
  const data = workspace.workbench.data as DocumentReviewData
  if (data.source_version !== destination.sourceVersion) throw new Error('The original document version has changed. Your input has been preserved.')
  const annotations = state.annotations.filter((item) => item.id === destination.annotationId)
  if (annotations.length !== 1) throw new Error('The target comment no longer exists or has an ambiguous id.')
  const annotation = annotations[0]
  const paragraph = data.paragraphs.find((item) => item.id === annotation.paragraph_id)
  if (!paragraph) throw new Error('The target comment refers to an unavailable paragraph.')
  // Retain only this paragraph, including for whole-paragraph comments without a quoted anchor.
  const contract = fingerprint({ type: workspace.workbench.type, version: workspace.workbench.version,
    sourceVersion: data.source_version, paragraphId: paragraph.id, paragraphText: paragraph.text })
  if (annotation.start !== null || annotation.end !== null || annotation.quote !== null) {
    const text = [...paragraph.text]
    if (annotation.start === null || annotation.end === null || !Number.isInteger(annotation.start) || !Number.isInteger(annotation.end)
      || annotation.start < 0 || annotation.end <= annotation.start || annotation.end > text.length
      || text.slice(annotation.start, annotation.end).join('') !== annotation.quote) throw new Error('The target comment no longer matches the original text.')
  }
  if ((annotation.kind === 'comment' && annotation.replacement !== null)
    || (annotation.kind === 'suggestion' && !validFieldText(annotation.replacement, 8000))
    || !validFieldText(annotation.body, 4000) || (destination.field === 'replacement' && annotation.kind !== 'suggestion')) {
    throw new Error('This comment cannot receive input in the selected field.')
  }
  return {
    value: annotation[destination.field]!, limit: destination.field === 'body' ? 4000 : 8000, contract,
    identity: fingerprint({ id: annotation.id, paragraphId: annotation.paragraph_id, kind: annotation.kind,
      start: annotation.start, end: annotation.end, quote: annotation.quote }),
    replace: (value: string) => snapshot({ ...state, annotations: state.annotations.map((item) => item === annotation
      ? { ...item, [destination.field]: value } : item) }),
  }
}

/** Append ordinary input; provenance and idempotency belong to the caller. */
export function appendWorkbenchField(field: WorkbenchField, text: string): FeedbackDraftSnapshot {
  if (!text.trim() || text.includes('\0')) throw new Error('The input contains no valid text.')
  const value = field.value + (field.value && !/\s$/.test(field.value) ? '\n' : '') + text
  if (!validFieldText(value, field.limit)) throw new Error('The input would exceed its text limit. Your original has been kept.')
  return field.replace(value)
}
