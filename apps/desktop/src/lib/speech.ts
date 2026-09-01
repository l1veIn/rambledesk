export type SpeechRecognitionStopReason = 'stopped' | 'cancelled' | 'unexpected'

export type SpeechRecognitionEvent =
  | {
      type: 'started'
      sessionId: string
      inputDevice: string
      provider: string
    }
  | {
      type: 'partial'
      sessionId: string
      text: string
    }
  | {
      type: 'level'
      sessionId: string
      rms: number
    }
  | {
      type: 'processing'
      sessionId: string
      segmentIndex: number
    }
  | {
      type: 'stable'
      sessionId: string
      segmentIndex: number
      text: string
    }
  | {
      type: 'warning'
      sessionId: string
      code: string
      message: string
    }
  | {
      type: 'stopped'
      sessionId: string
      reason: SpeechRecognitionStopReason
    }
  | {
      type: 'error'
      sessionId: string
      code: string
      message: string
    }

export type SpeechRecognitionListener = Readonly<{
  onEvent: (event: SpeechRecognitionEvent) => void
  onError: (cause: unknown) => void
}>

/**
 * A client-local recognition session. The Platform Plugin owns devices,
 * models, workers, and resource cleanup; callers only observe normalized
 * events and choose graceful stop or abortive cancellation.
 */
export interface SpeechRecognitionSession {
  readonly id: string
  readonly ready: Promise<void>
  stop(): Promise<void>
  cancel(): Promise<void>
}

export function eventBelongsToSpeechSession(
  event: SpeechRecognitionEvent,
  sessionId: string,
): boolean {
  return sessionId.length > 0 && event.sessionId === sessionId
}

export function voiceStartStillLive(
  phase: 'idle' | 'starting' | 'listening' | 'processing' | 'stopping' | 'error',
): boolean {
  return phase === 'starting' || phase === 'listening' || phase === 'processing'
}

export function stableTranscript(event: SpeechRecognitionEvent): string | null {
  if (event.type !== 'stable') return null
  const text = event.text.trim()
  return text.length > 0 ? text : null
}

export function stableSpeechSegmentId(
  event: Extract<SpeechRecognitionEvent, { type: 'stable' }>,
): string {
  return `asr-${event.sessionId}-${event.segmentIndex}`
}
