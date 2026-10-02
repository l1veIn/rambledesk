import { expect, it } from 'vitest'
import { ratingReviewDefinition, ratingMaterialVersion, opinionField } from './definition'
import { workbenchFieldTarget } from '../../../domain/inputTarget'
import type { WorkbenchState } from '../../../generated/feedback'

it('requires the score independently of opinion text', () => {
  const spec = ratingReviewDefinition.examples![0].spec
  expect(ratingReviewDefinition.complete(spec, { type: 'rating_review', score: null, note: 'An opinion' })).toBe(false)
  expect(ratingReviewDefinition.decodeState({ type: 'rating_review', score: 6, note: '' })).toBeNull()
  expect(ratingReviewDefinition.accepts({ ...spec.data, future: 'preserve as unknown' })).toBe(false)
  expect(ratingReviewDefinition.decodeState({ type: 'rating_review', score: 4, note: '', future: 'preserve' })).toBeNull()
  expect(ratingReviewDefinition.decodeState({ type: 'rating_review', note: 'An unfinished opinion' })).toEqual({ type: 'rating_review', score: null, note: 'An unfinished opinion' })
  expect(ratingReviewDefinition.complete(spec, { type: 'rating_review', score: 4, note: '' })).toBe(true)
})
it('rejects changed material and preserves score when replacing opinion', () => {
  const spec = ratingReviewDefinition.examples![0].spec
  const state: WorkbenchState = { type: 'rating_review', score: 4, note: 'Before' }
  const target = workbenchFieldTarget({ requestId: 'r', requestTitle: 'Review' }, { workbenchType: 'rating_review', version: 1,
    field: 'opinion', entityId: 'review', sourceVersion: ratingMaterialVersion(spec.data), label: 'Opinion' })
  expect(opinionField.read({ spec, state, target }).replace('After')).toEqual({ ...state, note: 'After' })
  expect(() => opinionField.read({ spec: { ...spec, data: { title: 'Changed', material: 'Changed' } }, state, target })).toThrow(/changed/)
})
