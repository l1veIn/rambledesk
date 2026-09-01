/* global SherpaOnnx, createOnlineRecognizer */
'use strict'

const V = 1
let sessionId = ''
let pcmPort = null
let Module = null
let recognizer = null
let stream = null
let resampler = null
let queue = Promise.resolve()
let processedSeq = 0
let stopAfterSeq = null
let segmentIndex = 0
let lastText = ''
let disposed = false
let initStage = 'not_started'
const engineDiagnostics = []

self.onmessage = ({ data }) => {
  if (!data || data.v !== V || typeof data.sessionId !== 'string') return
  if (data.type === 'init' && !sessionId) {
    sessionId = data.sessionId
    queue = initialize(data).catch((cause) => fatal('model_load_failed', cause))
    return
  }
  if (data.sessionId !== sessionId || disposed) return
  if (data.type === 'bindPcm' && data.port) {
    pcmPort = data.port
    pcmPort.onmessage = ({ data: pcm }) => {
      if (!pcm || pcm.v !== V || pcm.type !== 'pcm' || pcm.sessionId !== sessionId) return
      queue = queue.then(() => processPcm(pcm)).catch((cause) => fatal('worker_processing_failed', cause))
    }
    pcmPort.start()
  } else if (data.type === 'stop') {
    stopAfterSeq = data.lastSeq
    queue = queue.then(maybeStop).catch((cause) => fatal('shutdown_failed', cause))
  } else if (data.type === 'cancel') {
    disposed = true
    queue = queue.then(() => dispose('cancelled')).catch(() => post({ type: 'disposed', reason: 'cancelled' }))
  }
}

async function initialize(command) {
  initStage = 'runtime_fetch'
  const wasmResponse = await fetch(command.runtime.wasm, { credentials: 'same-origin', cache: 'force-cache' })
  if (!wasmResponse.ok) throw coded('runtime_asset_unavailable', `Wasm request failed with HTTP ${wasmResponse.status}.`)
  const wasmBinary = new Uint8Array(await wasmResponse.arrayBuffer())
  if (wasmBinary.byteLength !== command.runtime.wasmBytes) throw coded('runtime_asset_hash_mismatch', 'Wasm size mismatch.')
  const wasmDigest = await subtleSha256(wasmBinary)
  if (wasmDigest !== command.runtime.wasmSha256) throw coded('runtime_asset_hash_mismatch', `Wasm SHA-256 mismatch: ${wasmDigest}.`)
  if (!WebAssembly.validate(wasmBinary)) throw coded('wasm_simd_unsupported', 'This browser cannot validate the sherpa-onnx SIMD Wasm runtime.')

  initStage = 'runtime_import'
  importScripts(command.runtime.glue, command.runtime.wrapper)
  if (typeof SherpaOnnx !== 'function' || typeof createOnlineRecognizer !== 'function') {
    throw coded('runtime_asset_unavailable', 'The sherpa-onnx Web wrapper did not expose its runtime contract.')
  }
  initStage = 'runtime_instantiate'
  Module = await SherpaOnnx({
    wasmBinary,
    print: captureEngineDiagnostic,
    printErr: captureEngineDiagnostic,
  })
  const runtimeVersion = runtimeString('_SherpaOnnxGetVersionStr')
  const runtimeGitSha = runtimeString('_SherpaOnnxGetGitSha1')
  const ortVersion = runtimeString('_SherpaOnnxGetOnnxruntimeVersionStr')
  if (runtimeVersion !== command.runtime.version || !runtimeGitMatches(command.runtime.gitSha, runtimeGitSha)) {
    throw coded('runtime_version_mismatch', `Expected sherpa-onnx ${command.runtime.version} (${command.runtime.gitSha}), received ${runtimeVersion} (${runtimeGitSha}).`)
  }

  initStage = 'model_fs_prepare'
  try { Module.FS.mkdir('/models') } catch (cause) {
    if (!String(cause).includes('File exists')) throw cause
  }
  const cache = await caches.open(command.cacheName)
  for (const file of command.files) {
    initStage = `model_copy:${file.name}`
    await copyCachedModelFile(cache, file, command.markerUrl)
  }
  initStage = 'recognizer_create'
  recognizer = createOnlineRecognizer(Module, recognizerConfig(command.options.vadSilenceMs))
  if (!recognizer || !recognizer.handle) throw coded('model_load_failed', 'sherpa-onnx rejected the browser streaming recognizer configuration.')
  initStage = 'stream_create'
  stream = recognizer.createStream()
  initStage = 'ready'
  post({ type: 'ready', runtimeVersion, runtimeGitSha, ortVersion })
}

function runtimeGitMatches(expected, actual) {
  return typeof actual === 'string' && actual.length >= 8 && expected.startsWith(actual)
}

async function copyCachedModelFile(cache, file, markerUrl) {
  const response = await cache.match(file.url)
  if (!response) throw coded('model_not_installed', `Cached model file is missing: ${file.name}.`)
  const path = `/models/${file.name}`
  const bytes = new Uint8Array(await response.arrayBuffer())
  const digest = await subtleSha256(bytes)
  if (bytes.byteLength !== file.bytes || digest !== file.sha256) {
    await Promise.all([
      cache.delete(file.url),
      typeof markerUrl === 'string' ? cache.delete(markerUrl) : Promise.resolve(false),
    ])
    throw coded(
      bytes.byteLength !== file.bytes ? 'model_size_mismatch' : 'model_sha256_mismatch',
      `Cached ${file.name} failed integrity verification (${bytes.byteLength} bytes, SHA-256 ${digest}).`,
    )
  }
  Module.FS.writeFile(path, bytes)
}

function recognizerConfig(vadSilenceMs) {
  return {
    featConfig: { sampleRate: 16000, featureDim: 80 },
    modelConfig: {
      zipformer2Ctc: {
        model: '/models/ctc-chunk-32-left-128.int8.onnx',
      },
      tokens: '/models/tokens.txt',
      numThreads: 1,
      provider: 'cpu',
      debug: 0,
      modelType: '',
      modelingUnit: '',
      bpeVocab: '',
    },
    decodingMethod: 'greedy_search',
    maxActivePaths: 4,
    enableEndpoint: 1,
    rule1MinTrailingSilence: 2.4,
    rule2MinTrailingSilence: Math.max(0.2, Math.min(3, vadSilenceMs / 1000)),
    rule3MinUtteranceLength: 10,
  }
}

async function processPcm(message) {
  try {
    if (disposed) return
    if (!resampler) resampler = new StreamingResampler(message.sampleRate, 16000)
    if (message.sampleRate !== resampler.inputRate) throw coded('input_sample_rate_changed', 'The microphone sample rate changed during recognition.')
    const samples = resampler.push(new Float32Array(message.samples))
    if (samples.length > 0) acceptAndDecode(samples)
    processedSeq = Math.max(processedSeq, message.seq)
    await maybeStop()
  } finally {
    pcmPort?.postMessage({ v: V, type: 'ack', sessionId, seq: message.seq, credit: 1 })
  }
}

function acceptAndDecode(samples) {
  stream.acceptWaveform(16000, samples)
  let decoded = false
  while (recognizer.isReady(stream)) {
    recognizer.decode(stream)
    decoded = true
  }
  if (decoded) updateResult()
  if (recognizer.isEndpoint(stream)) finishSegment()
}

function updateResult() {
  const text = String(recognizer.getResult(stream)?.text || '').trim()
  if (text && text !== lastText) {
    lastText = text
    post({ type: 'partial', text })
  }
}

function finishSegment() {
  updateResult()
  if (lastText) {
    post({ type: 'processing', segmentIndex })
    post({ type: 'stable', segmentIndex, text: lastText })
    segmentIndex += 1
  }
  lastText = ''
  recognizer.reset(stream)
}

async function maybeStop() {
  if (stopAfterSeq === null || processedSeq < stopAfterSeq || disposed) return
  stopAfterSeq = null
  if (resampler) {
    const tail = resampler.flush()
    if (tail.length) acceptAndDecode(tail)
  }
  // Supply bounded silence so endpoint rules can finish the user's final utterance.
  acceptAndDecode(new Float32Array(16_000))
  stream.inputFinished()
  let iterations = 0
  while (recognizer.isReady(stream) && iterations < 1000) {
    recognizer.decode(stream)
    iterations += 1
  }
  updateResult()
  if (lastText) finishSegment()
  disposed = true
  await dispose('stopped')
}

async function dispose(reason) {
  pcmPort?.close()
  pcmPort = null
  try { stream?.free() } catch {}
  try { recognizer?.free() } catch {}
  stream = null
  recognizer = null
  post({ type: 'disposed', reason })
}

function fatal(fallbackCode, cause) {
  if (disposed) return
  disposed = true
  const code = cause?.code || fallbackCode
  const engineMessage = engineDiagnostics.at(-1)
  const message = cause instanceof Error
    ? cause.message
    : engineMessage || `sherpa-onnx native exception during ${initStage} (${String(cause)}).`
  post({ type: 'fatal', code, message })
  void dispose('cancelled')
}

function captureEngineDiagnostic(value) {
  const line = String(value).trim()
  if (!line) return
  engineDiagnostics.push(line)
  if (engineDiagnostics.length > 12) engineDiagnostics.shift()
}

function runtimeString(exportName) {
  const fn = Module[exportName]
  if (typeof fn !== 'function') return ''
  return Module.UTF8ToString(fn())
}

function post(message) {
  self.postMessage({ v: V, sessionId, ...message })
}

function coded(code, message) {
  const error = new Error(message)
  error.code = code
  return error
}

async function subtleSha256(bytes) {
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, '0')).join('')
}

class StreamingResampler {
  constructor(inputRate, outputRate) {
    this.inputRate = inputRate
    this.outputRate = outputRate
    this.buffer = new Float32Array(0)
    this.bufferStart = 0
    this.totalInput = 0
    this.nextOutput = 0
  }
  push(input) {
    const data = new Float32Array(this.buffer.length + input.length)
    data.set(this.buffer)
    data.set(input, this.buffer.length)
    this.totalInput += input.length
    const output = []
    let position = this.sourcePosition()
    while (position + 1 < this.totalInput) {
      const absoluteIndex = Math.floor(position)
      const index = absoluteIndex - this.bufferStart
      const fraction = position - absoluteIndex
      output.push(data[index] + (data[index + 1] - data[index]) * fraction)
      this.nextOutput += 1
      position = this.sourcePosition()
    }
    const nextRequired = Math.min(Math.floor(position), this.totalInput - 1)
    const consumed = Math.max(0, nextRequired - this.bufferStart)
    this.buffer = data.slice(consumed)
    this.bufferStart += consumed
    return Float32Array.from(output)
  }
  flush() {
    const output = []
    let position = this.sourcePosition()
    while (position < this.totalInput) {
      const index = Math.floor(position) - this.bufferStart
      const next = Math.min(index + 1, this.buffer.length - 1)
      const fraction = position - Math.floor(position)
      output.push(this.buffer[index] + (this.buffer[next] - this.buffer[index]) * fraction)
      this.nextOutput += 1
      position = this.sourcePosition()
    }
    this.buffer = new Float32Array(0)
    this.bufferStart = 0
    this.totalInput = 0
    this.nextOutput = 0
    return Float32Array.from(output)
  }
  sourcePosition() { return this.nextOutput * this.inputRate / this.outputRate }
}
