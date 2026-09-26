import type { DocumentReviewData, Question, QuestionAnswer, QuestionsData, SingleChoiceData, WorkbenchSpec, WorkbenchState } from './generated/feedback'
import { hasReviewInput, validateReviewState } from './workbench/document-review/reviewModel'
import { validDocumentReviewInput, validQuestionsInput, validRambleInput, validSingleChoiceInput } from './workbenchInputValidation'
import { readDocumentReviewState, readQuestionsState, readSingleChoiceState } from './workbenchStateDecoders'

export type WorkbenchType = 'ramble' | 'questions' | 'single_choice' | 'document_review'
type Policy = {
  type: WorkbenchType
  accepts: (data: Record<string, unknown>) => boolean
  readState: (value: unknown) => WorkbenchState | null
  hasInput: (spec: WorkbenchSpec, state: WorkbenchState | null) => boolean
  complete: (spec: WorkbenchSpec, state: WorkbenchState | null) => boolean
  submissionMessage?: (spec: WorkbenchSpec, state: WorkbenchState | null) => string | null
}
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value)
const text = (value: unknown): value is string => typeof value === 'string'
function validAnswer(question: Question, answer: QuestionAnswer): boolean {
  if (!answer || typeof answer.value !== 'string') return false
  return answer.wasCustom === true
    ? question.allowOther && answer.value.trim().length > 0 && [...answer.value].length <= 4000 && !answer.value.includes('\0')
    : answer.wasCustom === false && question.options.some((option) => option.value === answer.value)
}

/** Supported versions, renderer identity and submission rules share one decision. */
const policies: Record<WorkbenchType, Policy> = {
  ramble: {
    type: 'ramble', accepts: validRambleInput,
    readState: () => null, hasInput: () => false, complete: () => true,
  },
  questions: {
    type: 'questions', accepts: validQuestionsInput,
    readState: readQuestionsState,
    hasInput: (spec, state) => state?.type === 'questions' && (spec.data as QuestionsData).questions.some((question) =>
      state.answers.some((answer) => answer.id === question.id && validAnswer(question, answer))),
    complete: (spec, state) => state?.type === 'questions' && state.answers.length === (spec.data as QuestionsData).questions.length &&
      (spec.data as QuestionsData).questions.every((question) => {
        const answers = state.answers.filter((answer) => answer.id === question.id)
        return answers.length === 1 && validAnswer(question, answers[0])
      }),
  },
  single_choice: {
    // Compatibility only: new requests use questions, including one-question forms.
    type: 'single_choice', accepts: validSingleChoiceInput,
    readState: readSingleChoiceState,
    hasInput: (spec, state) => state?.type === 'single_choice' && (spec.data as SingleChoiceData).options.some((option) => option.id === state.selected_option_id),
    complete: (spec, state) => policies.single_choice.hasInput(spec, state),
  },
  document_review: {
    type: 'document_review', accepts: validDocumentReviewInput,
    readState: readDocumentReviewState,
    hasInput: (spec, state) => hasReviewInput(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null),
    complete: (spec, state) => validateReviewState(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null) === null,
    submissionMessage: (spec, state) => validateReviewState(spec.data as DocumentReviewData, state?.type === 'document_review' ? state : null),
  },
}

export function resolveWorkbenchPolicy(spec: WorkbenchSpec | null | undefined): Policy | null {
  if (!spec) return policies.ramble
  if (spec.version !== 1 || !Object.hasOwn(policies, spec.type) || !record(spec.data)) return null
  const policy = policies[spec.type as WorkbenchType]
  return policy.accepts(spec.data) ? policy : null
}

export function decodeWorkbenchState(value: unknown): WorkbenchState | null {
  if (!record(value) || !text(value.type) || !Object.hasOwn(policies, value.type)) return null
  return policies[value.type as WorkbenchType].readState(value)
}

export function workbenchIsReadOnly(spec: WorkbenchSpec | null | undefined): boolean {
  return resolveWorkbenchPolicy(spec) === null
}

export function workbenchSupportsApproval(spec: WorkbenchSpec | null | undefined): boolean {
  return resolveWorkbenchPolicy(spec)?.type === 'ramble'
}
