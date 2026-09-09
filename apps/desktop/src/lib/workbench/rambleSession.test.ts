import { get } from 'svelte/store'
import { describe, expect, it } from 'vitest'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import type { SpeechRecognitionListener } from '../speech/speech'
import { createRambleSession } from './rambleSession'

function harness() {
  const session = createRambleSession()
  let listener!: SpeechRecognitionListener
  const voice = session.connectVoice({
    speech: {
      ...createUnavailableWorkbenchCapabilities().speech,
      implementation: { ...createUnavailableWorkbenchCapabilities().speech.implementation, start: (_options, next) => {
        listener = next
        return { id: 'voice-1', ready: Promise.resolve(), stop: async () => {
          listener.onEvent({ type: 'stopped', sessionId: 'voice-1', reason: 'stopped' })
        }, cancel: async () => {} }
      } },
    },
    tr: (source) => source, messageFrom: String,
    getInputDevice: () => '', getModelId: () => '', getHotwords: () => [],
    getVadThreshold: () => 0.5, getVadSilenceMs: () => 500, getNotificationVolume: () => 0,
    resolveTarget: () => ({ requestId: get(session).requestId, requestTitle: get(session).requestTitle, action: null }),
    resetTargets: () => {}, onStable: () => {}, onRecording: () => {},
    onMicrophoneStopped: () => {}, onMicrophoneError: () => {}, waitForDrafts: async () => {},
  })
  return { session, voice, emit: (event: Parameters<SpeechRecognitionListener['onEvent']>[0]) => listener.onEvent(event) }
}

describe('ramble session observable contract', () => {
  it('projects live microphone facts immediately to all readers without UI bindings', async () => {
    const { session, voice, emit } = harness()
    session.begin({ request_id: 'request-1', title: 'Review' })
    session.transition('starting', 'Connecting')
    const observed: string[] = []
    const unsubscribe = session.subscribe((state) => observed.push(state.visiblePhase))
    await voice.start('request-1')
    emit({ type: 'level', sessionId: 'voice-1', rms: 0.1 })
    emit({ type: 'partial', sessionId: 'voice-1', text: 'A live observation' })
    expect(get(session)).toMatchObject({ active: true, voiceActive: true, voiceLevel: 0.8, voicePartial: 'A live observation' })
    expect(observed.at(-1)).toBe('active')
    expect(session.belongsToRequest('request-1')).toBe(true)
    expect(session.belongsToRequest('request-2')).toBe(false)
    await voice.stop()
    session.transition('paused', 'Paused')
    expect(get(session)).toMatchObject({ visiblePhase: 'paused', voiceCanStop: false, engaged: true })
    unsubscribe()
  })

  it('reads a stop error from the microphone owner and resets only after it is released', async () => {
    const { session, voice, emit } = harness()
    session.begin({ request_id: 'request-1', title: 'Review' })
    await voice.start('request-1')
    emit({ type: 'error', sessionId: 'voice-1', code: 'device_lost', message: 'microphone lost' })
    expect(session.speechStopError()).toBe('microphone lost')
    expect(get(session).voiceCanStop).toBe(true)
    await voice.cancel()
    session.reset()
    expect(get(session)).toMatchObject({ phase: 'idle', requestId: '', startedOnce: false, voicePhase: 'idle', voiceLevel: 0 })
    expect(session.belongsToRequest(null)).toBe(true)
  })
})
