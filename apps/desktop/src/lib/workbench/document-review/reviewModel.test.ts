import { describe, expect, it } from 'vitest'
import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
import { annotatedSegments, changeParagraphMark, emptyReviewState, hasReviewInput, validateReviewState } from './reviewModel'

const data: DocumentReviewData = { title: 'Speech', source_version: 'draft-1', paragraphs: [{ id: 'opening', text: '开场😀很好。' }] }
const note: ReviewAnnotation = { id: 'note-1', paragraph_id: 'opening', start: 2, end: 5, quote: '😀很好', kind: 'suggestion', body: 'Make the claim specific.', replacement: '有一个好处', status: 'open' }

describe('immutable document review contract', () => {
  it('requires an explicit decision but permits a review with no changes or prose', () => {
    expect(validateReviewState(data, emptyReviewState())).not.toBeNull()
    const ready = { ...emptyReviewState(), verdict: 'ready' as const }
    expect(hasReviewInput(data, ready)).toBe(true)
    expect(validateReviewState(data, ready)).toBeNull()
  })
  it('validates Unicode scalar ranges against the immutable source, including emoji', () => {
    const state = { ...emptyReviewState(), verdict: 'changes_requested' as const, annotations: [note] }
    expect(validateReviewState(data, state)).toBeNull()
    for (const bad of [{ ...note, end: 6 }, { ...note, quote: 'different' }, { ...note, start: null }, { ...note, paragraph_id: 'missing' }]) {
      expect(validateReviewState(data, { ...state, annotations: [bad] })).not.toBeNull()
    }
    expect(data.paragraphs[0].text).toBe('开场😀很好。')
  })
  it('allows an explicit deletion suggestion, and blocks incomplete or duplicate comments', () => {
    const state = { ...emptyReviewState(), verdict: 'changes_requested' as const, annotations: [{ ...note, replacement: '' }] }
    expect(validateReviewState(data, state)).toBeNull()
    expect(validateReviewState(data, { ...state, annotations: [{ ...note, body: '  ' }] })).not.toBeNull()
    expect(validateReviewState(data, { ...state, annotations: [note, note] })).not.toBeNull()
    expect(validateReviewState(data, { ...state, annotations: [{ ...note, kind: 'comment' }] })).not.toBeNull()
  })
  it('replaces and clears paragraph marks without changing annotations', () => {
    const original = { ...emptyReviewState(), annotations: [note] }
    const marked = changeParagraphMark(changeParagraphMark(original, 'opening', 'keep'), 'opening', 'revise')
    expect(marked.paragraph_marks).toEqual([{ paragraph_id: 'opening', decision: 'revise' }])
    expect(changeParagraphMark(marked, 'opening', '').paragraph_marks).toEqual([])
    expect(marked.annotations).toEqual([note])
  })
  it('renders overlapping highlights without losing or duplicating source characters', () => {
    const overlapping = { ...note, id: 'note-2', start: 3, end: 6, quote: '很好。', status: 'resolved' as const }
    const segments = annotatedSegments(data.paragraphs[0].text, [note, overlapping])
    expect(segments.map((segment) => segment.text).join('')).toBe(data.paragraphs[0].text)
    expect(segments.find((segment) => segment.text === '很好')?.annotationIds).toEqual(['note-1', 'note-2'])
  })
})
