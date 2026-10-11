import { describe, expect, it } from 'vitest'
import type { BrainstormData } from '../../generated/feedback'
import { brainstormDefinition as definition } from '../definitions/brainstorm/definition'
import { brainstormExamples } from '../definitions/brainstorm/examples'
import { validBrainstormInput } from '../definitions/brainstorm/input'
import { readBrainstormState } from '../definitions/brainstorm/state'
import { addBrainstormCard, initialBrainstormState, parkBrainstormCard, validateBrainstormDraft, validateBrainstormState } from './brainstormModel'

const spec = brainstormExamples[1].spec, data = spec.data as BrainstormData
describe('brainstorm snapshots and user ownership', () => {
  it('accepts resolved examples and clones every inherited choice without preselecting a next action', () => {
    for (const example of brainstormExamples) expect(validBrainstormInput(example.spec.data)).toBe(true)
    const state = initialBrainstormState(data)
    expect(state).toEqual({ type: 'brainstorm', ...data.initial_board, intent: null, instruction: '' })
    state.cards[0].body = 'My edit'
    state.selected_card_ids.pop()
    expect(data.initial_board.cards[0].body).not.toBe('My edit')
    expect(data.initial_board.selected_card_ids).toHaveLength(2)
    expect(definition.complete(spec, initialBrainstormState(data))).toBe(false)
  })
  it('rejects raw Agent input, forged round identity, malformed IDs and unknown fields', () => {
    expect(validBrainstormInput({ title: 'Raw input', cards: [], groups: [], inherit_from_request_id: null })).toBe(false)
    for (const patch of [{ source_version: data.source_version + '\n' }, { source_version: data.source_version.toUpperCase() },
      { round: 0 }, { round: 4294967296 }, { round: 1 }, { inherit_from_request_id: data.source_version }, { injected: true }])
      expect(validBrainstormInput({ ...data, ...patch })).toBe(false)
    const state = initialBrainstormState(data)
    expect(readBrainstormState({ ...state, cards: [{ ...state.cards[0], id: 'bad\n' }] })).toBeNull()
    expect(readBrainstormState({ ...state, cards: [...state.cards, state.cards[0]] })).toBeNull()
    expect(readBrainstormState({ ...state, phase: 'wizard' })).toBeNull()
  })
  it('preserves inherited identities while allowing text, grouping and view changes', () => {
    const state = initialBrainstormState(data)
    state.cards[0] = { ...state.cards[0], title: 'Human changed this', body: 'Keep my words', group_id: null }
    state.groups = []
    state.cards = state.cards.map((card) => ({ ...card, group_id: null }))
    state.phase = 'focus'; state.intent = 'proceed'
    expect(validateBrainstormState(data, state)).toBeNull()
    expect(definition.complete(spec, state)).toBe(true)
    expect(validateBrainstormDraft(data, { ...state, cards: state.cards.slice(1) })).not.toBeNull()
    expect(validateBrainstormDraft(data, { ...state, cards: state.cards.map((card, index) => index === 0 ? { ...card, origin: 'human' } : card) })).not.toBeNull()
    expect(validateBrainstormDraft(data, { ...state, cards: state.cards.map((card, index) => index === 0 ? { ...card, source_card_ids: [state.cards[1].id] } : card) })).not.toBeNull()
  })
  it('requires an explicit action and enough selected active cards', () => {
    const state = { ...initialBrainstormState(data), selected_card_ids: [] }
    expect(validateBrainstormState(data, state)).toBe('Choose what the agent should do next.')
    expect(validateBrainstormState(data, { ...state, intent: 'explore' })).toBeNull()
    for (const intent of ['expand', 'proceed'] as const)
      expect(validateBrainstormState(data, { ...state, intent })).toBe('Select at least one idea.')
    expect(validateBrainstormState(data, { ...state, intent: 'combine', selected_card_ids: [state.cards[0].id] })).toBe('Select at least two ideas to combine.')
    expect(validateBrainstormState(data, { ...state, intent: 'combine', selected_card_ids: state.cards.slice(0, 2).map((card) => card.id) })).toBeNull()
  })
  it('parks without deleting text or provenance and removes only that selection', () => {
    const state = initialBrainstormState(data), id = state.selected_card_ids[0]
    const parked = parkBrainstormCard(state, id, true)
    expect(parked.cards.find((card) => card.id === id)).toEqual({ ...state.cards.find((card) => card.id === id), parked: true })
    expect(parked.selected_card_ids).toEqual(state.selected_card_ids.slice(1))
    expect(readBrainstormState({ ...parked, selected_card_ids: [id] })).toBeNull()
    expect(parkBrainstormCard(parked, id, false).cards).toEqual(state.cards)
  })
  it('allows empty drafts but requires titles at submission and protects new human origin', () => {
    const state = addBrainstormCard(data, initialBrainstormState(data), 'my-new-card', null)
    expect(state.cards.at(-1)).toMatchObject({ origin: 'human', origin_request_id: data.source_version, source_card_ids: [] })
    expect(readBrainstormState(state)).not.toBeNull()
    expect(validateBrainstormDraft(data, state)).toBeNull()
    expect(validateBrainstormState(data, { ...state, intent: 'explore' })).toBe('Give every idea and group a title before sending.')
    expect(validateBrainstormDraft(data, { ...state, cards: state.cards.map((card) => card.id === 'my-new-card' ? { ...card, origin: 'agent' } : card) })).not.toBeNull()
    const nonblank = { ...state, cards: state.cards.map((card) => ({ ...card, title: card.title || '😀'.repeat(200) })) }
    expect(readBrainstormState(nonblank)).not.toBeNull()
    expect(readBrainstormState({ ...nonblank, instruction: '\uD800' })).toBeNull()
    expect(readBrainstormState({ ...nonblank, instruction: 'x'.repeat(4001) })).toBeNull()
  })
  it('keeps card, group and lineage limits strict', () => {
    const state = initialBrainstormState(data)
    expect(readBrainstormState({ ...state, cards: Array.from({ length: 201 }, (_, i) => ({ ...state.cards[0], id: `idea-${i}` })) })).toBeNull()
    expect(readBrainstormState({ ...state, groups: Array.from({ length: 31 }, (_, i) => ({ id: `group-${i}`, title: 'Group' })) })).toBeNull()
    expect(readBrainstormState({ ...state, cards: state.cards.map((card, index) => index ? card : { ...card, group_id: 'missing' }) })).toBeNull()
    expect(readBrainstormState({ ...state, cards: state.cards.map((card, index) => index ? card : { ...card, source_card_ids: ['missing'] }) })).toBeNull()
  })
})
