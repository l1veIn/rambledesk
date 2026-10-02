import { describe, expect, it } from 'vitest'
import { decodeRegisteredWorkbenchState } from './registry'
import { validateReviewState } from '../document-review/reviewModel'

describe('editable workbench draft decoding', () => {
  it('preserves incomplete review fields and extension data for editing, leaving submission validation separate', () => {
    const draft = {
      type: 'document_review', verdict: null, paragraph_marks: [], extension: { retained: true },
      annotations: [{
        id: 'note-1', paragraph_id: 'opening', body: '', kind: 'suggestion',
        start: -1, end: null, quote: null, replacement: null,
      }],
    }
    const decoded = decodeRegisteredWorkbenchState(draft)
    expect(decoded).toBe(draft)
    if (decoded?.type !== 'document_review') throw new Error('Expected an editable review draft')
    expect(validateReviewState({ title: 'Script', source_version: 'v1', paragraphs: [{ id: 'opening', text: 'Original' }] }, decoded))
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

  it.each(['open', 'resolved'])('keeps a legacy %s note while retiring its status', (status) => {
    const note = {
      id: 'note-1', paragraph_id: 'opening', body: 'Keep this thought.', kind: 'comment',
      start: null, end: null, quote: null, replacement: null, extension: { retained: true },
    }
    const draft = { type: 'document_review', verdict: 'changes_requested', paragraph_marks: [], annotations: [{ ...note, status }] }
    const decoded = decodeRegisteredWorkbenchState(draft)
    expect(decoded).toEqual({ ...draft, annotations: [note] })
    if (decoded?.type !== 'document_review') throw new Error('Expected an editable review draft')
    expect(validateReviewState({ title: 'Script', source_version: 'v1', paragraphs: [{ id: 'opening', text: 'Original' }] }, decoded)).toBeNull()
    expect(decoded.annotations[0]).not.toHaveProperty('status')
    expect(draft.annotations[0].status).toBe(status)
  })

  it.each([
    { type: 'questions', answers: [{ id: 'audience', value: '', label: '', wasCustom: 'true' }] },
    { type: 'single_choice', selected_option_id: 42 },
    { type: 'document_review', verdict: 'later', annotations: [], paragraph_marks: [] },
    { type: 'document_review', verdict: null, annotations: [], paragraph_marks: [{ paragraph_id: 'opening', decision: 'later' }] },
    { type: 'document_review', verdict: null, annotations: [{ id: 'note-1' }], paragraph_marks: [] },
    { type: 'future_workbench' },
  ])('rejects a malformed or unsupported shape: $type', (value) => {
    expect(decodeRegisteredWorkbenchState(value)).toBeNull()
  })
})
