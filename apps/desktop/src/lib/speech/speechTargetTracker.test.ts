import { describe, expect, it } from 'vitest'
import { createSpeechTargetTracker } from './speechTargetTracker'
import type { SpeechTarget } from './speechDraftQueue'

describe('speech destination tracking', () => {
  it('keeps streaming speech pinned to the original question after navigation to the next question', () => {
    let selected: SpeechTarget = { requestId: 'r', requestTitle: 'Questions', destination: { kind: 'question_answer', questionId: 'q1', questionLabel: 'First' } }
    const first = structuredClone(selected)
    const tracker = createSpeechTargetTracker(() => selected)
    tracker.observe({ type: 'partial', sessionId: 's', text: 'First answer' })
    selected = { ...selected, destination: { kind: 'question_answer', questionId: 'q2', questionLabel: 'Second' } }
    expect(tracker.activeTarget()).toEqual(first)
    tracker.observe({ type: 'processing', sessionId: 's', segmentIndex: 0 })
    expect(tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 0, text: 'First complete' })).toEqual(first)
    tracker.observe({ type: 'partial', sessionId: 's', text: 'Second answer' })
    expect(tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 1, text: 'Second complete' })).toEqual(selected)
  })
  const a: SpeechTarget = { requestId: 'a', requestTitle: 'A', destination: { kind: 'document', action: { actionId: 'first', actionIndex: 0, title: 'First action' } } }
  const b: SpeechTarget = { ...a, destination: { kind: 'document', action: { actionId: 'second', actionIndex: 1, title: 'Second action' } } }
  it('shows the pinned annotation until processing, independently of the next chosen field', () => {
    let target: SpeechTarget = { ...a, destination: { kind: 'review_annotation', annotationId: 'comment', field: 'body', sourceVersion: 'v1', paragraphLabel: 'Opening' } }
    const pinned = structuredClone(target)
    const tracker = createSpeechTargetTracker(() => target)
    tracker.observe({ type: 'speech-started', sessionId: 's', segmentIndex: 0 })
    if (target.destination.kind !== 'review_annotation') throw new Error('fixture')
    target.destination.field = 'replacement'
    expect(tracker.activeTarget()).toEqual(pinned)
    const displayed = tracker.activeTarget()!
    displayed.requestId = 'mutated-display'
    expect(tracker.activeTarget()).toEqual(pinned)
    tracker.observe({ type: 'processing', sessionId: 's', segmentIndex: 0 })
    expect(tracker.activeTarget()).toBeNull()
    target = b
    tracker.observe({ type: 'partial', sessionId: 's', text: 'Next words' })
    expect(tracker.activeTarget()).toEqual(b)
    expect(tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 0, text: 'Earlier annotation words' })).toEqual(pinned)
    expect(tracker.activeTarget()).toEqual(b)
    tracker.reset()
    expect(tracker.activeTarget()).toBeNull()
  })
  it('pins non-streaming speech at VAD onset across action and tab changes', () => {
    let target = a
    const tracker = createSpeechTargetTracker(() => target)
    tracker.observe({ type: 'level', sessionId: 's', rms: .8 })
    target = b
    tracker.observe({ type: 'speech-started', sessionId: 's', segmentIndex: 0 })
    target = a
    tracker.observe({ type: 'processing', sessionId: 's', segmentIndex: 0 })
    expect(tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 0, text: 'Hello' })).toEqual(b)
  })
  it('pins streaming partials and keeps the next segment separate while an earlier one completes', () => {
    let target = a
    const tracker = createSpeechTargetTracker(() => target)
    tracker.observe({ type: 'partial', sessionId: 's', text: 'First' })
    tracker.observe({ type: 'processing', sessionId: 's', segmentIndex: 0 })
    target = b
    tracker.observe({ type: 'partial', sessionId: 's', text: 'Second' })
    expect(tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 0, text: 'First' })).toEqual(a)
    expect(tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 1, text: 'Second' })).toEqual(b)
  })
  it('releases empty results and resets at session boundaries', () => {
    let target = a
    const tracker = createSpeechTargetTracker(() => target)
    tracker.observe({ type: 'speech-started', sessionId: 's', segmentIndex: 0 })
    tracker.observe({ type: 'stable', sessionId: 's', segmentIndex: 0, text: '' })
    tracker.reset()
    target = b
    expect(tracker.observe({ type: 'stable', sessionId: 'next', segmentIndex: 0, text: 'Next session' })).toEqual(b)
  })
})
