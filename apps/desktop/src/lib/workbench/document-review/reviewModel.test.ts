import { describe, expect, it } from 'vitest'
import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
import { emptyReviewState, hasReviewInput, prepareAnnotation, validateReviewState } from './reviewModel'

const data: DocumentReviewData = { title: 'Speech', source_version: 'draft-1', markdown: '# 开场😀很好。\r\n\r\n**下一段**' }
const note: ReviewAnnotation = { id: 'note-1', start: 4, end: 7, quote: '😀很好', kind: 'suggestion', body: 'Make it specific.', replacement: '一个好处' }
const whole = { start: null, end: null, quote: null }
const complete = (annotations: ReviewAnnotation[]) => ({ ...emptyReviewState(), verdict: 'changes_requested' as const, annotations })

describe('immutable Markdown review', () => {
  it('requires an explicit decision and permits a review with no changes', () => {
    expect(validateReviewState(data, emptyReviewState())).not.toBeNull()
    const ready = { ...emptyReviewState(), verdict: 'ready' as const }
    expect(hasReviewInput(data, ready)).toBe(true)
    expect(validateReviewState(data, ready)).toBeNull()
  })
  it('validates scalar offsets and exact raw Markdown, not rendered text', () => {
    expect(validateReviewState(data, complete([note]))).toBeNull()
    for (const bad of [{ ...note, end: 8 }, { ...note, quote: 'different' }, { ...note, start: null }, { ...note, start: 4.5 }]) {
      expect(validateReviewState(data, complete([bad]))).not.toBeNull()
    }
    const quote = '很好。\r\n\r\n**下一段'
    expect(validateReviewState(data, complete([{ ...note, start: 5, end: 5 + [...quote].length, quote }]))).toBeNull()
  })
  it('allows optional suggestion reasons but rejects malformed/duplicate identities', () => {
    expect(validateReviewState(data, complete([{ ...note, replacement: '' }]))).toBeNull()
    expect(validateReviewState(data, complete([{ ...note, body: '', replacement: note.quote }]))).toBe('Change the wording or add a reason before finishing.')
    for (const body of ['', '  ', '\u0085']) {
      expect(validateReviewState(data, complete([{ ...note, body, replacement: '' }]))).toBeNull()
      expect(hasReviewInput(data, { ...emptyReviewState(), annotations: [{ ...note, body, replacement: '' }] })).toBe(true)
      expect(validateReviewState(data, complete([{ ...note, kind: 'comment', body, replacement: null }]))).not.toBeNull()
    }
    for (const bad of [{ ...note, body: '\ud800' },
      { ...note, id: 'note\n' }, { ...note, kind: 'comment' as const }]) {
      expect(validateReviewState(data, complete([bad]))).not.toBeNull()
    }
    expect(validateReviewState(data, complete([note, note]))).not.toBeNull()
  })
  it('creates independent notes for the same or overlapping range without altering old contents', () => {
    const existing = complete([note])
    const created = prepareAnnotation(data, existing, note, 'comment', () => 'note-2')!
    expect(created.state.annotations).toHaveLength(2)
    expect(created.state.annotations[0]).toBe(note)
    expect(created.annotation).toEqual({ ...note, id: 'note-2', kind: 'comment', body: '', replacement: null })
    expect(existing.annotations).toEqual([note])
  })
  it('seeds exact raw suggestions and accepts explicit deletion without modifying Markdown', () => {
    expect(prepareAnnotation(data, emptyReviewState(), note, 'suggestion', () => 'rewrite')!.annotation.replacement).toBe(note.quote)
    expect(prepareAnnotation(data, emptyReviewState(), note, 'suggestion', () => 'delete', true)!.annotation.replacement).toBe('')
    expect(prepareAnnotation(data, emptyReviewState(), whole, 'comment', () => 'all')!.annotation).toMatchObject(whole)
    const large = { ...data, markdown: 'a'.repeat(8001) }
    expect(prepareAnnotation(large, emptyReviewState(), whole, 'suggestion', () => 'large')).toBeNull()
    expect(data.markdown).toBe('# 开场😀很好。\r\n\r\n**下一段**')
  })
  it('enforces the 500-note limit without merging distinct feedback', () => {
    const state = complete(Array.from({ length: 500 }, (_, index) => ({ ...note, id: `note-${index}` })))
    expect(validateReviewState(data, state)).toBeNull()
    expect(prepareAnnotation(data, state, note, 'comment', () => 'overflow')).toBeNull()
  })
})
