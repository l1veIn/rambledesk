import type { Question, QuestionAnswer, QuestionsData } from '../../../generated/feedback'
import type { WorkbenchDefinition } from '../contracts'
import { validQuestionsInput } from './input'
import { readQuestionsState } from './state'
import { questionInputFields } from '../../questions/inputFields'
import { examples } from './examples'
function validAnswer(question: Question, answer: QuestionAnswer): boolean {
  if (!answer || typeof answer.value !== 'string') return false
  return answer.wasCustom === true
    ? question.allowOther && answer.value.trim().length > 0 && [...answer.value].length <= 4000 && !answer.value.includes('\0')
    : answer.wasCustom === false && question.options.some((option) => option.value === answer.value)
}
export const questionsDefinition: WorkbenchDefinition = {
  type: 'questions', version: 1, accepts: validQuestionsInput, decodeState: readQuestionsState,
  hasInput: (spec, state) => state?.type === 'questions' && (spec.data as QuestionsData).questions.some((question) =>
    state.answers.some((answer) => answer.id === question.id && validAnswer(question, answer))),
  complete: (spec, state) => state?.type === 'questions' && state.answers.length === (spec.data as QuestionsData).questions.length &&
    (spec.data as QuestionsData).questions.every((question) => {
      const answers = state.answers.filter((answer) => answer.id === question.id)
      return answers.length === 1 && validAnswer(question, answers[0])
    }),
  layout: { padded: true, interactivePreview: false, expanded: false }, fields: questionInputFields,
  loadView: () => import('./View.svelte'), examples,
}
