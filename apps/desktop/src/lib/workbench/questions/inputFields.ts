import type { QuestionsData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint, validFieldText } from '../fields/value'

export const questionInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: (target) => target.destination.kind === 'question_answer',
  text(state, target) {
    const destination = target.destination
    if (destination.kind !== 'question_answer' || state.type !== 'questions') return null
    const answers = state.answers.filter((answer) => answer.id === destination.questionId)
    return answers.length === 1 && answers[0].wasCustom ? answers[0].value : null
  },
  removeAttachment(state, _attachmentId, remove) {
    if (state.type !== 'questions') return state
    let changed = false
    const answers = state.answers.map((answer) => {
      if (!answer.wasCustom) return answer
      const value = remove(answer.value), label = remove(answer.label)
      if (value === answer.value && label === answer.label) return answer
      changed = true
      return { ...answer, value, label }
    })
    return changed ? { ...state, answers } : state
  },
  read({ spec, state, target }) {
    const destination = target.destination
    if (destination.kind !== 'question_answer' || spec.type !== 'questions' || state.type !== 'questions') throw new Error('This answer is unavailable.')
    const question = (spec.data as QuestionsData).questions.find((item) => item.id === destination.questionId)
    const answers = state.answers.filter((item) => item.id === destination.questionId)
    if (!question?.allowOther || answers.length !== 1 || answers[0].wasCustom !== true || !validFieldText(answers[0].value, 4000)) {
      throw new Error('The custom answer was cleared or changed. Your input has been preserved.')
    }
    const answer = answers[0]
    return {
      value: answer.value, limit: 4000,
      contract: fieldFingerprint({ type: spec.type, version: spec.version, question }),
      identity: fieldFingerprint({ id: answer.id, wasCustom: true }),
      replace: (value) => ({ ...state, answers: state.answers.map((item) => {
        if (item !== answer) return item
        const next = { ...item, value, label: value }
        delete next.index
        return next
      }) }),
    }
  },
}]
