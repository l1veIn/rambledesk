import { describe, expect, it, vi } from 'vitest'
import type { SpeechRecognitionEvent } from '$lib/speech'
import { BROWSER_SPEECH_MODEL_ID, browserSpeechModelInfo } from './browserSpeechManifest'
import {
  createBrowserSpeechCapability,
  type BrowserSpeechEnvironment,
} from './browserSpeechCapability'

class FakeWorker {
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: OnErrorEventHandler = null
  terminated = false
  readonly commands: string[] = []
  postMessage(message: { type: string; sessionId: string }) {
    this.commands.push(message.type)
    if (message.type === 'init') queueMicrotask(() => this.emit({ type: 'ready', runtimeVersion: '1.13.7', runtimeGitSha: '917bed95c8e5c7c18aa4d69fea42e9ef8ef0a60e', ortVersion: 'test' }, message.sessionId))
    if (message.type === 'stop') queueMicrotask(() => {
      this.emit({ type: 'stable', segmentIndex: 0, text: '尾帧' }, message.sessionId)
      this.emit({ type: 'disposed', reason: 'stopped' }, message.sessionId)
    })
    if (message.type === 'cancel') queueMicrotask(() => {
      this.emit({ type: 'stable', segmentIndex: 0, text: '不应出现' }, message.sessionId)
      this.emit({ type: 'disposed', reason: 'cancelled' }, message.sessionId)
    })
  }
  terminate() { this.terminated = true }
  emit(message: Record<string, unknown>, sessionId = 'session-a') {
    if (!this.terminated) this.onmessage?.({ data: { v: 1, sessionId, ...message } } as MessageEvent)
  }
}

class FakeWorkletPort {
  onmessage: ((event: MessageEvent) => void) | null = null
  postMessage(message: { type: string }) {
    if (message.type === 'flush') queueMicrotask(() => this.onmessage?.({ data: { type: 'flushed', lastSeq: 4, droppedFrames: 0 } } as MessageEvent))
  }
}

function speechHarness(mediaPromise?: Promise<MediaStream>) {
  const worker = new FakeWorker()
  const track = { label: 'Test microphone', stop: vi.fn(), onended: null } as unknown as MediaStreamTrack
  const stream = { getTracks: () => [track], getAudioTracks: () => [track] } as unknown as MediaStream
  const port = new FakeWorkletPort()
  const worklet = { port, connect: vi.fn((_node) => _node), disconnect: vi.fn(), onprocessorerror: null } as unknown as AudioWorkletNode
  const source = { connect: vi.fn((_node) => _node), disconnect: vi.fn() } as unknown as MediaStreamAudioSourceNode
  const gain = { gain: { value: 1 }, connect: vi.fn((_node) => _node) } as unknown as GainNode
  const context = {
    state: 'running',
    audioWorklet: { addModule: vi.fn(async () => undefined) },
    createMediaStreamSource: vi.fn(() => source),
    createGain: vi.fn(() => gain),
    destination: {},
    resume: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  } as unknown as AudioContext
  const documentTarget = new EventTarget() as Document
  Object.defineProperty(documentTarget, 'visibilityState', { value: 'visible', configurable: true })
  const environment: BrowserSpeechEnvironment = {
    mediaDevices: {
      getUserMedia: vi.fn(() => mediaPromise ?? Promise.resolve(stream)),
      enumerateDevices: vi.fn(async () => []),
    } as unknown as MediaDevices,
    document: documentTarget,
    createWorker: () => worker as unknown as Worker,
    createAudioContext: () => context,
    createAudioWorkletNode: () => worklet,
    createMessageChannel: () => ({ port1: {}, port2: {} }) as MessageChannel,
    createSessionId: () => 'session-a',
    setTimeout,
    clearTimeout,
  }
  const modelStore = {
    listModels: async () => [browserSpeechModelInfo(true, [])],
    downloadModel: vi.fn(), deleteModel: vi.fn(), onProgress: vi.fn(() => () => {}),
  }
  const events: SpeechRecognitionEvent[] = []
  const onError = vi.fn()
  const capability = createBrowserSpeechCapability(modelStore as never, environment)
  return { capability, worker, track, stream, events, onError, listener: { onEvent: (event: SpeechRecognitionEvent) => events.push(event), onError } }
}

const OPTIONS = {
  inputDevice: null,
  modelId: BROWSER_SPEECH_MODEL_ID,
  vadThreshold: 0.5,
  vadSilenceMs: 700,
  hotwords: [],
} as const

describe('Browser SpeechRecognitionPlugin lifecycle', () => {
  it('flushes tail once, ignores stale events, and makes the first terminal intent idempotent', async () => {
    const base = speechHarness()
    const session = base.capability.start(OPTIONS, base.listener)
    await session.ready
    base.worker.emit({ type: 'stable', segmentIndex: 9, text: 'stale' }, 'old-session')
    await Promise.all([session.stop(), session.stop(), session.cancel()])
    base.worker.emit({ type: 'stable', segmentIndex: 10, text: 'late' })

    expect(base.worker.commands.filter((type) => type === 'stop')).toHaveLength(1)
    expect(base.worker.commands).not.toContain('cancel')
    expect(base.events.filter((event) => event.type === 'stable')).toEqual([
      { type: 'stable', sessionId: 'session-a', segmentIndex: 0, text: '尾帧' },
    ])
    expect(base.events.at(-1)).toEqual({ type: 'stopped', sessionId: 'session-a', reason: 'stopped' })
  })

  it('cancels abortively and suppresses queued worker tail', async () => {
    const base = speechHarness()
    const session = base.capability.start(OPTIONS, base.listener)
    await session.ready
    await session.cancel()
    await Promise.resolve()
    expect(base.events.some((event) => event.type === 'stable')).toBe(false)
    expect(base.events.at(-1)).toEqual({ type: 'stopped', sessionId: 'session-a', reason: 'cancelled' })
  })

  it('stops a getUserMedia result that resolves after cancellation', async () => {
    let resolveMedia: (stream: MediaStream) => void = () => {}
    const pending = new Promise<MediaStream>((resolve) => (resolveMedia = resolve))
    const base = speechHarness(pending)
    const session = base.capability.start(OPTIONS, base.listener)
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    await session.cancel()
    resolveMedia(base.stream)
    await Promise.resolve(); await Promise.resolve()
    await expect(session.ready).rejects.toThrow('stopped before it became ready')
    expect(base.track.stop).toHaveBeenCalled()
  })
})
