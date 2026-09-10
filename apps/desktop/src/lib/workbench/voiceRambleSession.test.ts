import { get } from 'svelte/store'
import { describe, expect, it, vi } from 'vitest'

import type {
  SpeechRecognitionEvent,
  SpeechRecognitionListener,
  SpeechRecognitionSession,
} from '../speech/speech'
import type { SpeechRecognitionPlugin } from '../capabilities/workbenchCapabilities'
import { createUnavailableWorkbenchCapabilities } from '../capabilities/unavailableCapabilities'
import { createSpeechDraftQueue, type SpeechTarget } from '../speech/speechDraftQueue'
import { createSpeechTargetTracker } from '../speech/speechTargetTracker'
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

function deferred() {
  let resolve!: () => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<void>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}

function lifecycleHarness() {
  let target: SpeechTarget = { requestId: 'request-1', requestTitle: 'First', action: null }
  const tracker = createSpeechTargetTracker(() => target)
  const committed: { requestId: string; text: string }[] = []
  let writeBarrier: Promise<void> = Promise.resolve()
  const queue = createSpeechDraftQueue({
    write: async (requestId, operation) => {
      await writeBarrier
      if (operation.kind === 'appendSpeech') committed.push({ requestId, text: operation.text })
    },
  })
  const sessions: {
    id: string
    ready: ReturnType<typeof deferred>
    stopped: ReturnType<typeof deferred>
    cancelled: ReturnType<typeof deferred>
    listener: SpeechRecognitionListener
  }[] = []
  const { voice } = harness({
    speech: {
      status: { availability: 'available', source: 'native' },
      implementation: {
        ...createUnavailableWorkbenchCapabilities().speech.implementation,
        start: (_options, listener) => {
          const session = {
            id: `voice-${sessions.length + 1}`, ready: deferred(),
            stopped: deferred(), cancelled: deferred(), listener,
          }
          sessions.push(session)
          return {
            id: session.id, ready: session.ready.promise,
            stop: () => session.stopped.promise,
            cancel: () => session.cancelled.promise,
          }
        },
      },
    },
    resolveTarget: (value) => tracker.observe(value) ?? target,
    resetTargets: () => tracker.reset(),
    onStable: (id, text, owner) => { void queue.enqueue(id, text, owner, false) },
    waitForDrafts: () => queue.settled(),
  })
  return {
    voice, sessions, queue, committed,
    target: (next: SpeechTarget) => { target = next },
    blockWrites: () => {
      const pending = deferred()
      writeBarrier = pending.promise
      return pending.resolve
    },
  }
}

describe('voice ramble session', () => {
  it('keeps a reset-and-restarted microphone when old cancellation finishes', async () => {
    const { voice, sessions } = lifecycleHarness()
    const first = voice.start('request-1')
    sessions[0].ready.resolve()
    await first
    const cancellation = voice.cancel()
    voice.reset()
    const replacement = voice.start('request-2')
    sessions[1].ready.resolve()
    await replacement
    sessions[0].cancelled.resolve()
    await cancellation
    expect(get(voice)).toMatchObject({ phase: 'listening', sessionId: 'voice-2' })
  })

  it('waits for the final stable segment to be written to its speech-onset target', async () => {
    const { voice, sessions, queue, committed, target, blockWrites } = lifecycleHarness()
    const starting = voice.start('request-1')
    sessions[0].ready.resolve()
    await starting
    const session = sessions[0]
    session.listener.onEvent({ type: 'speech-started', sessionId: session.id, segmentIndex: 0 })
    target({ requestId: 'request-2', requestTitle: 'Second', action: null })
    const releaseWrite = blockWrites()
    let finished = false
    const stopping = voice.stop().then((result) => { finished = true; return result })
    session.listener.onEvent({
      type: 'stable', sessionId: session.id, segmentIndex: 0, text: 'Final words for the first request',
    })
    session.listener.onEvent({ type: 'stopped', sessionId: session.id, reason: 'stopped' })
    session.stopped.resolve()
    // Let the capability stop and Svelte tick finish while the real queue write is held.
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(finished).toBe(false)
    expect(committed).toEqual([])
    expect(get(queue).drafts).toMatchObject([{ requestId: 'request-1', status: 'writing' }])

    releaseWrite()
    await expect(stopping).resolves.toBe(true)
    expect(committed).toEqual([{ requestId: 'request-1', text: 'Final words for the first request' }])
    expect(get(queue).drafts).toEqual([])
    expect(get(voice)).toMatchObject({ phase: 'idle', sessionId: '' })
  })

  it.each(['resolve', 'reject'] as const)(
    'keeps an intentional stop while startup %ss late',
    async (outcome) => {
      const { voice, sessions } = lifecycleHarness()
      const starting = voice.start('request-1')
      const stopping = voice.stop()
      const session = sessions[0]
      session.listener.onEvent({
        type: 'started', sessionId: session.id, inputDevice: 'Built-in', provider: 'fixture',
      })
      session.cancelled.resolve()
      if (outcome === 'resolve') session.ready.resolve()
      else session.ready.reject(new Error('Speech recognition stopped before it became ready.'))
      await expect(starting).resolves.toBe(false)
      expect(get(voice).phase).toBe('stopping')

      session.listener.onEvent({ type: 'stopped', sessionId: session.id, reason: 'stopped' })
      session.stopped.resolve()
      await expect(stopping).resolves.toBe(true)
      expect(get(voice)).toMatchObject({ phase: 'idle', sessionId: '' })
    },
  )

  it.each(['resolve', 'reject'] as const)(
    'does not change a replacement when an old stop %ss late',
    async (outcome) => {
      const { voice, sessions } = lifecycleHarness()
      const first = voice.start('request-1')
      sessions[0].ready.resolve()
      await first
      const stopping = voice.stop()
      const cancellation = voice.cancel()
      sessions[0].cancelled.resolve()
      await cancellation
      const replacement = voice.start('request-2')
      sessions[1].ready.resolve()
      await replacement
      sessions[1].listener.onEvent({ type: 'level', sessionId: 'voice-2', rms: 0.1 })

      if (outcome === 'reject') sessions[0].stopped.reject(new Error('Old stop failed'))
      else sessions[0].stopped.resolve()
      await expect(stopping).resolves.toBe(false)
      expect(get(voice)).toMatchObject({ phase: 'listening', sessionId: 'voice-2', level: 0.8 })
    },
  )

  it('ignores old recognition errors and segments after another microphone starts', async () => {
    const { voice, sessions, queue } = lifecycleHarness()
    const first = voice.start('request-1')
    sessions[0].ready.resolve()
    await first
    const cancellation = voice.cancel()
    sessions[0].cancelled.resolve()
    await cancellation
    const replacement = voice.start('request-2')
    sessions[1].ready.resolve()
    await replacement

    sessions[0].listener.onError(new Error('Old subscription failed'))
    sessions[0].listener.onEvent({
      type: 'stable', sessionId: 'voice-1', segmentIndex: 0, text: 'Old words',
    })
    expect(get(voice)).toMatchObject({ phase: 'listening', sessionId: 'voice-2' })
    expect(get(queue).drafts).toEqual([])
  })

  it('cancels pending startup without accepting tail words and can start again', async () => {
    const { voice, sessions, queue } = lifecycleHarness()
    const first = voice.start('request-1')
    const cancellation = voice.cancel()
    sessions[0].listener.onEvent({
      type: 'stable', sessionId: 'voice-1', segmentIndex: 0, text: 'Cancelled tail',
    })
    expect(get(queue).drafts).toEqual([])
    sessions[0].cancelled.resolve()
    await cancellation

    const replacement = voice.start('request-2')
    expect(sessions).toHaveLength(2)
    sessions[1].ready.resolve()
    await expect(replacement).resolves.toBe(true)
    sessions[0].ready.resolve()
    await expect(first).resolves.toBe(false)
    expect(get(voice)).toMatchObject({ phase: 'listening', sessionId: 'voice-2' })
  })

  it('keeps the replacement recording when a cancelled start rejects late', async () => {
    const { voice, sessions } = lifecycleHarness()
    const first = voice.start('request-1')
    const cancellation = voice.cancel()
    sessions[0].cancelled.resolve()
    await cancellation
    voice.reset()

    const replacement = voice.start('request-2')
    sessions[1].ready.resolve()
    await expect(replacement).resolves.toBe(true)
    sessions[0].ready.reject(new Error('The old microphone start was cancelled'))
    await expect(first).resolves.toBe(false)

    expect(get(voice)).toMatchObject({
      phase: 'listening', requestId: 'request-2', sessionId: 'voice-2', modelMissing: false,
    })
  })

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
