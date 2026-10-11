import type { BrainstormData } from '../../generated/feedback'
import type { WorkbenchFieldAdapter } from '../fields/contracts'
import { fieldFingerprint } from '../fields/value'
import { validBrainstormInput } from '../definitions/brainstorm/input'
import { readBrainstormState } from '../definitions/brainstorm/state'
import { validateBrainstormDraft } from './brainstormModel'

export const brainstormInputFields: readonly WorkbenchFieldAdapter[] = [{
  accepts: ({ destination }) => destination.kind === 'workbench_field' && destination.workbenchType === 'brainstorm'
    && destination.version === 1 && ['card_body', 'instruction'].includes(destination.field),
  text(state, { destination }) {
    if (state.type !== 'brainstorm' || destination.kind !== 'workbench_field' || destination.workbenchType !== 'brainstorm' || destination.version !== 1) return null
    if (destination.field === 'instruction') return destination.entityId === 'round' ? state.instruction : null
    const cards = state.cards.filter((card) => card.id === destination.entityId)
    return destination.field === 'card_body' && cards.length === 1 ? cards[0].body : null
  },
  removeAttachment(state, _id, removeText) {
    if (state.type !== 'brainstorm') return state
    const cards = state.cards.map((card) => { const body = removeText(card.body); return body === card.body ? card : { ...card, body } })
    const instruction = removeText(state.instruction)
    return instruction === state.instruction && cards.every((card, index) => card === state.cards[index]) ? state : { ...state, cards, instruction }
  },
  read({ spec, state, target }) {
    const destination = target.destination, draft = readBrainstormState(state)
    if (spec.type !== 'brainstorm' || spec.version !== 1 || !validBrainstormInput(spec.data) || !draft
      || destination.kind !== 'workbench_field' || destination.workbenchType !== 'brainstorm' || destination.version !== 1
      || destination.sourceVersion !== spec.data.source_version || validateBrainstormDraft(spec.data, draft))
      throw new Error('This brainstorm round has changed. Select the field again.')
    const data: BrainstormData = spec.data
    const card = destination.field === 'card_body' ? draft.cards.find((item) => item.id === destination.entityId) : null
    const instruction = destination.field === 'instruction' && destination.entityId === 'round'
    if (!instruction && !card) throw new Error('This idea field is unavailable.')
    return {
      value: instruction ? draft.instruction : card!.body, limit: 4000,
      contract: fieldFingerprint({ type: spec.type, version: spec.version, data }),
      identity: fieldFingerprint(instruction ? { field: 'instruction' } : { id: card!.id, origin: card!.origin,
        origin_request_id: card!.origin_request_id, source_card_ids: card!.source_card_ids }),
      replace: (value) => instruction ? { ...draft, instruction: value }
        : { ...draft, cards: draft.cards.map((item) => item.id === card!.id ? { ...item, body: value } : item) },
    }
  },
}]
