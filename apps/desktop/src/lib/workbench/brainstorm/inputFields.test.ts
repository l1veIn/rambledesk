import { describe, expect, it } from 'vitest'
import type { BrainstormData } from '../../generated/feedback'
import { workbenchFieldTarget } from '../../domain/inputTarget'
import { brainstormExamples } from '../definitions/brainstorm/examples'
import { initialBrainstormState } from './brainstormModel'
import { brainstormInputFields } from './inputFields'
import type { BrainstormState } from '../definitions/brainstorm/state'
import { applySpeechWriteback } from '../../speech/speechWriteback'
import { readWorkbenchState } from '../../workbenchState'
import { updateFeedbackDraftState, snapshotFeedbackDraftMarkdown } from '../../feedbackDraftDocument'

const spec = brainstormExamples[1].spec, data = spec.data as BrainstormData, adapter = brainstormInputFields[0]
const target = (id = data.initial_board.cards[0].id, field = 'card_body', sourceVersion = data.source_version) => workbenchFieldTarget(
  { requestId: data.source_version, requestTitle: data.title }, { workbenchType: 'brainstorm', version: 1, field, entityId: id, sourceVersion, label: 'Idea' })
describe('brainstorm generic input fields', () => {
  it('writes only the selected card body and keeps human grouping, parking, selection and provenance', () => {
    const state = initialBrainstormState(data), field = adapter.read({ spec, state, target: target() })
    const next = field.replace('Spoken idea\n\n![evidence](attachment://proof)') as BrainstormState
    expect(next.cards[0]).toEqual({ ...state.cards[0], body: 'Spoken idea\n\n![evidence](attachment://proof)' })
    expect(next.cards.slice(1)).toEqual(state.cards.slice(1))
    expect(next.groups).toEqual(state.groups)
    expect(next.selected_card_ids).toEqual(state.selected_card_ids)
    const moved = { ...state, cards: state.cards.map((card) => ({ ...card, group_id: null })) }
    expect(adapter.read({ spec, state: moved, target: target() }).identity).toBe(field.identity)
    expect(adapter.read({ spec, state: park(state), target: target() }).identity).toBe(field.identity)
  })
  it('rejects stale rounds, removed or duplicated entities and unknown fields', () => {
    const state = initialBrainstormState(data)
    expect(() => adapter.read({ spec, state, target: target(state.cards[0].id, 'card_body', data.board_id) })).toThrow('round has changed')
    expect(() => adapter.read({ spec, state, target: target('missing') })).toThrow('unavailable')
    expect(() => adapter.read({ spec, state: { ...state, cards: [...state.cards, state.cards[0]] }, target: target() })).toThrow('round has changed')
    expect(() => target('x', 'cards[0].body')).toThrow('Invalid workbench input target')
    expect(adapter.accepts(target('x', 'unowned_field'))).toBe(false)
    expect(adapter.text(state, target('missing'))).toBeNull()
    expect(adapter.text(state, target('wrong', 'instruction'))).toBeNull()
  })
  it('keeps round instruction independent and unlinks owned body references without touching titles or lineage', () => {
    const state = initialBrainstormState(data)
    state.cards[0] = { ...state.cards[0], title: 'literal attachment://proof', body: 'Keep\n\n[proof](attachment://proof)' }
    state.instruction = '[proof](attachment://proof)'
    const field = adapter.read({ spec, state, target: target('round', 'instruction') })
    expect((field.replace('Please combine') as BrainstormState).cards).toEqual(state.cards)
    const next = adapter.removeAttachment(state, 'proof', (value) => value.replace('[proof](attachment://proof)', '').trim()) as BrainstormState
    expect(next.cards[0]).toEqual({ ...state.cards[0], body: 'Keep' })
    expect(next.instruction).toBe('')
    expect(adapter.removeAttachment(next, 'proof', (value) => value)).toBe(next)
  })
  it('routes actual shared speech writeback into the card while preserving feedback notes and another field', () => {
    const state = initialBrainstormState(data), oldBody = state.cards[0].body
    const draft = updateFeedbackDraftState(snapshotFeedbackDraftMarkdown('Overall notes'), state)
    const next = applySpeechWriteback({ request: { request_id: data.source_version, status: 'in_progress' }, workbench: spec,
      draft: { document_json: draft.documentJson, body_markdown: draft.bodyMarkdown } },
    { ...target(), id: 'speech-idea', text: 'Spoken addition', cleanupState: 'pending' })
    const result = readWorkbenchState(next.documentJson) as BrainstormState
    expect(result.cards[0].body).toBe(oldBody + '\nSpoken addition')
    expect(result.cards[1]).toEqual(state.cards[1])
    expect(next.bodyMarkdown).toBe('Overall notes')
  })
})
function park(state: BrainstormState): BrainstormState {
  return { ...state, selected_card_ids: [], cards: state.cards.map((card) => ({ ...card, parked: true })) }
}
