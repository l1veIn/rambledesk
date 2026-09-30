import type { ParagraphMark, QuestionAnswer, ReviewAnnotation, TerminalTrialSession, WebReviewAnnotation, WorkbenchState } from '../../../generated/feedback'
import { record, text, nullableText, nullableInteger } from '../stateShape'

type QuestionsState = Extract<WorkbenchState, { type: 'questions' }>

function questionAnswer(value: unknown): value is QuestionAnswer {
  return record(value) && text(value.id) && text(value.value) && text(value.label)
    && typeof value.wasCustom === 'boolean'
}

// Draft decoding checks editable shape only. Empty text, incomplete anchors and
// unknown fields must survive loading; submission policy validates their meaning.
export function readQuestionsState(value: unknown): QuestionsState | null {
  return record(value) && value.type === 'questions'
    && Array.isArray(value.answers) && value.answers.every(questionAnswer)
    ? value as QuestionsState : null
}
