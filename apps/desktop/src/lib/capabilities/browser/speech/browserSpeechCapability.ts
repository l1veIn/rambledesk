import type {
  SpeechAdministrationCapability,
  SpeechRecognitionOptions,
  SpeechRecognitionPlugin,
} from '../../workbenchCapabilities'
import type {
  SpeechRecognitionListener,
  SpeechRecognitionSession,
  SpeechRecognitionStopReason,
} from '$lib/speech'
import { BrowserModelStore, BrowserSpeechError } from './browserModelStore'
import {
  BROWSER_SPEECH_CACHE_NAME,
  BROWSER_SPEECH_MODEL_FILES,
  BROWSER_SPEECH_MODEL_ID,
  BROWSER_SPEECH_MARKER_URL,
  BROWSER_SPEECH_RUNTIME,
} from './browserSpeechManifest'
import { BROWSER_SPEECH_PROTOCOL_VERSION, isWorkerEvent } from './protocol'

export type BrowserSpeechSupport = Readonly<{
  supported: boolean
  code?: string
  message?: string
}>

export type BrowserSpeechEnvironment = Readonly<{
  mediaDevices: MediaDevices
  document: Document
  createWorker(url: string): Worker
  createAudioContext(): AudioContext
  createAudioWorkletNode(context: AudioContext): AudioWorkletNode
  createMessageChannel(): MessageChannel
  createSessionId(): string
  setTimeout: typeof globalThis.setTimeout
  clearTimeout: typeof globalThis.clearTimeout
}>

export function detectBrowserSpeechSupport(scope: typeof globalThis = globalThis): BrowserSpeechSupport {
  if (!scope.isSecureContext) return unsupported('browser_insecure_context', 'Browser speech requires a secure context.')
  if (!scope.navigator?.mediaDevices?.getUserMedia) return unsupported('media_devices_unavailable', 'getUserMedia is unavailable.')
  if (!('AudioContext' in scope) || !('AudioWorkletNode' in scope)) return unsupported('audio_worklet_unavailable', 'AudioWorklet is unavailable.')
  if (!('Worker' in scope)) return unsupported('worker_unavailable', 'Dedicated Worker is unavailable.')
  if (!('WebAssembly' in scope)) return unsupported('webassembly_unavailable', 'WebAssembly is unavailable.')
  if (!('caches' in scope)) return unsupported('cache_storage_unavailable', 'Cache Storage is unavailable.')
  return Object.freeze({ supported: true })
}

export function createBrowserSpeechCapability(
  modelStore: Pick<BrowserModelStore, 'listModels' | 'downloadModel' | 'deleteModel' | 'onProgress'> = new BrowserModelStore(),
  environment: BrowserSpeechEnvironment = browserSpeechEnvironment(),
): SpeechRecognitionPlugin & SpeechAdministrationCapability {
  let activeSessionId: string | null = null
  return {
    start(options, listener) {
      const id = environment.createSessionId()
      if (activeSessionId !== null) return failedSession(id, new BrowserSpeechError('session_active', 'A browser speech session is already active.'))
      activeSessionId = id
      return createSession(environment, modelStore, id, options, listener, () => {
        if (activeSessionId === id) activeSessionId = null
      })
    },
    listModels: () => modelStore.listModels(),
    downloadModel: (modelId) => modelStore.downloadModel(modelId),
    deleteModel: (modelId) => modelStore.deleteModel(modelId),
    listInputDevices: async () => {
      const devices = await environment.mediaDevices.enumerateDevices()
      return devices.filter((device) => device.kind === 'audioinput').map((device) => device.deviceId)
    },
    onModelProgress: (handler, onError) => modelStore.onProgress(handler, onError),
  }
}

function createSession(
  environment: BrowserSpeechEnvironment,
  modelStore: Pick<BrowserModelStore, 'listModels'>,
  sessionId: string,
  requestedOptions: SpeechRecognitionOptions,
  listener: SpeechRecognitionListener,
  release: () => void,
): SpeechRecognitionSession {
  let worker: Worker | null = null
  let audioContext: AudioContext | null = null
  let source: MediaStreamAudioSourceNode | null = null
  let worklet: AudioWorkletNode | null = null
  let mediaStream: MediaStream | null = null
  let active = true
  let readySettled = false
  let terminalIntent: 'stop' | 'cancel' | null = null
  let terminalPromise: Promise<void> | null = null
  let resolveReady: () => void = () => {}
  let rejectReady: (cause: unknown) => void = () => {}
  let resolveTerminal: () => void = () => {}
  const ready = new Promise<void>((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })
  const terminal = new Promise<void>((resolve) => (resolveTerminal = resolve))
  let resolveFlushed: ((lastSeq: number) => void) | null = null
  const visibilityHandler = () => {
    if (environment.document.visibilityState === 'hidden' && active) {
      fail(new BrowserSpeechError('background_suspended', 'Browser speech stopped because the page became hidden.'))
    }
  }
  environment.document.addEventListener('visibilitychange', visibilityHandler)

  void initialize().catch(fail)

  async function initialize() {
    const models = await modelStore.listModels()
    if (!models[0]?.installed) {
      throw new BrowserSpeechError('model_not_installed', 'The browser speech model is not installed. Open Voice settings to download it.')
    }
    const options = requestedOptions.modelId === BROWSER_SPEECH_MODEL_ID
      ? requestedOptions
      : { ...requestedOptions, modelId: BROWSER_SPEECH_MODEL_ID }
    if (requestedOptions.modelId !== BROWSER_SPEECH_MODEL_ID) {
      listener.onEvent({
        type: 'warning', sessionId, code: 'browser_model_selection_migrated',
        message: `Browser speech selected its local streaming model ${BROWSER_SPEECH_MODEL_ID} instead of ${requestedOptions.modelId}.`,
      })
    }
    if (options.hotwords.length > 0) {
      listener.onEvent({
        type: 'warning', sessionId, code: 'hotwords_unsupported',
        message: 'Browser speech does not currently apply hotwords.',
      })
    }

    worker = environment.createWorker(BROWSER_SPEECH_RUNTIME.worker)
    worker.onerror = () => fail(new BrowserSpeechError('worker_crashed', 'The browser speech Worker crashed.'))
    const workerReady = new Promise<void>((resolve, reject) => {
      if (!worker) return reject(new Error('Worker was not created.'))
      worker.onmessage = ({ data }) => {
        if (!isWorkerEvent(data, sessionId) || !active) return
        switch (data.type) {
          case 'ready': resolve(); break
          case 'partial': listener.onEvent({ type: 'partial', sessionId, text: data.text }); break
          case 'processing': listener.onEvent({ type: 'processing', sessionId, segmentIndex: data.segmentIndex }); break
          case 'stable': listener.onEvent({ type: 'stable', sessionId, segmentIndex: data.segmentIndex, text: data.text }); break
          case 'warning': listener.onEvent({ type: 'warning', sessionId, code: data.code, message: data.message }); break
          case 'disposed': finish(data.reason); break
          case 'fatal': reject(new BrowserSpeechError(data.code, data.message)); fail(new BrowserSpeechError(data.code, data.message)); break
        }
      }
    })
    worker.postMessage({
      v: BROWSER_SPEECH_PROTOCOL_VERSION,
      type: 'init',
      sessionId,
      cacheName: BROWSER_SPEECH_CACHE_NAME,
      markerUrl: BROWSER_SPEECH_MARKER_URL,
      runtime: {
        glue: BROWSER_SPEECH_RUNTIME.glue,
        wrapper: BROWSER_SPEECH_RUNTIME.wrapper,
        wasm: BROWSER_SPEECH_RUNTIME.wasm,
        wasmBytes: BROWSER_SPEECH_RUNTIME.wasmBytes,
        wasmSha256: BROWSER_SPEECH_RUNTIME.wasmSha256,
        version: BROWSER_SPEECH_RUNTIME.version,
        gitSha: BROWSER_SPEECH_RUNTIME.gitSha,
      },
      files: BROWSER_SPEECH_MODEL_FILES,
      options: { vadSilenceMs: options.vadSilenceMs },
    })

    // Load and verify the runtime/model before opening the microphone. This
    // keeps the 8-credit PCM window for recognition, not cold-start warmup.
    await withTimeout(
      workerReady,
      environment,
      60_000,
      'worker_initialization_timeout',
      'The browser speech engine did not initialize within 60 seconds.',
    )

    const mediaPromise = environment.mediaDevices.getUserMedia({
      audio: options.inputDevice
        ? { deviceId: { exact: options.inputDevice }, channelCount: { ideal: 1 }, echoCancellation: false, noiseSuppression: false, autoGainControl: false }
        : { channelCount: { ideal: 1 }, echoCancellation: false, noiseSuppression: false, autoGainControl: false },
      video: false,
    })
    mediaStream = await withPermissionTimeout(mediaPromise, environment, 15_000, () => !active)
    if (!active) {
      stopTracks(mediaStream)
      throw new BrowserSpeechError('session_cancelled', 'Browser speech was cancelled before microphone setup completed.')
    }
    for (const track of mediaStream.getAudioTracks()) {
      track.onended = () => fail(new BrowserSpeechError('input_device_ended', 'The browser microphone input ended.'))
    }
    audioContext = environment.createAudioContext()
    await audioContext.audioWorklet.addModule(BROWSER_SPEECH_RUNTIME.worklet)
    if (audioContext.state === 'suspended') await audioContext.resume()
    source = audioContext.createMediaStreamSource(mediaStream)
    worklet = environment.createAudioWorkletNode(audioContext)
    worklet.onprocessorerror = () => fail(new BrowserSpeechError('audio_worklet_failed', 'The browser audio capture worklet failed.'))
    const silent = audioContext.createGain()
    silent.gain.value = 0
    source.connect(worklet).connect(silent).connect(audioContext.destination)
    worklet.port.onmessage = ({ data }) => {
      if (data?.type === 'level') listener.onEvent({ type: 'level', sessionId, rms: Number(data.rms) || 0 })
      if (data?.type === 'flushed') {
        if (data.droppedFrames > 0) {
          listener.onEvent({ type: 'warning', sessionId, code: 'pcm_backpressure', message: `Browser audio dropped ${data.droppedFrames} frames because recognition could not keep up.` })
        }
        resolveFlushed?.(Number(data.lastSeq) || 0)
        resolveFlushed = null
      }
    }
    const channel = environment.createMessageChannel()
    worker.postMessage({ v: 1, type: 'bindPcm', sessionId, port: channel.port1 }, [channel.port1])
    worklet.port.postMessage({ type: 'bind', sessionId, credits: 8, port: channel.port2 }, [channel.port2])
    if (!active || terminalIntent !== null) throw new BrowserSpeechError('session_cancelled', 'Browser speech stopped before it became ready.')
    readySettled = true
    listener.onEvent({
      type: 'started', sessionId,
      inputDevice: options.inputDevice || mediaStream.getAudioTracks()[0]?.label || 'System default microphone',
      provider: 'sherpa-onnx-web-1.13.7/zipformer-small-ctc',
    })
    resolveReady()
  }

  function terminate(intent: 'stop' | 'cancel'): Promise<void> {
    if (!active) return terminal
    if (terminalIntent !== null) return terminalPromise ?? terminal
    terminalIntent = intent
    terminalPromise = intent === 'stop' ? gracefulStop() : abortiveCancel()
    return terminalPromise
  }

  async function gracefulStop() {
    if (!readySettled || !worklet || !worker) return abortiveCancel()
    try {
      const lastSeq = await withTimeout(new Promise<number>((resolve) => {
        resolveFlushed = resolve
        worklet?.port.postMessage({ type: 'flush' })
      }), environment, 2_000, 'audio_worklet_flush_timeout', 'The audio worklet did not flush within 2 seconds.')
      stopAudio()
      worker.postMessage({ v: 1, type: 'stop', sessionId, lastSeq })
      return await withTimeout(terminal, environment, 10_000, 'shutdown_timeout', 'The speech Worker did not finish the final segment within 10 seconds.')
    } catch (cause) {
      fail(cause)
      return terminal
    }
  }

  async function abortiveCancel() {
    stopAudio()
    worker?.postMessage({ v: 1, type: 'cancel', sessionId })
    finish('cancelled')
    return terminal
  }

  function fail(cause: unknown) {
    if (!active) return
    const error = cause instanceof BrowserSpeechError
      ? cause
      : new BrowserSpeechError('browser_speech_failed', cause instanceof Error ? cause.message : String(cause), { cause })
    listener.onEvent({ type: 'error', sessionId, code: error.code, message: error.message })
    listener.onError(error)
    if (!readySettled) rejectReady(error)
    stopAudio()
    worker?.postMessage({ v: 1, type: 'cancel', sessionId })
    finish('unexpected')
  }

  function finish(reason: SpeechRecognitionStopReason) {
    if (!active) return
    active = false
    environment.document.removeEventListener('visibilitychange', visibilityHandler)
    stopAudio()
    worker?.terminate()
    worker = null
    release()
    if (!readySettled && reason !== 'unexpected') rejectReady(new BrowserSpeechError('session_cancelled', 'Browser speech stopped before it became ready.'))
    listener.onEvent({ type: 'stopped', sessionId, reason })
    resolveTerminal()
  }

  function stopAudio() {
    source?.disconnect()
    worklet?.disconnect()
    stopTracks(mediaStream)
    mediaStream = null
    void audioContext?.close().catch(() => undefined)
    audioContext = null
    source = null
    worklet = null
  }

  return Object.freeze({ id: sessionId, ready, stop: () => terminate('stop'), cancel: () => terminate('cancel') })
}

async function withPermissionTimeout(
  promise: Promise<MediaStream>,
  environment: Pick<BrowserSpeechEnvironment, 'setTimeout' | 'clearTimeout'>,
  milliseconds: number,
  stale: () => boolean,
): Promise<MediaStream> {
  let timeout: ReturnType<typeof globalThis.setTimeout> | undefined
  const lateGuard = promise.then((stream) => {
    if (stale()) stopTracks(stream)
    return stream
  })
  try {
    return await Promise.race([
      lateGuard,
      new Promise<never>((_, reject) => {
        timeout = environment.setTimeout(() => reject(new BrowserSpeechError('microphone_permission_timeout', 'The browser did not finish the microphone permission request within 15 seconds.')), milliseconds)
      }),
    ])
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === 'NotAllowedError') {
      throw new BrowserSpeechError('microphone_permission_denied', 'Microphone permission was denied.', { cause })
    }
    throw cause
  } finally {
    if (timeout !== undefined) environment.clearTimeout(timeout)
  }
}

async function withTimeout<T>(
  promise: Promise<T>,
  environment: Pick<BrowserSpeechEnvironment, 'setTimeout' | 'clearTimeout'>,
  milliseconds: number,
  code: string,
  message: string,
): Promise<T> {
  let timeout: ReturnType<typeof globalThis.setTimeout> | undefined
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timeout = environment.setTimeout(() => reject(new BrowserSpeechError(code, message)), milliseconds)
      }),
    ])
  } finally {
    if (timeout !== undefined) environment.clearTimeout(timeout)
  }
}

function stopTracks(stream: MediaStream | null) {
  for (const track of stream?.getTracks() ?? []) track.stop()
}

function failedSession(id: string, cause: unknown): SpeechRecognitionSession {
  return Object.freeze({ id, ready: Promise.reject(cause), stop: async () => undefined, cancel: async () => undefined })
}

function browserSpeechEnvironment(): BrowserSpeechEnvironment {
  return {
    mediaDevices: navigator.mediaDevices,
    document,
    createWorker: (url) => new Worker(url, { name: 'rambledesk-browser-speech' }),
    createAudioContext: () => new AudioContext({ latencyHint: 'interactive' }),
    createAudioWorkletNode: (context) => new AudioWorkletNode(context, 'rambledesk-pcm-capture', {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      outputChannelCount: [1],
    }),
    createMessageChannel: () => new MessageChannel(),
    createSessionId: () => crypto.randomUUID(),
    setTimeout: globalThis.setTimeout.bind(globalThis),
    clearTimeout: globalThis.clearTimeout.bind(globalThis),
  }
}

function unsupported(code: string, message: string): BrowserSpeechSupport {
  return Object.freeze({ supported: false, code, message })
}
