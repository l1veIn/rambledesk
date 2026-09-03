import type {
  SpeechAdministrationCapability,
  SpeechRecognitionOptions,
  SpeechRecognitionPlugin,
} from '../workbenchCapabilities'
import type {
  SpeechRecognitionEvent,
  SpeechRecognitionListener,
  SpeechRecognitionSession,
  SpeechRecognitionStopReason,
} from '$lib/speech'
import { subscribeToTauriEvent } from './subscription'
import type { TauriCapabilityApi, TauriUnlisten } from './tauriCapabilityApi'

type NativeSpeechEvent =
  | Readonly<{
      type: 'started'
      recognition_session_id: string
      input_device: string
      provider: string
    }>
  | Readonly<{
      type: 'partial'
      recognition_session_id: string
      text: string
    }>
  | Readonly<{
      type: 'level'
      recognition_session_id: string
      rms: number
    }>
  | Readonly<{
      type: 'processing' | 'speech_started'
      recognition_session_id: string
      chunk_index: number
    }>
  | Readonly<{
      type: 'stable'
      recognition_session_id: string
      chunk_index: number
      text: string
    }>
  | Readonly<{
      type: 'warning'
      recognition_session_id: string
      code: string
      message: string
    }>
  | Readonly<{
      type: 'stopped'
      recognition_session_id: string
    }>
  | Readonly<{
      type: 'error'
      recognition_session_id: string
      code: string
      message: string
    }>

type NativeSpeechSessionView = Readonly<{
  recognition_session_id: string
  provider: string
  model_path: string
}>

type TerminalIntent = 'stop' | 'cancel'

class SpeechSessionTerminatedBeforeReadyError extends Error {
  constructor() {
    super('Speech recognition stopped before it became ready.')
    this.name = 'SpeechSessionTerminatedBeforeReadyError'
  }
}

export function createTauriSpeechCapability(
  api: TauriCapabilityApi,
  createSessionId: () => string = () => crypto.randomUUID(),
): SpeechRecognitionPlugin & SpeechAdministrationCapability {
  let activeSessionId: string | null = null

  return {
    start: (options, listener) => {
      const sessionId = createSessionId()
      if (activeSessionId !== null) {
        return failedSession(
          sessionId,
          new Error('A speech recognition session is already active.'),
        )
      }
      activeSessionId = sessionId
      return createSession(api, sessionId, options, listener, () => {
        if (activeSessionId === sessionId) activeSessionId = null
      })
    },
    listModels: () => api.invoke('list_speech_models'),
    downloadModel: (modelId) => api.invoke('download_speech_model', { modelId }),
    deleteModel: (modelId) => api.invoke('delete_speech_model', { modelId }),
    listInputDevices: () => api.invoke('list_speech_input_devices'),
    onModelProgress: (handler, onError) =>
      subscribeToTauriEvent(api, 'speech-model-progress', handler, onError),
  }
}

function createSession(
  api: TauriCapabilityApi,
  sessionId: string,
  options: SpeechRecognitionOptions,
  listener: SpeechRecognitionListener,
  releaseSlot: () => void,
): SpeechRecognitionSession {
  let active = true
  let unlisten: TauriUnlisten | undefined
  let terminalIntent: TerminalIntent | null = null
  let termination: Promise<void> | null = null
  let startIssued = false
  let startCommand: Promise<NativeSpeechSessionView> | null = null
  let resolveStartDecision: () => void = () => {}
  const startDecision = new Promise<void>((resolve) => {
    resolveStartDecision = resolve
  })
  let resolveTerminal: () => void = () => {}
  let rejectTerminal: (cause: unknown) => void = () => {}
  const terminal = new Promise<void>((resolve, reject) => {
    resolveTerminal = resolve
    rejectTerminal = reject
  })

  const finish = (reason: SpeechRecognitionStopReason) => {
    if (!active) return
    active = false
    unlisten?.()
    unlisten = undefined
    releaseSlot()
    listener.onEvent({ type: 'stopped', sessionId, reason })
    resolveTerminal()
  }

  const fail = (cause: unknown) => {
    if (!active) return
    active = false
    unlisten?.()
    unlisten = undefined
    releaseSlot()
    listener.onError(cause)
    resolveTerminal()
  }

  const handleNativeEvent = (wire: NativeSpeechEvent) => {
    if (!active || wire.recognition_session_id !== sessionId) return
    if (wire.type === 'stopped') {
      const reason = terminalIntent === 'cancel'
        ? 'cancelled'
        : terminalIntent === 'stop'
          ? 'stopped'
          : 'unexpected'
      finish(reason)
      return
    }
    // The native compatibility facade currently uses graceful shutdown for
    // both paths. Cancellation still has abortive public semantics because no
    // tail or queued recognition event escapes after the caller cancels.
    if (terminalIntent === 'cancel') return
    listener.onEvent(mapNativeSpeechEvent(wire))
  }

  const ready = (async () => {
    try {
      unlisten = await api.listen<NativeSpeechEvent>(
        'voice-ramble-event',
        ({ payload }) => handleNativeEvent(payload),
      )
      if (terminalIntent !== null) {
        resolveStartDecision()
        finish('cancelled')
        throw new SpeechSessionTerminatedBeforeReadyError()
      }
      startIssued = true
      startCommand = api.invoke<NativeSpeechSessionView>('start_voice_ramble', {
        input: {
          recognition_session_id: sessionId,
          input_device: options.inputDevice,
          model_id: options.modelId,
          vad_threshold: options.vadThreshold,
          vad_silence_ms: options.vadSilenceMs,
          hotwords: [...options.hotwords],
        },
      })
      resolveStartDecision()
      const started = await startCommand
      if (started.recognition_session_id !== sessionId) {
        throw new Error('Native speech recognition returned a different session id.')
      }
      if (terminalIntent !== null) {
        throw new SpeechSessionTerminatedBeforeReadyError()
      }
    } catch (cause) {
      resolveStartDecision()
      if (terminalIntent === null) fail(cause)
      throw cause
    }
  })()

  function terminate(intent: TerminalIntent): Promise<void> {
    if (!active) return terminal
    if (terminalIntent !== null) return termination ?? terminal
    terminalIntent = intent
    termination = runTermination()
    return termination
  }

  async function runTermination(): Promise<void> {
    await startDecision
    if (!active) return terminal
    if (!startIssued || startCommand === null) {
      finish('cancelled')
      return terminal
    }
    try {
      await startCommand
      if (!active) return terminal
      await api.invoke<void>('stop_voice_ramble')
      return terminal
    } catch (cause) {
      if (active) {
        listener.onError(cause)
        listener.onEvent({ type: 'stopped', sessionId, reason: 'unexpected' })
        active = false
        unlisten?.()
        unlisten = undefined
        releaseSlot()
        rejectTerminal(cause)
      }
      return terminal
    }
  }

  return Object.freeze({
    id: sessionId,
    ready,
    stop: () => terminate('stop'),
    cancel: () => terminate('cancel'),
  })
}

function failedSession(sessionId: string, cause: unknown): SpeechRecognitionSession {
  const ready = Promise.reject(cause)
  return Object.freeze({
    id: sessionId,
    ready,
    stop: async () => undefined,
    cancel: async () => undefined,
  })
}

function mapNativeSpeechEvent(
  wire: Exclude<NativeSpeechEvent, { type: 'stopped' }>,
): SpeechRecognitionEvent {
  switch (wire.type) {
    case 'started':
      return {
        type: 'started',
        sessionId: wire.recognition_session_id,
        inputDevice: wire.input_device,
        provider: wire.provider,
      }
    case 'partial':
      return { type: 'partial', sessionId: wire.recognition_session_id, text: wire.text }
    case 'level':
      return { type: 'level', sessionId: wire.recognition_session_id, rms: wire.rms }
    case 'speech_started':
      return {
        type: 'speech-started', sessionId: wire.recognition_session_id, segmentIndex: wire.chunk_index,
      }
    case 'processing':
      return {
        type: 'processing',
        sessionId: wire.recognition_session_id,
        segmentIndex: wire.chunk_index,
      }
    case 'stable':
      return {
        type: 'stable',
        sessionId: wire.recognition_session_id,
        segmentIndex: wire.chunk_index,
        text: wire.text,
      }
    case 'warning':
      return {
        type: 'warning',
        sessionId: wire.recognition_session_id,
        code: wire.code,
        message: wire.message,
      }
    case 'error':
      return {
        type: 'error',
        sessionId: wire.recognition_session_id,
        code: wire.code,
        message: wire.message,
      }
  }
}
