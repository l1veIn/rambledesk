import { describe, expect, it } from 'vitest'
import type { DocumentReviewData, ReviewAnnotation } from '../../generated/feedback'
import { annotatedSegments, changeParagraphMark, emptyReviewState, findParagraphAnnotation, hasReviewInput, prepareParagraphAnnotation, toggleParagraphRemoval, validateReviewState } from './reviewModel'

const data: DocumentReviewData = { title: 'Speech', source_version: 'draft-1', paragraphs: [{ id: 'opening', text: '开场😀很好。' }] }
const note: ReviewAnnotation = { id: 'note-1', paragraph_id: 'opening', start: 2, end: 5, quote: '😀很好', kind: 'suggestion', body: 'Make the claim specific.', replacement: '有一个好处' }
const paragraphAnchor = { paragraph_id: 'opening', start: null, end: null, quote: null }

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
  it('toggles deletion independently of comments, other paragraphs and the immutable source', () => {
    const original = { ...emptyReviewState(), annotations: [note], paragraph_marks: [{ paragraph_id: 'ending', decision: 'remove' as const }] }
    const removed = toggleParagraphRemoval(original, 'opening')
    expect(removed.paragraph_marks).toEqual([...original.paragraph_marks, { paragraph_id: 'opening', decision: 'remove' }])
    expect(toggleParagraphRemoval(removed, 'opening')).toEqual(original)
    expect(removed.annotations).toBe(original.annotations)
    expect(original.paragraph_marks).toHaveLength(1)
    expect(data.paragraphs[0].text).toBe('开场😀很好。')
    for (const decision of ['keep', 'revise'] as const) {
      expect(toggleParagraphRemoval(changeParagraphMark(original, 'opening', decision), 'opening').paragraph_marks)
        .toEqual(removed.paragraph_marks)
    }
  })
  it('renders overlapping highlights without losing or duplicating source characters', () => {
    const overlapping = { ...note, id: 'note-2', start: 3, end: 6, quote: '很好。' }
    const segments = annotatedSegments(data.paragraphs[0].text, [note, overlapping])
    expect(segments.map((segment) => segment.text).join('')).toBe(data.paragraphs[0].text)
    expect(segments.find((segment) => segment.text === '很好')?.annotationIds).toEqual(['note-1', 'note-2'])
  })
})

describe('one ongoing paragraph annotation', () => {
  it('creates a note only for the first comment and reuses its body, identity and anchor thereafter', () => {
    const first = prepareParagraphAnnotation(data, emptyReviewState(), paragraphAnchor, 'comment', () => 'created')!
    expect(first.created).toBe(true)
    expect(first.annotation).toEqual({ ...paragraphAnchor, id: 'created', kind: 'comment', body: '', replacement: null })
    const written = { ...first.annotation, body: 'First thought\n[file](attachment://attachment-1)' }
    const state = { ...first.state, annotations: [written] }
    const next = prepareParagraphAnnotation(data, state, note, 'comment', () => { throw new Error('must not create another ID') })!
    expect(next.created).toBe(false)
    expect(next.state).toBe(state)
    expect(next.annotation).toBe(written)
    expect(next.annotation).toEqual(written)
    expect(next.state.annotations).toEqual([written])
    expect(state.annotations).toEqual([written])
  })

  it('preserves historic notes and chooses the selected note, then the first note', () => {
    const first = { ...note, id: 'first' }
    const second = { ...note, id: 'second' }
    const unrelated = { ...note, id: 'unrelated', paragraph_id: 'ending' }
    const annotations = [first, note, second, unrelated]
    expect(findParagraphAnnotation(annotations, 'opening', 'first')).toBe(first)
    expect(findParagraphAnnotation(annotations, 'opening', 'second')).toBe(second)
    expect(findParagraphAnnotation(annotations, 'opening', 'unrelated')).toBe(first)
    expect(findParagraphAnnotation([first], 'opening')).toBe(first)
    expect(findParagraphAnnotation(annotations, 'missing')).toBeUndefined()
    const state = { ...emptyReviewState(), annotations }
    const next = prepareParagraphAnnotation(data, state, paragraphAnchor, 'comment', () => 'unused', 'first')!
    expect(next.annotation).toBe(first)
    expect(next.state).toBe(state)
    expect(next.state.annotations).toEqual([first, note, second, unrelated])
  })

  it('adds suggested wording to the original note without replacing the original selection or body', () => {
    const existing: ReviewAnnotation = { ...note, kind: 'comment', replacement: null }
    const state = { ...emptyReviewState(), annotations: [existing] }
    const next = prepareParagraphAnnotation(data, state, paragraphAnchor, 'suggestion', () => 'unused')!
    expect(next.annotation).toEqual({ ...existing, kind: 'suggestion', replacement: existing.quote })
    expect(next.state.annotations).toHaveLength(1)
    const paragraphNote = { ...existing, ...paragraphAnchor }
    const selectedPassage = prepareParagraphAnnotation(data, { ...state, annotations: [paragraphNote] }, note, 'suggestion', () => 'unused')!
    expect(selectedPassage.annotation).toEqual({ ...paragraphNote, kind: 'suggestion', replacement: data.paragraphs[0].text })
  })

  it('keeps previously entered suggestions, including an intentional empty replacement', () => {
    for (const replacement of [note.replacement, '']) {
      const existing = { ...note, replacement }
      for (const kind of ['comment', 'suggestion'] as const) {
        const next = prepareParagraphAnnotation(data, { ...emptyReviewState(), annotations: [existing] }, paragraphAnchor, kind, () => 'unused')!
        expect(next.annotation).toEqual(existing)
      }
    }
  })

  it('seeds new suggestions from the selected text or entire paragraph without modifying the source', () => {
    const selected = prepareParagraphAnnotation(data, emptyReviewState(), note, 'suggestion', () => 'selected')!
    const whole = prepareParagraphAnnotation(data, emptyReviewState(), paragraphAnchor, 'suggestion', () => 'whole')!
    expect(selected.annotation.replacement).toBe(note.quote)
    expect(whole.annotation.replacement).toBe(data.paragraphs[0].text)
    expect(data.paragraphs[0].text).toBe('开场😀很好。')
  })

  it('allows continuation at the annotation limit while preventing a new note on another paragraph', () => {
    const annotations = Array.from({ length: 500 }, (_, index) => ({ ...note, id: `note-${index}` }))
    const state = { ...emptyReviewState(), annotations }
    const reused = prepareParagraphAnnotation(data, state, paragraphAnchor, 'comment', () => 'unused')!
    expect(reused.created).toBe(false)
    expect(reused.state.annotations).toHaveLength(500)
    expect(prepareParagraphAnnotation(data, state, { ...paragraphAnchor, paragraph_id: 'ending' }, 'comment', () => 'overflow')).toBeNull()
    expect(validateReviewState(data, { ...state, verdict: 'changes_requested' })).toBeNull()
  })
})
