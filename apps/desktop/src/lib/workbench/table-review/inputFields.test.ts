import { describe, expect, it } from 'vitest'
import type { InputTarget } from '../../domain/inputTarget'
import type { TableReviewState } from '../definitions/table_review/state'
import { tableReviewDefinition } from '../definitions/table_review/definition'
import { repositoryGuideTable as data } from '../definitions/table_review/examples'
import { tableReviewInputFields } from './inputFields'

const spec = { type: 'table_review', version: 1, data }, adapter = tableReviewInputFields[0]
const state: TableReviewState = { type: 'table_review', changes: [{ row_id: 'ramble', column_id: 'steps', value: '6' }],
  comments: [{ id: 'first', row_id: 'ramble', column_id: 'steps', body: 'Check\n\n[notes](attachment://a)' }] }
const target = (field = 'comment_body', entityId = 'first', sourceVersion = data.source_version): InputTarget => ({
  requestId: 'table', requestTitle: 'Review', destination: { kind: 'workbench_field', workbenchType: 'table_review', version: 1,
    field, entityId, sourceVersion, label: 'C1 · 引导步骤数' },
})
describe('durable table fields', () => {
  it('writes speech to the owning field, preserves the cell and other review input, and restores blank first-comment fields', () => {
    const first = { ...state, comments: [{ ...state.comments[0], body: '' }] }
    const write = adapter.read({ spec, state: first, target: target() })
    expect(write.value).toBe('')
    const next = write.replace('Voice') as TableReviewState
    expect(next.comments[0]).toEqual({ ...first.comments[0], body: 'Voice' })
    expect(next.changes).toBe(state.changes)
    expect(tableReviewDefinition.complete(spec, next)).toBe(true)
    const suggestion = adapter.read({ spec, state, target: target('change_value', 'ramble:steps') })
    expect((suggestion.replace('7') as TableReviewState).changes[0]).toEqual({ ...state.changes[0], value: '7' })
    expect(adapter.text(state, target('change_value', 'ramble:steps'))).toBe('6')
  })
  it('captures source and anchor identity and refuses writes to stale, deleted, duplicate or unknown fields', () => {
    const field = adapter.read({ spec, state, target: target() })
    expect(adapter.read({ spec: { ...spec, data: { ...data, title: 'Another title' } }, state, target: target() }).contract).not.toBe(field.contract)
    expect(adapter.read({ spec, state: { ...state, comments: [{ ...state.comments[0], column_id: 'workbench' }] }, target: target() }).identity).not.toBe(field.identity)
    expect(() => adapter.read({ spec, state, target: target('comment_body', 'first', 'stale') })).toThrow('original table has changed')
    for (const broken of [{ ...state, comments: [] }, { ...state, comments: [state.comments[0], state.comments[0]] },
      { ...state, comments: [{ ...state.comments[0], row_id: 'missing' }] }])
      expect(() => adapter.read({ spec, state: broken, target: target() })).toThrow('unavailable')
    expect(adapter.accepts(target('rows[0].cells', 'ramble:steps'))).toBe(false)
    expect(adapter.text(state, target('comment_body', 'missing'))).toBeNull()
  })
  it('unlinks only shared textual attachment references without changing source coordinates', () => {
    const literal = { ...state, changes: [{ ...state.changes[0], value: '[notes](attachment://a)' }] }
    const next = adapter.removeAttachment(literal, 'a', (value) => value.replace('[notes](attachment://a)', '').trim()) as TableReviewState
    expect(next.comments[0]).toEqual({ ...state.comments[0], body: 'Check' })
    expect(next.changes[0]).toBe(literal.changes[0])
    expect(next.changes[0].value).toBe('[notes](attachment://a)')
    expect(adapter.removeAttachment(next, 'a', (value) => value)).toBe(next)
  })
})
