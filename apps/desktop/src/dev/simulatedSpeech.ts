import { get, writable } from 'svelte/store'
import { createUnavailableWorkbenchCapabilities } from '../lib/capabilities/unavailableCapabilities'
import type { WorkbenchCapabilities } from '../lib/capabilities/workbenchCapabilities'
import type { SpeechRecognitionListener } from '../lib/speech/speech'

/** Only the hardware boundary is simulated; the real controller owns targets and writes. */
export function createSimulatedSpeech() {
  const state = writable({ recording: false, segment: null as number | null })
  let listener: SpeechRecognitionListener | null = null
  let sessionId = ''
  let nextIndex = 0
  let partial = ''
  function finish(text = partial) {
    const segment = get(state).segment
    if (!listener || segment === null) return
    listener.onEvent({ type: 'processing', sessionId, segmentIndex: segment })
    listener.onEvent({ type: 'stable', sessionId, segmentIndex: segment, text })
    state.update((value) => ({ ...value, segment: null }))
  }
  const base = createUnavailableWorkbenchCapabilities()
  const capabilities: WorkbenchCapabilities = { ...base, speech: {
    status: { availability: 'available', source: 'native' },
    implementation: { ...base.speech.implementation,
      start: (_options, next) => {
        listener = next; sessionId = `simulated-${crypto.randomUUID()}`; nextIndex = 0
        state.set({ recording: true, segment: null })
        const id = sessionId
        return { id, ready: Promise.resolve(),
          stop: async () => { finish(); listener?.onEvent({ type: 'stopped', sessionId: id, reason: 'stopped' }); listener = null; state.set({ recording: false, segment: null }) },
          cancel: async () => { listener?.onEvent({ type: 'stopped', sessionId: id, reason: 'cancelled' }); listener = null; state.set({ recording: false, segment: null }) },
        }
      },
    },
  } }
  const drafts = new Map<string, string>()
  return {
    subscribe: state.subscribe, capabilities,
    storage: { getItem: (key: string) => drafts.get(key) ?? null, setItem: (key: string, value: string) => { drafts.set(key, value) } },
    begin(text: string) {
      if (!listener || !get(state).recording || get(state).segment !== null) return
      const segmentIndex = nextIndex++
      partial = text
      state.update((value) => ({ ...value, segment: segmentIndex }))
      listener.onEvent({ type: 'speech-started', sessionId, segmentIndex })
      listener.onEvent({ type: 'partial', sessionId, text })
      listener.onEvent({ type: 'level', sessionId, rms: 0.5 })
    },
    finish,
  }
}
