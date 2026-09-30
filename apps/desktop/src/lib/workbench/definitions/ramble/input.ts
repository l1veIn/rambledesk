import { object, fields, list, text, optionalText, uniqueId, type RecordValue } from '../validation'

export function validRambleInput(data: RecordValue): boolean {
  const ids = new Set<string>()
  return fields(data, ['actions']) && list(data.actions, 1, 20) && data.actions.every((action) =>
    object(action) && fields(action, ['id', 'instruction']) && uniqueId(action.id, ids) && text(action.instruction, 2000))
}
