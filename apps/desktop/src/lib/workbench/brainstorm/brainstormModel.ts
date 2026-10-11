import type { BrainstormCard, BrainstormData, BrainstormIntent } from '../../generated/feedback'
import { validBrainstormBoard } from '../definitions/brainstorm/input'
import type { BrainstormState } from '../definitions/brainstorm/state'
import { text } from '../definitions/validation'

export function initialBrainstormState(data: BrainstormData): BrainstormState {
  return { type: 'brainstorm', ...structuredClone(data.initial_board), intent: null, instruction: '' }
}
export function validateBrainstormDraft(data: BrainstormData, state: BrainstormState): string | null {
  const { cards, groups, selected_card_ids, phase } = state
  if (!validBrainstormBoard({ cards, groups, selected_card_ids, phase })) return 'This brainstorm draft is invalid.'
  const original = new Map(data.initial_board.cards.map((card) => [card.id, card]))
  if (data.initial_board.cards.some((card) => !cards.some((current) => current.id === card.id))) return 'Keep inherited cards. Park ideas you do not need.'
  for (const card of cards) {
    const previous = original.get(card.id)
    if (previous ? card.origin !== previous.origin || card.origin_request_id !== previous.origin_request_id
      || JSON.stringify(card.source_card_ids) !== JSON.stringify(previous.source_card_ids)
      : card.origin !== 'human' || card.origin_request_id !== data.source_version || card.source_card_ids.length !== 0)
      return 'The origin of an idea cannot be changed.'
  }
  return null
}
export function validateBrainstormState(data: BrainstormData, state: BrainstormState | null): string | null {
  if (!state) return 'Choose what the agent should do next.'
  const draftError = validateBrainstormDraft(data, state)
  if (draftError) return draftError
  if (!state.cards.every((card) => text(card.title, 200)) || !state.groups.every((group) => text(group.title, 200)))
    return 'Give every idea and group a title before sending.'
  if (!state.intent) return 'Choose what the agent should do next.'
  const minimum = state.intent === 'combine' ? 2 : state.intent === 'explore' ? 0 : 1
  if (state.selected_card_ids.length < minimum) return minimum === 2 ? 'Select at least two ideas to combine.' : 'Select at least one idea.'
  return null
}
export function hasBrainstormInput(_data: BrainstormData, state: BrainstormState | null): boolean {
  return state !== null
}
export function addBrainstormCard(data: BrainstormData, state: BrainstormState, id: string, groupId: string | null): BrainstormState {
  if (state.cards.length >= 200 || state.cards.some((card) => card.id === id)) return state
  const card: BrainstormCard = { id, title: '', body: '', group_id: groupId, source_card_ids: [], origin: 'human', origin_request_id: data.source_version, parked: false }
  return { ...state, cards: [...state.cards, card] }
}
export function parkBrainstormCard(state: BrainstormState, id: string, parked: boolean): BrainstormState {
  return { ...state, cards: state.cards.map((card) => card.id === id ? { ...card, parked } : card),
    selected_card_ids: parked ? state.selected_card_ids.filter((value) => value !== id) : state.selected_card_ids }
}
export function chooseBrainstormIntent(state: BrainstormState, intent: BrainstormIntent): BrainstormState {
  return { ...state, intent }
}
