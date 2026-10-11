import { describe, expect, it } from 'vitest'
import { decodeRegisteredWorkbenchState } from './registry'
import { validateReviewState } from '../document-review/reviewModel'

describe('editable workbench draft decoding', () => {
  it('preserves incomplete review fields and extension data for editing, leaving submission validation separate', () => {
    const draft = {
      type: 'document_review', verdict: null, extension: { retained: true },
      annotations: [{
        id: 'note-1', body: '', kind: 'suggestion',
        start: -1, end: null, quote: null, replacement: null,
      }],
    }
    const decoded = decodeRegisteredWorkbenchState(draft)
    expect(decoded).toBe(draft)
    if (decoded?.type !== 'document_review') throw new Error('Expected an editable review draft')
    expect(validateReviewState({ title: 'Script', source_version: 'v1', markdown: 'Original' }, decoded))
      .toBe('Choose a review decision.')
  })

  it('preserves empty custom answers, optional answer metadata and legacy selections', () => {
    const questions = { type: 'questions', answers: [
      { id: 'audience', value: '', label: '', wasCustom: true, extension: 'retained' },
      { id: 'format', value: 'short', label: 'Short', wasCustom: false, index: 2 },
    ] }
    expect(decodeRegisteredWorkbenchState(questions)).toBe(questions)
    const legacy = { type: 'single_choice', selected_option_id: null }
    expect(decodeRegisteredWorkbenchState(legacy)).toBe(legacy)
  })

  it('keeps independent notes anchored to the same source range', () => {
    const note = {
      id: 'note-1', body: 'Keep this thought.', kind: 'comment',
      start: 0, end: 8, quote: 'Original', replacement: null, extension: { retained: true },
    }
    const draft = { type: 'document_review', verdict: 'changes_requested', annotations: [note, { ...note, id: 'note-2', body: 'A separate thought.' }] }
    const decoded = decodeRegisteredWorkbenchState(draft)
    expect(decoded).toBe(draft)
    if (decoded?.type !== 'document_review') throw new Error('Expected an editable review draft')
    expect(validateReviewState({ title: 'Script', source_version: 'v1', markdown: 'Original' }, decoded)).toBeNull()
    expect(decoded.annotations).toHaveLength(2)
  })

  it.each([
    { type: 'questions', answers: [{ id: 'audience', value: '', label: '', wasCustom: 'true' }] },
    { type: 'single_choice', selected_option_id: 42 },
    { type: 'document_review', verdict: 'later', annotations: [] },
    { type: 'document_review', verdict: null, annotations: [{ id: 'note-1' }] },
    { type: 'future_workbench' },
  ])('rejects a malformed or unsupported shape: $type', (value) => {
    expect(decodeRegisteredWorkbenchState(value)).toBeNull()
  })
})
