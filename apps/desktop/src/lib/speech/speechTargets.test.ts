import { describe, expect, it } from 'vitest'
import { normalizeSpeechTarget, sameSpeechTarget, snapshotSpeechTarget, type SpeechTarget } from './speechTargets'

const review: SpeechTarget = { requestId: 'r', requestTitle: 'Review', destination: {
  kind: 'review_annotation', annotationId: 'a', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening',
} }

describe('speech targets', () => {
  it('restores question targets and compares question identity rather than its display label', () => {
    const target: SpeechTarget = { requestId: 'r', requestTitle: 'Questions', destination: { kind: 'question_answer', questionId: 'q1', questionLabel: 'First question' } }
    expect(normalizeSpeechTarget(JSON.parse(JSON.stringify(target)))).toEqual(target)
    expect(sameSpeechTarget(target, { ...target, destination: { kind: 'question_answer', questionId: 'q1', questionLabel: 'Renamed' } })).toBe(true)
    expect(sameSpeechTarget(target, { ...target, destination: { kind: 'question_answer', questionId: 'q2', questionLabel: 'First question' } })).toBe(false)
    expect(sameSpeechTarget(target, { ...target, requestId: 'another-request' })).toBe(false)
    expect(sameSpeechTarget(target, review)).toBe(false)
    expect(normalizeSpeechTarget({ ...target, destination: { kind: 'question_answer', questionId: 'q1' } })?.destination.kind).toBe('unknown')
  })
  it('migrates legacy action targets and preserves future destination payloads across reloads', () => {
    const legacy = { requestId: 'r', requestTitle: 'R', action: { actionId: 'a', actionIndex: 0, title: 'Action' } }
    expect(normalizeSpeechTarget(legacy)).toEqual({ requestId: 'r', requestTitle: 'R', destination: { kind: 'document', action: legacy.action } })
    const future = { ...legacy, destination: { kind: 'future', version: 8, nested: { destinations: ['a', null] } } }
    const restored = normalizeSpeechTarget(future)!
    expect(restored.destination).toEqual({ kind: 'unknown', raw: future.destination })
    expect(normalizeSpeechTarget(JSON.parse(JSON.stringify(restored)))).toEqual(restored)
    expect(normalizeSpeechTarget({ ...legacy, destination: { kind: 'review_annotation', annotationId: 'missing-fields' } })?.destination.kind).toBe('unknown')
  })

  it('snapshots all destination fields and compares the stable write identity', () => {
    const snapshot = snapshotSpeechTarget(review)
    if (snapshot.destination.kind !== 'review_annotation') throw new Error('fixture')
    snapshot.destination.field = 'replacement'
    expect(sameSpeechTarget(review, snapshot)).toBe(false)
    const renamed = structuredClone(review)
    if (renamed.destination.kind !== 'review_annotation') throw new Error('fixture')
    renamed.destination.paragraphLabel = 'New display name'
    expect(sameSpeechTarget(review, renamed)).toBe(true)
    renamed.destination.sourceVersion = 'v2'
    expect(sameSpeechTarget(review, renamed)).toBe(false)
    expect(sameSpeechTarget(review, { ...review, requestId: 'another' })).toBe(false)
    const unknown = { ...review, destination: { kind: 'unknown' as const, raw: {} } }
    expect(sameSpeechTarget(unknown, unknown)).toBe(false)
  })
})
