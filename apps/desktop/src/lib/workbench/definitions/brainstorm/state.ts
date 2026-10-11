import type { WorkbenchState } from '../../../generated/feedback'
import { fields, object, text } from '../validation'
import { validBrainstormBoard } from './input'

export type BrainstormState = Extract<WorkbenchState, { type: 'brainstorm' }>
export function readBrainstormState(value: unknown): BrainstormState | null {
  if (!object(value) || !fields(value, ['type', 'cards', 'groups', 'selected_card_ids', 'phase', 'intent', 'instruction'])
    || value.type !== 'brainstorm' || !(value.intent === null || ['explore', 'expand', 'combine', 'proceed'].includes(String(value.intent)))
    || !text(value.instruction, 4000, false)) return null
  const board = { cards: value.cards, groups: value.groups, selected_card_ids: value.selected_card_ids, phase: value.phase }
  return validBrainstormBoard(board) ? structuredClone(value) as BrainstormState : null
}
