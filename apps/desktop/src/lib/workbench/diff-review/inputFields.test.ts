import { describe, expect, it } from 'vitest'
import type { InputTarget } from '../../domain/inputTarget'
import type { DiffReviewData } from '../../generated/feedback'
import { diffReviewDefinition } from '../definitions/diff_review/definition'
import type { DiffReviewState } from '../definitions/diff_review/state'
import { diffReviewInputFields } from './inputFields'

const spec = diffReviewDefinition.examples![0].spec, data = spec.data as DiffReviewData
const adapter = diffReviewInputFields[0]
const target: InputTarget = { requestId: 'r', requestTitle: 'Review', destination: { kind: 'workbench_field',
  workbenchType: 'diff_review', version: 1, field: 'comment_body', entityId: 'comment-1', sourceVersion: data.source_version, label: 'src/cli/errors.ts · +2' } }
const state: DiffReviewState = { type: 'diff_review', comments: [
  { id: 'comment-1', anchor: { file_id: 'errors', hunk_index: 0, side: 'new', start_line: 2, end_line: 3 }, body: 'Typed\n\n[notes](attachment://a)' },
  { id: 'comment-2', anchor: { file_id: 'quickstart', hunk_index: 0, side: 'new', start_line: null, end_line: null }, body: 'Other comment' },
] }
const fieldTarget = (changes: Record<string, unknown>): InputTarget => ({ ...target, destination: { ...target.destination, ...changes } } as InputTarget)

describe('generic diff comment fields', () => {
  it('routes speech/tidy writes to only the typed comment body and preserves all immutable anchors', () => {
    const value = adapter.read({ spec, state, target })
    const next = value.replace('Typed\nVoice') as DiffReviewState
    expect(value.limit).toBe(4000)
    expect(adapter.accepts(target)).toBe(true)
    expect(adapter.text(state, target)).toBe(state.comments[0].body)
    expect(next.comments[0]).toEqual({ ...state.comments[0], body: 'Typed\nVoice' })
    expect(next.comments[0].anchor).toBe(state.comments[0].anchor)
    expect(next.comments[1]).toBe(state.comments[1])
    expect(state.comments[0].body).toContain('attachment://a')
    expect(adapter.accepts(fieldTarget({ field: 'files[0].diff' }))).toBe(false)
  })
  it('captures the full source contract and comment anchor identity for stale-write protection', () => {
    const captured = adapter.read({ spec, state, target })
    const changed = { ...spec, data: { ...data, title: 'Changed title' } }
    expect(adapter.read({ spec: changed, state, target }).contract).not.toBe(captured.contract)
    const moved = { ...state, comments: [{ ...state.comments[0], anchor: { ...state.comments[0].anchor, end_line: 4 } }] }
    expect(adapter.read({ spec, state: moved, target }).identity).not.toBe(captured.identity)
    expect(() => adapter.read({ spec, state, target: fieldTarget({ sourceVersion: 'another-version' }) })).toThrow('original diff has changed')
  })
  it('refuses deleted, duplicated or invalid anchors instead of writing into another field', () => {
    for (const broken of [
      { ...state, comments: [] }, { ...state, comments: [state.comments[0], state.comments[0]] },
      { ...state, comments: [{ ...state.comments[0], anchor: { ...state.comments[0].anchor, file_id: 'missing' } }] },
    ]) expect(() => adapter.read({ spec, state: broken, target })).toThrow('unavailable')
    expect(adapter.text({ ...state, comments: [] }, target)).toBeNull()
    expect(adapter.text(state, fieldTarget({ workbenchType: 'form' }))).toBeNull()
  })
  it('unlinks attachments from owned bodies while retaining coordinates and unrelated comments', () => {
    const next = adapter.removeAttachment(state, 'a', (value) => value.replace('\n\n[notes](attachment://a)', '')) as DiffReviewState
    expect(next.comments[0]).toEqual({ ...state.comments[0], body: 'Typed' })
    expect(next.comments[1]).toBe(state.comments[1])
    expect(adapter.removeAttachment(next, 'missing', (value) => value)).toBe(next)
  })
})
