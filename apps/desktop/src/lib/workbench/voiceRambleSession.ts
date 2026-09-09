import { get, writable } from 'svelte/store'
import { tick } from 'svelte'

import { playRecordArmSound } from '../notifications'
import {
  eventBelongsToSpeechSession,
  stableSpeechSegmentId,
  stableTranscript,
  voiceStartStillLive,
  type SpeechRecognitionEvent,
  type SpeechRecognitionSession,
} from '../speech/speech'
import type { WorkbenchCapabilities } from '../capabilities/workbenchCapabilities'
import type { SpeechTarget } from './speechDraftQueue'
import type { VoicePhase } from './types'

export type VoiceRambleState = Readonly<{
  phase: VoicePhase
  requestId: string
  sessionId: string
  device: string
  partial: string
  level: number
  chunkIndex: number
  modelMissing: boolean
  message: string
}>

export type VoiceRambleContext = {
  speech: WorkbenchCapabilities['speech']
  tr: (source: string, values?: Record<string, string | number>) => string
  messageFrom: (cause: unknown) => string
  getInputDevice: () => string
  getModelId: () => string
  getVadThreshold: () => number
  getVadSilenceMs: () => number
  getHotwords: () => string[]
  getNotificationVolume: () => number
  /** Resolves the request/Action a speech segment belongs to. */
  resolveTarget: (event: SpeechRecognitionEvent) => SpeechTarget
  resetTargets: () => void
  onStable: (segmentId: string, transcript: string, target: SpeechTarget) => void
  onRecording: () => void
  onMicrophoneStopped: (message: string) => void
  onMicrophoneError: (message: string) => void
  /** Resolves once queued speech drafts are written. */
  waitForDrafts: () => Promise<void>
}

const initial: VoiceRambleState = {
  phase: 'idle',
  requestId: '',
  sessionId: '',
  device: '',
  partial: '',
  level: 0,
  chunkIndex: 0,
  modelMissing: false,
  message: '',
}

/**
 * The microphone session: starting/stopping speech recognition and turning its
 * events into the live voice state the workbench and the Ramble console read.
 */
export function createVoiceRambleSession(context: VoiceRambleContext) {
  const store = writable<VoiceRambleState>(initial)
  let speechSession: SpeechRecognitionSession | null = null

  function patch(next: Partial<VoiceRambleState>) {
    store.update((current) => ({ ...current, ...next }))
  }

  const phase = () => get(store).phase
  const active = () => {
    const current = phase()
    return (
      current === 'starting' ||
      current === 'listening' ||
      current === 'processing' ||
      current === 'stopping'
    )
  }
  const canStop = () => active() || (phase() === 'error' && get(store).sessionId.length > 0)

  function reset() {
    speechSession = null
    store.set(initial)
    context.resetTargets()
  }

  async function start(requestId: string): Promise<boolean> {
    if (!requestId || active() || speechSession) return false
    patch({
      phase: 'starting',
      requestId,
      sessionId: '',
      device: '',
      partial: '',
      level: 0,
      modelMissing: false,
      message: context.tr('Connecting the microphone…'),
    })
    context.resetTargets()
    void playRecordArmSound(context.getNotificationVolume())
    try {
      const session = context.speech.implementation.start(
        {
          inputDevice: context.getInputDevice() || null,
          modelId: context.getModelId(),
          vadThreshold: context.getVadThreshold(),
          vadSilenceMs: context.getVadSilenceMs(),
          hotwords: context.getHotwords(),
        },
        {
          onEvent: handleEvent,
          onError: (cause) => {
            patch({
              phase: 'error',
              message: context.tr('Cannot listen for speech events: {error}', {
                error: context.messageFrom(cause),
              }),
            })
          },
        },
      )
      speechSession = session
      patch({ sessionId: session.id })
      await session.ready
      if (!voiceStartStillLive(phase())) {
        await session.cancel().catch(() => {})
        if (speechSession === session) speechSession = null
        patch({ sessionId: '' })
        return false
      }
      if (phase() === 'starting') {
        patch({
          phase: 'listening',
          message: context.tr('VAD is listening · Transcribes automatically after each spoken segment'),
        })
      }
    } catch (cause) {
      speechSession = null
      const message = context.messageFrom(cause)
      patch({
        sessionId: '',
        phase: 'error',
        message,
        modelMissing: /not installed|尚未安装/.test(message),
      })
      return false
    }
    return true
  }

  async function stop(): Promise<boolean> {
    if (!canStop()) return true
    const session = speechSession
    if (!session) {
      reset()
      return true
    }
    patch({
      phase: 'stopping',
      message: context.tr('Finishing the final transcription segment…'),
    })
    try {
      await session.stop()
      for (let attempt = 0; attempt < 5 && phase() === 'stopping'; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 20))
      }
      await tick()
      await context.waitForDrafts()
      if (phase() === 'stopping') {
        patch({ phase: 'idle', message: context.tr('Recording stopped') })
      }
    } catch (cause) {
      patch({ phase: 'error', message: context.messageFrom(cause) })
      return false
    } finally {
      patch({ level: 0 })
    }
    return phase() !== 'error'
  }

  async function cancel() {
    await speechSession?.cancel().catch(() => {})
    speechSession = null
  }

  function handleEvent(event: SpeechRecognitionEvent) {
    const current = get(store)
    if (!current.requestId || !eventBelongsToSpeechSession(event, current.sessionId)) return
    const target = context.resolveTarget(event)
    switch (event.type) {
      case 'started':
        patch({
          phase: 'listening',
          device: event.inputDevice,
          message: context.tr('Recording · {device}', { device: event.inputDevice }),
        })
        context.onRecording()
        break
      case 'partial':
        patch({
          partial: event.text,
          phase: phase() !== 'stopping' ? 'listening' : phase(),
        })
        context.onRecording()
        break
      case 'level':
        patch({ level: Math.min(1, Math.max(0, event.rms * 8)) })
        context.onRecording()
        break
      case 'speech-started':
        if (phase() !== 'stopping') patch({ phase: 'listening' })
        break
      case 'processing':
        patch({
          chunkIndex: event.segmentIndex + 1,
          phase: phase() !== 'stopping' ? 'processing' : phase(),
          message: context.tr('Transcribing segment {count}…', { count: event.segmentIndex + 1 }),
        })
        context.onRecording()
        break
      case 'stable': {
        const transcript = stableTranscript(event)
        if (transcript) {
          context.onStable(stableSpeechSegmentId(event), transcript, target)
        }
        patch({
          partial: '',
          chunkIndex: event.segmentIndex + 1,
          phase: phase() !== 'stopping' ? 'listening' : phase(),
          message: context.tr('Listening…'),
        })
        context.onRecording()
        break
      }
      case 'warning':
        patch({ message: event.message })
        break
      case 'stopped': {
        const unexpected = event.reason === 'unexpected' || phase() === 'error'
        patch({
          phase: unexpected ? 'error' : 'idle',
          sessionId: '',
          level: 0,
          partial: '',
          message: unexpected
            ? context.tr('The microphone stopped unexpectedly; Ramble is paused')
            : context.tr('Recording stopped'),
        })
        speechSession = null
        if (unexpected) {
          context.onMicrophoneStopped(
            context.tr('The microphone stopped unexpectedly; Ramble is paused'),
          )
        }
        break
      }
      case 'error':
        patch({
          phase: 'error',
          level: 0,
          partial: '',
          message: event.message,
        })
        context.onMicrophoneError(event.message)
        break
    }
  }

  return {
    subscribe: store.subscribe,
    start,
    stop,
    reset,
    cancel,
    handleEvent,
    phase,
    active,
    canStop,
    state: () => get(store),
  }
}

export type VoiceRambleSession = ReturnType<typeof createVoiceRambleSession>
