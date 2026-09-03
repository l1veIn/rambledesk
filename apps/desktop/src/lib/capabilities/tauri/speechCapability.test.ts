import { describe, expect, it, vi } from 'vitest'

import type { SpeechRecognitionEvent } from '$lib/speech'
import { createTauriSpeechCapability } from './speechCapability'
import type {
  TauriCapabilityApi,
  TauriEvent,
  TauriUnlisten,
} from './tauriCapabilityApi'

type NativeEvent = Readonly<Record<string, unknown> & {
  type: string
  recognition_session_id: string
}>

type SpeechHarness = Readonly<{
  api: TauriCapabilityApi
  invoke: ReturnType<typeof vi.fn>
  listen: ReturnType<typeof vi.fn>
  unlisten: ReturnType<typeof vi.fn>
  emit(event: NativeEvent): void
}>

const OPTIONS = Object.freeze({
  inputDevice: null,
  modelId: 'sense-voice-small',
  vadThreshold: 0.4,
  vadSilenceMs: 650,
  hotwords: ['RambleDesk'],
})

function harness(
  onStop: (emit: (event: NativeEvent) => void) => void = (emit) => {
    emit({ type: 'stopped', recognition_session_id: 'session-a' })
  },
): SpeechHarness {
  let handler: ((event: TauriEvent<NativeEvent>) => void) | undefined
  const unlisten = vi.fn()
  const emit = (event: NativeEvent) => handler?.({ payload: event })
  const listen = vi.fn(async (_event: string, next: typeof handler): Promise<TauriUnlisten> => {
    handler = next
    return unlisten
  })
  const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'start_voice_ramble') {
      const input = args?.input as { recognition_session_id: string }
      return {
        recognition_session_id: input.recognition_session_id,
        provider: 'sense_voice',
        model_path: '/models/sense-voice',
      }
    }
    if (command === 'stop_voice_ramble') onStop(emit)
    return undefined
  })
  return {
    api: { invoke, listen } as unknown as TauriCapabilityApi,
    invoke,
    listen,
    unlisten,
    emit,
  }
}

function listener() {
  const events: SpeechRecognitionEvent[] = []
  const onError = vi.fn()
  return {
    events,
    onError,
    value: {
      onEvent: (event: SpeechRecognitionEvent) => events.push(event),
      onError,
    },
  }
}

describe('Tauri SpeechRecognitionPlugin contract', () => {
  it('registers the event listener before it invokes native start', async () => {
    let resolveListen: ((unlisten: TauriUnlisten) => void) | undefined
    const base = harness()
    base.listen.mockImplementationOnce(
      () => new Promise<TauriUnlisten>((resolve) => (resolveListen = resolve)),
    )
    const speech = createTauriSpeechCapability(base.api, () => 'session-a')
    const observed = listener()

    const session = speech.start(OPTIONS, observed.value)
    expect(session.id).toBe('session-a')
    expect(base.listen).toHaveBeenCalledWith('voice-ramble-event', expect.any(Function))
    expect(base.invoke).not.toHaveBeenCalled()

    resolveListen?.(base.unlisten as unknown as TauriUnlisten)
    await session.ready

    expect(base.invoke.mock.invocationCallOrder[0]).toBeGreaterThan(
      base.listen.mock.invocationCallOrder[0] ?? 0,
    )
    expect(base.invoke).toHaveBeenCalledWith('start_voice_ramble', {
      input: {
        recognition_session_id: 'session-a',
        input_device: null,
        model_id: 'sense-voice-small',
        vad_threshold: 0.4,
        vad_silence_ms: 650,
        hotwords: ['RambleDesk'],
      },
    })
  })

  it('maps only events belonging to its own session', async () => {
    const base = harness()
    const observed = listener()
    const session = createTauriSpeechCapability(base.api, () => 'session-a')
      .start(OPTIONS, observed.value)
    await session.ready

    base.emit({
      type: 'stable',
      recognition_session_id: 'old-session',
      chunk_index: 0,
      text: '旧内容',
    })
    base.emit({
      type: 'speech_started',
      recognition_session_id: 'session-a',
      chunk_index: 2,
    })
    base.emit({
      type: 'stable',
      recognition_session_id: 'session-a',
      chunk_index: 2,
      text: '新内容',
    })

    expect(observed.events).toEqual([
      { type: 'speech-started', sessionId: 'session-a', segmentIndex: 2 },
      { type: 'stable', sessionId: 'session-a', segmentIndex: 2, text: '新内容' },
    ])
  })

  it('gracefully stops once and preserves native tail events before stopped', async () => {
    const base = harness((emit) => {
      emit({
        type: 'stable',
        recognition_session_id: 'session-a',
        chunk_index: 3,
        text: '尾帧',
      })
      emit({ type: 'stopped', recognition_session_id: 'session-a' })
    })
    const observed = listener()
    const session = createTauriSpeechCapability(base.api, () => 'session-a')
      .start(OPTIONS, observed.value)
    await session.ready

    await Promise.all([session.stop(), session.stop()])

    expect(base.invoke.mock.calls.filter(([command]) => command === 'stop_voice_ramble')).toHaveLength(1)
    expect(observed.events).toEqual([
      { type: 'stable', sessionId: 'session-a', segmentIndex: 3, text: '尾帧' },
      { type: 'stopped', sessionId: 'session-a', reason: 'stopped' },
    ])
    expect(base.unlisten).toHaveBeenCalledOnce()
  })

  it('cancels once and suppresses native tail recognition output', async () => {
    const base = harness((emit) => {
      emit({
        type: 'stable',
        recognition_session_id: 'session-a',
        chunk_index: 4,
        text: '不应写入',
      })
      emit({ type: 'stopped', recognition_session_id: 'session-a' })
    })
    const observed = listener()
    const session = createTauriSpeechCapability(base.api, () => 'session-a')
      .start(OPTIONS, observed.value)
    await session.ready

    await Promise.all([session.cancel(), session.stop(), session.cancel()])

    expect(base.invoke.mock.calls.filter(([command]) => command === 'stop_voice_ramble')).toHaveLength(1)
    expect(observed.events).toEqual([
      { type: 'stopped', sessionId: 'session-a', reason: 'cancelled' },
    ])
  })

  it('can cancel while listener registration is still pending without starting native audio', async () => {
    let resolveListen: ((unlisten: TauriUnlisten) => void) | undefined
    const base = harness()
    base.listen.mockImplementationOnce(
      () => new Promise<TauriUnlisten>((resolve) => (resolveListen = resolve)),
    )
    const observed = listener()
    const session = createTauriSpeechCapability(base.api, () => 'session-a')
      .start(OPTIONS, observed.value)

    const cancelled = session.cancel()
    resolveListen?.(base.unlisten as unknown as TauriUnlisten)

    await cancelled
    await expect(session.ready).rejects.toThrow('stopped before it became ready')
    expect(base.invoke).not.toHaveBeenCalledWith('start_voice_ramble', expect.anything())
    expect(observed.events).toEqual([
      { type: 'stopped', sessionId: 'session-a', reason: 'cancelled' },
    ])
  })
})
