import type { FeedbackDraftSnapshot } from './feedbackDraftDocument'
import type { Question, QuestionAnswer, WorkbenchSpec, WorkbenchState } from './generated/feedback'

export function readWorkbenchState(documentJson: string | null | undefined): WorkbenchState | null {
  if (!documentJson) return null
  try {
    const envelope = JSON.parse(documentJson)
    const value = envelope.schemaVersion === 2 ? envelope.workbenchState : null
    if (value?.type === 'questions' && Array.isArray(value.answers)) return value
    if (value?.type === 'single_choice' && (value.selected_option_id === null || typeof value.selected_option_id === 'string')) return value
  } catch { /* Legacy Markdown-only drafts have no interaction state. */ }
  return null
}

export function withWorkbenchState(snapshot: FeedbackDraftSnapshot, state: WorkbenchState | null): FeedbackDraftSnapshot {
  if (!state) return snapshot
  return { ...snapshot, documentJson: JSON.stringify({ ...JSON.parse(snapshot.documentJson), workbenchState: state }) }
}

export function preserveWorkbenchState(snapshot: FeedbackDraftSnapshot, previous: string | null | undefined): FeedbackDraftSnapshot {
  return withWorkbenchState(snapshot, readWorkbenchState(snapshot.documentJson) ?? readWorkbenchState(previous))
}

function validAnswer(question: Question, answer: QuestionAnswer): boolean {
  if (!answer || typeof answer.value !== 'string') return false
  return answer.wasCustom === true
    ? question.allowOther && answer.value.trim().length > 0 && [...answer.value].length <= 4000 && !answer.value.includes('\0')
    : answer.wasCustom === false && question.options.some((option) => option.value === answer.value)
}

export function hasWorkbenchInput(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null): boolean {
  if (!spec || spec.version !== 1) return false
  if (spec.type === 'single_choice' && 'options' in spec.data && state?.type === 'single_choice') {
    return spec.data.options.some((option) => option.id === state.selected_option_id)
  }
  if (spec.type === 'questions' && 'questions' in spec.data && state?.type === 'questions') {
    return spec.data.questions.some((question) => state.answers.some((answer) => answer?.id === question.id && validAnswer(question, answer)))
  }
  return false
}

function workbenchComplete(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null): boolean {
  if (!spec || spec.type === 'ramble') return true
  if (spec.version !== 1) return false
  if (spec.type === 'single_choice' && 'options' in spec.data && state?.type === 'single_choice') {
    return spec.data.options.some((option) => option.id === state.selected_option_id)
  }
  if (spec.type === 'questions' && 'questions' in spec.data && state?.type === 'questions') {
    if (state.answers.length !== spec.data.questions.length) return false
    return spec.data.questions.every((question) => {
      const answers = state.answers.filter((answer) => answer?.id === question.id)
      if (answers.length !== 1) return false
      return validAnswer(question, answers[0])
    })
  }
  return false
}

export function workbenchSubmissionIssue(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null, notes: string): 'empty' | 'incomplete' | null {
  if (!notes.trim() && !hasWorkbenchInput(spec, state)) return 'empty'
  return workbenchComplete(spec, state) ? null : 'incomplete'
}

export function canSubmitWorkbench(spec: WorkbenchSpec | null | undefined, state: WorkbenchState | null, notes: string): boolean {
  return workbenchSubmissionIssue(spec, state, notes) === null
}
