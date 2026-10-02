import { describe, expect, it } from 'vitest'
import type { DiffReviewAnchor, DiffReviewData } from '../../../generated/feedback'
import { emptyDiffReviewState, prepareDiffReviewComment } from '../../diff-review/reviewModel'
import { diffReviewDefinition } from './definition'
import { readDiffReviewState } from './state'

const spec = diffReviewDefinition.examples![0].spec
const data = spec.data as DiffReviewData
const anchor: DiffReviewAnchor = { file_id: 'errors', hunk_index: 0, side: 'new', start_line: 2, end_line: 3 }
const comment = { id: 'comment-1', anchor, body: 'Keep the actionable hint.' }
const state = { type: 'diff_review' as const, comments: [comment] }

describe('diff review definition and drafts', () => {
  it('accepts every development example and exposes an expanded immutable view', () => {
    for (const example of diffReviewDefinition.examples!) expect(diffReviewDefinition.accepts(example.spec.data)).toBe(true)
    expect(diffReviewDefinition.layout).toEqual({ padded: false, interactivePreview: false, expanded: true })
  })
  it('permits overall-notes-only submission, but blocks any empty comment draft', () => {
    expect(diffReviewDefinition.hasInput(spec, null)).toBe(false)
    expect(diffReviewDefinition.complete(spec, null)).toBe(true)
    expect(diffReviewDefinition.complete(spec, emptyDiffReviewState())).toBe(true)
    expect(diffReviewDefinition.hasInput(spec, { ...state, comments: [{ ...comment, body: '' }] })).toBe(true)
    expect(diffReviewDefinition.complete(spec, { ...state, comments: [{ ...comment, body: ' \n' }] })).toBe(false)
    expect(diffReviewDefinition.complete(spec, state)).toBe(true)
  })
  it.each([['\u0085', false], ['\uFEFF', true]])('matches Rust Unicode whitespace for comments and source labels %#', (value, visible) => {
    expect(readDiffReviewState({ ...state, comments: [{ ...comment, body: value }] })).not.toBeNull()
    expect(diffReviewDefinition.complete(spec, { ...state, comments: [{ ...comment, body: value }] })).toBe(visible)
    expect(diffReviewDefinition.accepts({ ...data, title: value })).toBe(visible)
    expect(diffReviewDefinition.accepts({ ...data, source_version: value })).toBe(visible)
    expect(diffReviewDefinition.accepts({ ...data, files: [{ ...data.files[0], old_path: value, new_path: value }] })).toBe(visible)
  })
  it('restores blank and Unicode drafts while rejecting unknown fields and unsafe identities', () => {
    const draft = { ...state, comments: [{ ...comment, body: '😀'.repeat(4000) }] }
    expect(readDiffReviewState(draft)).toEqual(draft)
    expect(readDiffReviewState({ ...state, comments: [{ ...comment, body: '' }] })).not.toBeNull()
    for (const invalid of [
      { ...state, verdict: 'approve' }, { ...state, comments: [comment, comment] },
      { ...state, comments: [{ ...comment, id: 'bad.id' }] }, { ...state, comments: [{ ...comment, body: '\0' }] },
      { ...state, comments: [{ ...comment, body: '\uD800' }] }, { ...draft, comments: [{ ...draft.comments[0], body: '😀'.repeat(4001) }] },
      { ...state, comments: [{ ...comment, anchor: { ...anchor, start_line: null } }] },
      { ...state, comments: [{ ...comment, anchor: { ...anchor, start_line: 0 } }] },
      { ...state, comments: [{ ...comment, anchor: { ...anchor, end_line: 1 } }] },
    ]) expect(readDiffReviewState(invalid)).toBeNull()
  })
  it('rejects missing files, cross-hunk ranges and changed immutable source during publishing', () => {
    for (const next of [{ ...anchor, file_id: 'missing' }, { ...anchor, end_line: 5 }, { ...anchor, hunk_index: 1 }]) {
      expect(diffReviewDefinition.complete(spec, { ...state, comments: [{ ...comment, anchor: next }] })).toBe(false)
    }
    const changed = { ...spec, data: { ...data, files: [{ ...data.files[0], diff: '@@ -1 +1 @@\n-old\n+new' }] } }
    expect(diffReviewDefinition.complete(changed, state)).toBe(false)
    expect(diffReviewDefinition.complete(spec, { type: 'questions', answers: [] })).toBe(false)
  })
  it('enforces input budgets, unique file IDs, scalar lengths and strict single-file diffs', () => {
    expect(diffReviewDefinition.accepts({ ...data, title: '😀'.repeat(200) })).toBe(true)
    const large = '@@ -0,0 +1 @@\n+' + 'x'.repeat(100000)
    for (const invalid of [
      { ...data, files: [] }, { ...data, files: [data.files[0], data.files[0]] }, { ...data, title: '😀'.repeat(201) },
      { ...data, source_version: ' ' }, { ...data, verdict: 'approve' },
      { ...data, files: [{ ...data.files[0], diff: 'Binary files differ' }] },
      { ...data, files: [{ ...data.files[0], old_path: '' }] },
      { ...data, files: Array.from({ length: 5 }, (_, index) => ({ ...data.files[0], id: `file-${index}`, diff: large })) },
    ]) expect(diffReviewDefinition.accepts(invalid)).toBe(false)
  })
  it('creates reversible empty drafts, reopens identical anchors and preserves old/new identity', () => {
    const source = JSON.stringify(data)
    const prepared = prepareDiffReviewComment(data, emptyDiffReviewState(), anchor, () => 'draft-1')!
    expect(prepared.comment).toEqual({ id: 'draft-1', anchor, body: '' })
    expect(prepared.comment.anchor).not.toBe(anchor)
    expect(prepareDiffReviewComment(data, prepared.state, anchor, () => 'unused')?.state).toBe(prepared.state)
    const old = prepareDiffReviewComment(data, prepared.state, { ...anchor, side: 'old', end_line: 2 }, () => 'draft-2')!
    expect(old.state.comments).toHaveLength(2)
    expect(prepareDiffReviewComment(data, old.state, { ...anchor, end_line: 4 }, () => 'bad.id')).toBeNull()
    expect(prepareDiffReviewComment(data, old.state, { ...anchor, end_line: 4 }, () => 'draft-1')).toBeNull()
    expect(JSON.stringify(data)).toBe(source)
  })
})
