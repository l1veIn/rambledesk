import { describe, expect, it } from 'vitest'
import { createSpeechTargetTracker } from './speechTargetTracker'
import type { SpeechTarget } from './speechDraftQueue'

describe('speech destination tracking', () => {
  const a: SpeechTarget = { requestId: 'a', requestTitle: 'A', action: { actionId: 'first', actionIndex: 0, title: 'First action' } }
  const b: SpeechTarget = { ...a, action: { actionId: 'second', actionIndex: 1, title: 'Second action' } }
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
