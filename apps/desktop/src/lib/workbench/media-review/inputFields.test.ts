import { describe, expect, it } from 'vitest'
import type { InputTarget } from '../../domain/inputTarget'
import type { MediaReviewData, WorkbenchSpec } from '../../generated/feedback'
import type { MediaReviewState } from '../definitions/media_review/state'
import { mediaReviewInputFields } from './inputFields'

const data: MediaReviewData = { title: 'Clip', source_version: 'clip-v1', media_kind: 'video', media_file_name: 'clip.webm', duration_ms: 8000 }
const spec: WorkbenchSpec = { type: 'media_review', version: 1, data }
const adapter = mediaReviewInputFields[0]
const target: InputTarget = { requestId: 'r', requestTitle: 'Review', destination: { kind: 'workbench_field', workbenchType: 'media_review',
  version: 1, field: 'comment_body', entityId: 'first', sourceVersion: data.source_version, label: 'clip.webm · 00:01.000' } }
const state: MediaReviewState = { type: 'media_review', comments: [
  { id: 'first', start_ms: 1000, end_ms: 2500, body: 'Typed\n\n[notes](attachment://a)' },
  { id: 'second', start_ms: 4000, end_ms: null, body: 'Other comment' },
] }
describe('generic media comment fields', () => {
  it('writes only the captured comment body and preserves timing, source and unrelated comments', () => {
    const captured = adapter.read({ spec, state, target }), next = captured.replace('Typed\nVoice') as MediaReviewState
    expect(captured.limit).toBe(4000); expect(adapter.accepts(target)).toBe(true)
    expect(adapter.text(state, target)).toBe(state.comments[0].body)
    expect(next.comments[0]).toEqual({ ...state.comments[0], body: 'Typed\nVoice' })
    expect(next.comments[1]).toBe(state.comments[1])
    expect(state.comments[0].body).toContain('attachment://a')
  })
  it('fingerprints the immutable source and point/range identity for late voice or tidy writes', () => {
    const captured = adapter.read({ spec, state, target })
    expect(adapter.read({ spec: { ...spec, data: { ...data, duration_ms: 9000 } }, state, target }).contract).not.toBe(captured.contract)
    expect(adapter.read({ spec, state: { ...state, comments: [{ ...state.comments[0], end_ms: 3000 }] }, target }).identity).not.toBe(captured.identity)
    const movedTarget = { ...target, destination: { ...target.destination, sourceVersion: 'another' } } as InputTarget
    expect(() => adapter.read({ spec, state, target: movedTarget })).toThrow('original media has changed')
  })
  it('rejects deleted, ambiguous and out-of-range anchors and unlinks attachments without moving comments', () => {
    for (const comments of [[], [state.comments[0], state.comments[0]], [{ ...state.comments[0], end_ms: 8001 }]]) {
      expect(() => adapter.read({ spec, state: { ...state, comments }, target })).toThrow('unavailable')
    }
    const next = adapter.removeAttachment(state, 'a', (body) => body.replace('\n\n[notes](attachment://a)', '')) as MediaReviewState
    expect(next.comments[0]).toEqual({ ...state.comments[0], body: 'Typed' })
    expect(next.comments[1]).toBe(state.comments[1])
    expect(adapter.removeAttachment(next, 'missing', (body) => body)).toBe(next)
  })
})
