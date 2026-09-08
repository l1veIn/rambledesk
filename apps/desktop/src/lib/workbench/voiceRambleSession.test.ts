import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'

import type {
  SpeechRecognitionEvent,
  SpeechRecognitionListener,
  SpeechRecognitionSession,
} from '../speech'
import type { SpeechRecognitionPlugin } from '../capabilities/workbenchCapabilities'
import {
  createVoiceRambleSession,
  type VoiceRambleContext,
} from './voiceRambleSession'

function harness(overrides: Partial<VoiceRambleContext> = {}) {
  let listener: SpeechRecognitionListener | undefined
  let ready = Promise.resolve()
  const session: SpeechRecognitionSession = {
    id: 'voice-1',
    get ready() {
      return ready
    },
    stop: vi.fn(async () => undefined),
    cancel: vi.fn(async () => undefined),
  }
  const speech = {
    status: { availability: 'available', source: 'native' } as const,
    implementation: {
      start: vi.fn((_options, next: SpeechRecognitionListener) => {
        listener = next
        return session
      }),
    } as SpeechRecognitionPlugin,
  }
  const context = {
    speech,
    tr: (source: string, values?: Record<string, string | number>) =>
      values ? `${source}:${JSON.stringify(values)}` : source,
    messageFrom: (cause: unknown) => String(cause),
    getInputDevice: () => 'default',
    getModelId: () => 'model',
    getVadThreshold: () => 0.5,
    getVadSilenceMs: () => 500,
    getHotwords: () => ['ramble'],
    getNotificationVolume: () => 0,
    resolveTarget: () => ({ requestId: 'request-1', requestTitle: 'Task', action: null }),
    resetTargets: vi.fn(),
    onStable: vi.fn(),
    onRecording: vi.fn(),
    onMicrophoneStopped: vi.fn(),
    onMicrophoneError: vi.fn(),
    waitForDrafts: vi.fn(async () => undefined),
    ...overrides,
  } as VoiceRambleContext
  return {
    voice: createVoiceRambleSession(context),
    context,
    session,
    emit: (event: SpeechRecognitionEvent) => listener?.onEvent(event),
    fail: (cause: unknown) => listener?.onError?.(cause),
    deferReady: () => {
      let resolve!: () => void
      ready = new Promise<void>((done) => (resolve = done))
      return resolve
    },
  }
}

function event(partial: Partial<SpeechRecognitionEvent> & { type: SpeechRecognitionEvent['type'] }) {
  return { sessionId: 'voice-1', ...partial } as SpeechRecognitionEvent
}

describe('voice ramble session', () => {
  it('starts a microphone session and reports live state', async () => {
    const { voice, context, emit } = harness()

    await expect(voice.start('request-1')).resolves.toBe(true)
    expect(get(voice)).toMatchObject({ phase: 'listening', requestId: 'request-1', sessionId: 'voice-1' })

    emit(event({ type: 'started', inputDevice: 'Built-in' } as never))
    expect(get(voice)).toMatchObject({ phase: 'listening', device: 'Built-in' })
    expect(context.onRecording).toHaveBeenCalled()
  })

  it('refuses to start a second session while one is live', async () => {
    const { voice } = harness()
    await voice.start('request-1')
    await expect(voice.start('request-1')).resolves.toBe(false)
    expect(get(voice).sessionId).toBe('voice-1')
  })

  it('reports a start failure and remembers a missing model', async () => {
    const { voice } = harness({
      speech: {
        status: { availability: 'available', source: 'native' },
        implementation: {
          start: () => {
            throw new Error('The speech model is not installed')
          },
        },
      } as never,
    })

    await expect(voice.start('request-1')).resolves.toBe(false)
    expect(get(voice)).toMatchObject({ phase: 'error', modelMissing: true })
  })

  it('turns a stable segment into a draft for the resolved target', async () => {
    const { voice, context, emit } = harness()
    await voice.start('request-1')

    emit(event({
      type: 'stable',
      segmentId: 'segment-1',
      segmentIndex: 0,
      text: '  Hello world  ',
    } as never))

    expect(context.onStable).toHaveBeenCalledWith('asr-voice-1-0', 'Hello world', {
      requestId: 'request-1',
      requestTitle: 'Task',
      action: null,
    })
    expect(get(voice).partial).toBe('')
    expect(get(voice).message).toContain('Listening…')
  })

  it('pauses Ramble when the microphone stops unexpectedly', async () => {
    const { voice, context, emit } = harness()
    await voice.start('request-1')

    emit(event({ type: 'stopped', reason: 'unexpected' } as never))

    expect(get(voice)).toMatchObject({ phase: 'error', sessionId: '' })
    expect(context.onMicrophoneStopped).toHaveBeenCalled()
  })

  it('stops the session, waits for drafts, and returns to idle', async () => {
    const { voice, context, session } = harness()
    await voice.start('request-1')

    await expect(voice.stop()).resolves.toBe(true)

    expect(session.stop).toHaveBeenCalled()
    expect(context.waitForDrafts).toHaveBeenCalled()
    expect(get(voice)).toMatchObject({ phase: 'idle', level: 0 })
  })

  it('surfaces a recognition error to the caller', async () => {
    const { voice, context, emit } = harness()
    await voice.start('request-1')

    emit(event({ type: 'error', message: 'Microphone permission denied' } as never))

    expect(get(voice)).toMatchObject({ phase: 'error', message: 'Microphone permission denied' })
    expect(context.onMicrophoneError).toHaveBeenCalledWith('Microphone permission denied')
  })

  it('resets every voice field and the speech target tracker', () => {
    const { voice, context } = harness()
    voice.reset()
    expect(get(voice)).toEqual({
      phase: 'idle',
      requestId: '',
      sessionId: '',
      device: '',
      partial: '',
      level: 0,
      chunkIndex: 0,
      modelMissing: false,
      message: '',
    })
    expect(context.resetTargets).toHaveBeenCalled()
  })
})
