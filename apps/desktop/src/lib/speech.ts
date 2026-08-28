export type SpeechEvent =
  | {
      type: 'started'
      request_id: string
      voice_session_id: string
      input_device: string
      provider: string
    }
  | {
      type: 'partial'
      request_id: string
      voice_session_id: string
      text: string
    }
  | {
      type: 'level'
      request_id: string
      voice_session_id: string
      rms: number
    }
  | {
      type: 'processing'
      request_id: string
      voice_session_id: string
      chunk_index: number
    }
  | {
      type: 'stable'
      request_id: string
      voice_session_id: string
      chunk_index: number
      text: string
    }
  | {
      type: 'warning'
      request_id: string
      voice_session_id: string
      code: string
      message: string
    }
  | {
      type: 'stopped'
      request_id: string
      voice_session_id: string
    }
  | {
      type: 'error'
      request_id: string
      voice_session_id: string
      code: string
      message: string
    }

export type VoiceRambleSessionView = {
  voice_session_id: string
  provider: string
  model_path: string
}

export function eventBelongsToVoiceSession(
  event: SpeechEvent,
  requestId: string,
  voiceSessionId: string,
): boolean {
  return (
    event.request_id === requestId &&
    (voiceSessionId.length === 0 || event.voice_session_id === voiceSessionId)
  )
}

export function voiceStartStillLive(
  phase: 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error',
): boolean {
  return phase === 'starting' || phase === 'listening' || phase === 'processing'
}

export function stableTranscript(event: SpeechEvent): string | null {
  if (event.type !== 'stable') return null
  const text = event.text.trim()
  return text.length > 0 ? text : null
}
