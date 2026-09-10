import { derived, get, writable } from 'svelte/store'

import type { FeedbackRequestSummary } from '../feedback'
import type { RamblePhase } from '../domain/sessionPhases'
import { resolvedRamblePhase } from './rambleSessionState'
import { createVoiceRambleSession, createVoiceRambleState, type VoiceRambleContext } from './voiceRambleSession'

const initial = {
  phase: 'idle' as RamblePhase,
  startedOnce: false,
  requestId: '',
  requestTitle: '',
  message: '',
}

/** Ramble owns request identity; the microphone writes its own facts directly. */
export function createRambleSession() {
  const facts = writable(initial)
  const voiceFacts = createVoiceRambleState()
  let voiceConnected = false
  const state = derived([facts, voiceFacts], ([ramble, voice]) => {
    const visiblePhase = resolvedRamblePhase(ramble.phase, voice.phase)
    const voiceActive = ['starting', 'listening', 'processing', 'stopping'].includes(voice.phase)
    return {
      ...ramble,
      voicePhase: voice.phase,
      voiceDevice: voice.device,
      voicePartial: voice.partial,
      voiceLevel: voice.level,
      voiceChunkIndex: voice.chunkIndex,
      voiceModelMissing: voice.modelMissing,
      visiblePhase,
      voiceActive,
      voiceCanStop: voiceActive || (voice.phase === 'error' && !!voice.sessionId),
      engaged: visiblePhase !== 'idle',
      active: visiblePhase === 'active',
      speechStopError: voice.phase === 'error' ? voice.message || ramble.message : '',
    }
  })

  function begin(request: Pick<FeedbackRequestSummary, 'request_id' | 'title'>) {
    facts.set({ ...initial, startedOnce: true, requestId: request.request_id, requestTitle: request.title })
  }

  function transition(phase: RamblePhase, message: string) {
    facts.update((current) => ({ ...current, phase, message }))
  }

  return {
    subscribe: state.subscribe,
    // Share the original voice store, never copy its six fields through UI bindings.
    connectVoice: (context: VoiceRambleContext) => {
      if (voiceConnected) throw new Error('A Ramble session has one microphone controller for its lifetime.')
      voiceConnected = true
      return createVoiceRambleSession(context, voiceFacts)
    },
    begin,
    transition,
    setMessage: (message: string) => facts.update((current) => ({ ...current, message })),
    reset: () => facts.set(initial),
    belongsToRequest: (requestId: string | null | undefined) => !get(state).engaged || requestId === get(state).requestId,
    speechStopError: () => get(state).speechStopError,
  }
}

export type RambleSession = ReturnType<typeof createRambleSession>
export type RambleSessionState = Parameters<Parameters<RambleSession['subscribe']>[0]>[0]
