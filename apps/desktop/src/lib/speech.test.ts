import { describe, expect, it } from 'vitest'

import {
  eventBelongsToVoiceSession,
  stableTranscript,
  type SpeechEvent,
} from './speech'

describe('voice ramble events', () => {
  const stable: SpeechEvent = {
    type: 'stable',
    request_id: 'request-a',
    session_id: 'session-a',
    chunk_index: 0,
    text: '  中文片段  ',
  }

  it('accepts the first event before the start command has returned its session id', () => {
    expect(eventBelongsToVoiceSession(stable, 'request-a', '')).toBe(true)
  })

  it('rejects events from another request or an old session', () => {
    expect(eventBelongsToVoiceSession(stable, 'request-b', '')).toBe(false)
    expect(eventBelongsToVoiceSession(stable, 'request-a', 'session-b')).toBe(false)
  })

  it('only exposes non-empty stable transcript text', () => {
    expect(stableTranscript(stable)).toBe('中文片段')
    expect(stableTranscript({ ...stable, text: '  ' })).toBeNull()
    expect(
      stableTranscript({
        type: 'processing',
        request_id: 'request-a',
        session_id: 'session-a',
        chunk_index: 0,
      }),
    ).toBeNull()
  })
})
