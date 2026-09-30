import { object, fields, list, text, optionalText, uniqueId, type RecordValue } from '../validation'

export function validQuestionsInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['questions']) && list(data.questions, 1, 20) && data.questions.every((question) => {
    if (!object(question) || !fields(question, ['id', 'prompt', 'label', 'options', 'allowOther']) ||
      !uniqueId(question.id, ids) || !text(question.prompt, 2000) || !optionalText(question.label, 40) ||
      (question.allowOther !== undefined && typeof question.allowOther !== 'boolean') || !list(question.options, 2, 6)) return false
    const values = new Set<string>()
    return question.options.every((option) => {
      if (!object(option) || !fields(option, ['value', 'label', 'description']) || !text(option.value, 64) ||
        values.has(option.value) || !text(option.label, 2000) || !optionalText(option.description, 2000)) return false
      values.add(option.value)
      return true
    })
  })
}

