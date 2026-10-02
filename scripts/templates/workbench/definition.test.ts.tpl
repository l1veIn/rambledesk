import { expect, it } from 'vitest'
import { __camel__Definition, __camel__MaterialVersion, opinionField } from './definition'
import { workbenchFieldTarget } from '../../../domain/inputTarget'
import type { WorkbenchState } from '../../../generated/feedback'

it('requires the score independently of opinion text', () => {
  const spec = __camel__Definition.examples![0].spec
  expect(__camel__Definition.complete(spec, { type: '__snake__', score: null, note: 'An opinion' })).toBe(false)
  expect(__camel__Definition.decodeState({ type: '__snake__', score: 6, note: '' })).toBeNull()
  expect(__camel__Definition.accepts({ ...spec.data, future: 'preserve as unknown' })).toBe(false)
  expect(__camel__Definition.decodeState({ type: '__snake__', score: 4, note: '', future: 'preserve' })).toBeNull()
  expect(__camel__Definition.decodeState({ type: '__snake__', note: 'An unfinished opinion' })).toEqual({ type: '__snake__', score: null, note: 'An unfinished opinion' })
  expect(__camel__Definition.complete(spec, { type: '__snake__', score: 4, note: '' })).toBe(true)
})
it('rejects changed material and preserves score when replacing opinion', () => {
  const spec = __camel__Definition.examples![0].spec
  const state: WorkbenchState = { type: '__snake__', score: 4, note: 'Before' }
  const target = workbenchFieldTarget({ requestId: 'r', requestTitle: 'Review' }, { workbenchType: '__snake__', version: 1,
    field: 'opinion', entityId: 'review', sourceVersion: __camel__MaterialVersion(spec.data), label: 'Opinion' })
  expect(opinionField.read({ spec, state, target }).replace('After')).toEqual({ ...state, note: 'After' })
  expect(() => opinionField.read({ spec: { ...spec, data: { title: 'Changed', material: 'Changed' } }, state, target })).toThrow(/changed/)
})
