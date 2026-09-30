import { object, fields, list, text, optionalText, uniqueId, type RecordValue } from '../validation'

export function validSingleChoiceInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['prompt', 'options']) && text(data.prompt, 2000) && list(data.options, 2, 20) &&
    data.options.every((option) => object(option) && fields(option, ['id', 'label']) && uniqueId(option.id, ids) && text(option.label, 2000))
}
