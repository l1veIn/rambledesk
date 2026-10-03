import { assert, describe, expect, it } from 'vitest'
import type { MediaReviewData, WorkbenchSpec } from '../../../generated/feedback'
import { mediaReviewDefinition as definition } from './definition'
import { readMediaReviewState, type MediaReviewState } from './state'
import { mediaDurationMismatch, prepareMediaComment, sortedMediaComments } from '../../media-review/mediaModel'

const data: MediaReviewData = { title: 'Review', source_version: 'source-v1', media_kind: 'video', media_file_name: 'clip.webm', duration_ms: 8000 }
const spec: WorkbenchSpec = { type: 'media_review', version: 1, data }
const comment = { id: 'one', start_ms: 1000, end_ms: null, body: 'Make the sound quieter.' }
const draft: MediaReviewState = { type: 'media_review', comments: [comment] }

describe('media review contract', () => {
  it('allows overall notes alone and blank saved comments, but requires every submitted comment to be visible', () => {
    expect(definition.complete(spec, null)).toBe(true)
    expect(definition.complete(spec, { type: 'media_review', comments: [] })).toBe(true)
    const blank = { ...draft, comments: [{ ...comment, body: '' }] }
    expect(readMediaReviewState(blank)).toEqual(blank)
    expect(definition.hasInput(spec, blank)).toBe(true)
    expect(definition.complete(spec, blank)).toBe(false)
    expect(definition.complete(spec, draft)).toBe(true)
    expect(definition.supportsApproval).not.toBe(true)
    expect(definition.complete(spec, { ...draft, comments: [{ ...comment, body: '\u0085' }] })).toBe(false)
    expect(definition.complete(spec, { ...draft, comments: [{ ...comment, body: '\ufeff' }] })).toBe(true)
  })
  it.each([' clip.webm ', '\u0085clip.webm', '../clip.webm', 'clip/part.webm', '.', '..', '\ud800', '音'.repeat(86)])('rejects invalid or changed attachment identity %j', (name) => {
    expect(definition.accepts({ ...data, media_file_name: name })).toBe(false)
  })
  it('accepts exact Unicode filenames and rejects unknown fields, URLs and invalid timebases', () => {
    expect(definition.accepts({ ...data, media_file_name: '演示 视频.webm' })).toBe(true)
    for (const invalid of [{ ...data, url: 'https://example.test/a' }, { ...data, duration_ms: 8.5 }, { ...data, duration_ms: 0 }, { ...data, duration_ms: 86400001 }]) {
      expect(definition.accepts(invalid)).toBe(false)
    }
  })
  it('validates immutable millisecond points and ranges, including an endpoint point', () => {
    for (const anchor of [{ start_ms: 8000, end_ms: null }, { start_ms: 0, end_ms: 8000 }]) {
      expect(definition.complete(spec, { ...draft, comments: [{ ...comment, ...anchor }] })).toBe(true)
    }
    for (const anchor of [{ start_ms: 8001, end_ms: null }, { start_ms: 0, end_ms: 8001 }, { start_ms: 2000, end_ms: 2000 }, { start_ms: -1, end_ms: null }, { start_ms: 0.5, end_ms: null }]) {
      expect(definition.complete(spec, { ...draft, comments: [{ ...comment, ...anchor }] })).toBe(false)
    }
  })
  it('rejects duplicate IDs, unknown state fields and malformed or oversized text without truncation', () => {
    for (const invalid of [{ ...draft, comments: [comment, comment] }, { ...draft, approval: 'yes' },
      { ...draft, comments: [{ ...comment, body: '\ud800' }] }, { ...draft, comments: [{ ...comment, body: '🙂'.repeat(4001) }] },
      { ...draft, comments: Array.from({ length: 501 }, (_, i) => ({ ...comment, id: `comment-${i}` })) }]) {
      expect(readMediaReviewState(invalid)).toBeNull()
    }
    expect(readMediaReviewState({ ...draft, comments: [{ ...comment, body: '🙂'.repeat(4000) }] })).not.toBeNull()
  })
  it('reopens matching anchors and sorts a copy without rewriting the saved creation order', () => {
    expect(prepareMediaComment(data, draft, comment, () => 'two')?.state).toBe(draft)
    const second = { ...comment, id: 'two', start_ms: 0, end_ms: 500 }
    const original = [comment, second]
    expect(sortedMediaComments(original, 'time')).toEqual([second, comment])
    expect(sortedMediaComments(original, 'newest')).toEqual([second, comment])
    expect(sortedMediaComments(original, 'oldest')).toEqual(original)
    expect(original).toEqual([comment, second])
    expect(prepareMediaComment(data, draft, { start_ms: 9000, end_ms: null }, () => 'two')).toBeNull()
  })
  it('keeps duration diagnostics separate from immutable anchors and supplies real previews with matching identities', () => {
    expect(mediaDurationMismatch(8.008, 8000)).toBe(false)
    expect(mediaDurationMismatch(12, 8000)).toBe(true)
    expect(mediaDurationMismatch(Infinity, 8000)).toBe(true)
    expect(definition.examples!.map((example) => example.key)).toEqual(['media_review-audio', 'media_review-video'])
    for (const example of definition.examples!) {
      const exampleData = example.spec.data
      expect(definition.accepts(exampleData)).toBe(true)
      assert('media_file_name' in exampleData && 'media_kind' in exampleData, 'Expected media review example data')
      expect(example.attachments!.filter((attachment) => attachment.name === exampleData.media_file_name)).toHaveLength(1)
      const decoded = atob(example.attachments![0].contentsBase64!)
      expect(decoded.length).toBeGreaterThan(44)
      expect(decoded.slice(0, 4)).toBe(exampleData.media_kind === 'audio' ? 'RIFF' : '\x1aE\xdf\xa3')
    }
  })
})
