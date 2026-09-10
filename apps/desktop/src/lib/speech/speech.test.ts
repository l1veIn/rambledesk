import { describe, expect, it } from 'vitest'

import {
  eventBelongsToSpeechSession,
  stableSpeechSegmentId,
  stableTranscript,
  voiceStartStillLive,
  type SpeechRecognitionEvent,
} from './speech'

describe('voice ramble events', () => {
  const stable: SpeechRecognitionEvent = {
    type: 'stable',
    sessionId: 'session-a',
    segmentIndex: 0,
    text: '  中文片段  ',
  }

  it('accepts events only for the active client-local recognition session', () => {
    expect(eventBelongsToSpeechSession(stable, 'session-a')).toBe(true)
  })

  it('rejects events from an old or not-yet-bound session', () => {
    expect(eventBelongsToSpeechSession(stable, 'session-b')).toBe(false)
    expect(eventBelongsToSpeechSession(stable, '')).toBe(false)
  })

  it('treats a start as failed if error or stop arrived before the command returned', () => {
    expect(voiceStartStillLive('starting')).toBe(true)
    expect(voiceStartStillLive('listening')).toBe(true)
    expect(voiceStartStillLive('idle')).toBe(false)
    expect(voiceStartStillLive('error')).toBe(false)
    expect(voiceStartStillLive('stopping')).toBe(false)
  })

  it('only exposes non-empty stable transcript text', () => {
    expect(stableTranscript(stable)).toBe('中文片段')
    expect(stableTranscript({ ...stable, text: '  ' })).toBeNull()
    expect(
      stableTranscript({
        type: 'processing',
        sessionId: 'session-a',
        segmentIndex: 0,
      }),
    ).toBeNull()
  })

  it('derives a stable segment id so duplicate native delivery is idempotent', () => {
    const event = stable as Extract<SpeechRecognitionEvent, { type: 'stable' }>
    expect(stableSpeechSegmentId(event)).toBe('asr-session-a-0')
    expect(stableSpeechSegmentId(event)).toBe('asr-session-a-0')
  })
})
