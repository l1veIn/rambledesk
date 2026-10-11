import type { BrainstormBoard, BrainstormData } from '../../../generated/feedback'
import { fields, list, object, text } from '../validation'

export function brainstormId(value: unknown): value is string {
  return typeof value === 'string' && value.length <= 80 && /^[a-z0-9]/.test(value) && !/[^a-z0-9_-]/.test(value)
}
export const requestId = (value: unknown): value is string => typeof value === 'string' && value.length === 36
  && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
const uniqueIds = (value: unknown, max: number): value is string[] => list(value, 0, max)
  && value.every(brainstormId) && new Set(value).size === value.length

export function validBrainstormBoard(value: unknown, requireTitles = false): value is BrainstormBoard {
  if (!object(value) || !fields(value, ['cards', 'groups', 'selected_card_ids', 'phase'])
    || !list(value.cards, 0, 200) || !list(value.groups, 0, 30) || !uniqueIds(value.selected_card_ids, 200)
    || !['diverge', 'organize', 'focus'].includes(String(value.phase))) return false
  const groups = new Set<string>(), cards = new Set<string>()
  if (!value.groups.every((group) => {
    if (!object(group) || !fields(group, ['id', 'title']) || !brainstormId(group.id)
      || groups.has(group.id) || !text(group.title, 200, requireTitles)) return false
    groups.add(group.id); return true
  })) return false
  if (!value.cards.every((card) => {
    if (!object(card) || !fields(card, ['id', 'title', 'body', 'group_id', 'source_card_ids', 'origin', 'origin_request_id', 'parked'])
      || !brainstormId(card.id) || cards.has(card.id) || !text(card.title, 200, requireTitles) || !text(card.body, 4000, false)
      || !(card.group_id === null || (typeof card.group_id === 'string' && groups.has(card.group_id)))
      || !uniqueIds(card.source_card_ids, 20) || !['agent', 'human'].includes(String(card.origin))
      || !requestId(card.origin_request_id) || typeof card.parked !== 'boolean') return false
    cards.add(card.id); return true
  })) return false
  const entries = value.cards
  return value.selected_card_ids.every((id) => entries.some((card) => object(card) && card.id === id && !card.parked))
    && entries.every((card) => object(card) && (card.source_card_ids as string[]).every((id) => cards.has(id) && id !== card.id))
}

/** Only server-resolved boards are editable; raw Agent request data must be hydrated first. */
export function validBrainstormInput(value: unknown): value is BrainstormData {
  return object(value) && fields(value, ['title', 'source_version', 'board_id', 'round', 'inherit_from_request_id', 'initial_board'])
    && text(value.title, 200) && requestId(value.source_version) && requestId(value.board_id)
    && Number.isSafeInteger(value.round) && Number(value.round) >= 1 && Number(value.round) <= 4294967295
    && (value.inherit_from_request_id === null || (requestId(value.inherit_from_request_id) && value.inherit_from_request_id !== value.source_version))
    && ((value.round === 1) === (value.inherit_from_request_id === null))
    && (value.round !== 1 || value.board_id === value.source_version)
    && validBrainstormBoard(value.initial_board, true)
}
