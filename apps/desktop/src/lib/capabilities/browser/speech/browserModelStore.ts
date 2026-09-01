import type {
  CapabilityErrorHandler,
  CapabilityUnsubscribe,
  SpeechModelInfo,
  SpeechModelProgress,
} from '../../workbenchCapabilities'
import {
  BROWSER_SPEECH_CACHE_NAME,
  BROWSER_SPEECH_MODEL_DIGEST,
  BROWSER_SPEECH_MODEL_FILES,
  BROWSER_SPEECH_MODEL_ID,
  BROWSER_SPEECH_MARKER_URL,
  BROWSER_SPEECH_TOTAL_BYTES,
  browserSpeechModelInfo,
} from './browserSpeechManifest'
import { Sha256 } from './sha256'

export class BrowserSpeechError extends Error {
  constructor(readonly code: string, message: string, options?: ErrorOptions) {
    super(message, options)
    this.name = 'BrowserSpeechError'
  }
}

type CacheEnvironment = Readonly<{
  caches: CacheStorage
  fetch: typeof fetch
  storage?: Pick<StorageManager, 'estimate' | 'persist'>
}>

export class BrowserModelStore {
  readonly #environment: CacheEnvironment
  readonly #progressListeners = new Set<(progress: SpeechModelProgress) => void>()
  #download: AbortController | null = null

  constructor(environment: CacheEnvironment = browserCacheEnvironment()) {
    this.#environment = environment
  }

  async listModels(): Promise<readonly SpeechModelInfo[]> {
    const cache = await this.#environment.caches.open(BROWSER_SPEECH_CACHE_NAME)
    const missing = await missingModelFiles(cache)
    const marker = await cache.match(BROWSER_SPEECH_MARKER_URL)
    const installed = marker !== undefined && missing.length === 0 && await validMarker(marker)
    return [browserSpeechModelInfo(installed, installed ? [] : missing)]
  }

  async downloadModel(modelId: string): Promise<SpeechModelInfo> {
    requireSupportedModel(modelId)
    if (this.#download !== null) {
      throw new BrowserSpeechError('model_download_active', 'The browser speech model is already downloading.')
    }
    await this.#assertQuota()
    const abort = new AbortController()
    this.#download = abort
    let downloaded = 0
    const cache = await this.#environment.caches.open(BROWSER_SPEECH_CACHE_NAME)
    await cache.delete(BROWSER_SPEECH_MARKER_URL)
    this.#emitProgress(downloaded)
    try {
      for (const file of BROWSER_SPEECH_MODEL_FILES) {
        const existing = await cache.match(file.url)
        if (existing && responseMatchesManifest(existing, file.bytes, file.sha256)) {
          downloaded += file.bytes
          this.#emitProgress(downloaded)
          continue
        }
        if (existing) await cache.delete(file.url)
        const response = await this.#environment.fetch(file.url, {
          cache: 'no-store',
          credentials: 'omit',
          mode: 'cors',
          signal: abort.signal,
        })
        if (!response.ok || response.body === null) {
          throw new BrowserSpeechError(
            'model_download_blocked',
            `Could not download ${file.name} from the pinned ModelScope mirror (HTTP ${response.status}).`,
          )
        }
        const contentLengthHeader = response.headers.get('content-length')
        const contentLength = contentLengthHeader === null ? null : Number(contentLengthHeader)
        if (contentLength !== null && Number.isFinite(contentLength) && contentLength !== file.bytes) {
          throw new BrowserSpeechError(
            'model_size_mismatch',
            `${file.name} size mismatch: expected ${file.bytes}, received ${contentLength}.`,
          )
        }
        const [verifyBody, cacheBody] = response.body.tee()
        const headers = new Headers(response.headers)
        headers.set('x-rambledesk-bytes', String(file.bytes))
        headers.set('x-rambledesk-sha256', file.sha256)
        const cacheWrite = cache.put(
          file.url,
          new Response(cacheBody, { status: 200, headers }),
        )
        const reader = verifyBody.getReader()
        const hash = new Sha256()
        let fileBytes = 0
        try {
          while (true) {
            const { value, done } = await reader.read()
            if (done) break
            if (abort.signal.aborted) throw abort.signal.reason
            hash.update(value)
            fileBytes += value.byteLength
            this.#emitProgress(downloaded + Math.min(fileBytes, file.bytes))
          }
          await cacheWrite
        } catch (cause) {
          // A tee branch may still be writing after verification fails. Settle
          // it before delete so no write-after-delete can create a partial file.
          await cacheWrite.catch(() => undefined)
          await cache.delete(file.url)
          throw cause
        }
        if (fileBytes !== file.bytes) {
          await cache.delete(file.url)
          throw new BrowserSpeechError(
            'model_size_mismatch',
            `${file.name} size mismatch: expected ${file.bytes}, received ${fileBytes}.`,
          )
        }
        const actual = hash.digestHex()
        if (actual !== file.sha256) {
          await cache.delete(file.url)
          throw new BrowserSpeechError(
            'model_sha256_mismatch',
            `${file.name} SHA-256 mismatch: expected ${file.sha256}, received ${actual}.`,
          )
        }
        downloaded += file.bytes
        this.#emitProgress(downloaded)
      }
      await cache.put(BROWSER_SPEECH_MARKER_URL, new Response(JSON.stringify({
        schemaVersion: 1,
        modelId: BROWSER_SPEECH_MODEL_ID,
        manifestSha256: BROWSER_SPEECH_MODEL_DIGEST,
      }), { headers: { 'content-type': 'application/json' } }))
      this.#emitProgress(BROWSER_SPEECH_TOTAL_BYTES)
      return (await this.listModels())[0]
    } catch (cause) {
      if (abort.signal.aborted) {
        throw new BrowserSpeechError('model_download_cancelled', 'Browser speech model download was cancelled.', { cause })
      }
      if (cause instanceof BrowserSpeechError) throw cause
      throw new BrowserSpeechError('model_download_blocked', `Browser speech model download failed: ${messageFrom(cause)}`, { cause })
    } finally {
      if (this.#download === abort) this.#download = null
    }
  }

  cancelDownload(): void {
    this.#download?.abort(new Error('cancelled'))
  }

  async deleteModel(modelId: string): Promise<SpeechModelInfo> {
    requireSupportedModel(modelId)
    this.cancelDownload()
    await this.#environment.caches.delete(BROWSER_SPEECH_CACHE_NAME)
    return browserSpeechModelInfo(false, BROWSER_SPEECH_MODEL_FILES.map((file) => file.name))
  }

  onProgress(
    handler: (progress: SpeechModelProgress) => void,
    _onError: CapabilityErrorHandler,
  ): CapabilityUnsubscribe {
    this.#progressListeners.add(handler)
    return () => this.#progressListeners.delete(handler)
  }

  async #assertQuota() {
    if (!this.#environment.storage) return
    const estimate = await this.#environment.storage.estimate()
    if (estimate.quota === undefined || estimate.usage === undefined) return
    const remaining = estimate.quota - estimate.usage
    if (remaining < BROWSER_SPEECH_TOTAL_BYTES * 1.1) {
      throw new BrowserSpeechError(
        'storage_quota_insufficient',
        `Browser storage is too small for the speech model (${Math.floor(remaining / 1024 / 1024)} MB free).`,
      )
    }
    void this.#environment.storage.persist().catch(() => false)
  }

  #emitProgress(downloaded: number) {
    const progress = Object.freeze({
      model_id: BROWSER_SPEECH_MODEL_ID,
      downloaded,
      total: BROWSER_SPEECH_TOTAL_BYTES,
    })
    for (const listener of this.#progressListeners) listener(progress)
  }
}

function browserCacheEnvironment(): CacheEnvironment {
  if (!('caches' in globalThis)) {
    throw new BrowserSpeechError('cache_storage_unavailable', 'Cache Storage is unavailable in this browser.')
  }
  return {
    caches: globalThis.caches,
    fetch: globalThis.fetch.bind(globalThis),
    storage: navigator.storage,
  }
}

async function missingModelFiles(cache: Cache): Promise<string[]> {
  const missing: string[] = []
  for (const file of BROWSER_SPEECH_MODEL_FILES) {
    const response = await cache.match(file.url)
    if (!response || !responseMatchesManifest(response, file.bytes, file.sha256)) missing.push(file.name)
  }
  return missing
}

function responseMatchesManifest(response: Response, bytes: number, sha256: string): boolean {
  return response.headers.get('x-rambledesk-bytes') === String(bytes) &&
    response.headers.get('x-rambledesk-sha256') === sha256
}

async function validMarker(response: Response): Promise<boolean> {
  try {
    const marker = await response.json() as Record<string, unknown>
    return marker.modelId === BROWSER_SPEECH_MODEL_ID &&
      marker.manifestSha256 === BROWSER_SPEECH_MODEL_DIGEST
  } catch {
    return false
  }
}

function requireSupportedModel(modelId: string) {
  if (modelId !== BROWSER_SPEECH_MODEL_ID) {
    throw new BrowserSpeechError('model_unsupported', `Browser speech supports only ${BROWSER_SPEECH_MODEL_ID}; received ${modelId}.`)
  }
}

function messageFrom(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause)
}
