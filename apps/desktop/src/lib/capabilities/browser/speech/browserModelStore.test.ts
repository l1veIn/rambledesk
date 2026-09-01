import { describe, expect, it, vi } from 'vitest'
import { BrowserModelStore } from './browserModelStore'
import {
  BROWSER_SPEECH_CACHE_NAME,
  BROWSER_SPEECH_MODEL_DIGEST,
  BROWSER_SPEECH_MODEL_FILES,
  BROWSER_SPEECH_MODEL_ID,
  BROWSER_SPEECH_MARKER_URL,
} from './browserSpeechManifest'

class MemoryCache {
  readonly entries = new Map<string, Response>()
  putCount = 0
  async match(input: RequestInfo | URL) {
    return this.entries.get(String(input))?.clone()
  }
  async put(input: RequestInfo | URL, response: Response) {
    this.putCount += 1
    this.entries.set(String(input), response.clone())
  }
  async delete(input: RequestInfo | URL) {
    return this.entries.delete(String(input))
  }
}

class MemoryCaches {
  readonly cache = new MemoryCache()
  async open(_name: string) { return this.cache as unknown as Cache }
  async delete(name: string) {
    if (name !== BROWSER_SPEECH_CACHE_NAME) return false
    this.cache.entries.clear()
    return true
  }
}

function storedFile(bytes: number, sha256: string) {
  return new Response(new Uint8Array(), { headers: {
    'x-rambledesk-bytes': String(bytes),
    'x-rambledesk-sha256': sha256,
  } })
}

function store(caches = new MemoryCaches(), fetch = vi.fn<typeof globalThis.fetch>()) {
  return {
    caches,
    fetch,
    value: new BrowserModelStore({
      caches: caches as unknown as CacheStorage,
      fetch,
      storage: {
        estimate: async () => ({ quota: 2_000_000_000, usage: 0 }),
        persist: async () => true,
      },
    }),
  }
}

describe('BrowserModelStore', () => {
  it('advertises installed only after the versioned marker and every verified entry exist', async () => {
    const base = store()
    for (const file of BROWSER_SPEECH_MODEL_FILES) {
      base.caches.cache.entries.set(file.url, storedFile(file.bytes, file.sha256))
    }
    expect((await base.value.listModels())[0].installed).toBe(false)

    base.caches.cache.entries.set(BROWSER_SPEECH_MARKER_URL, new Response(JSON.stringify({
      modelId: BROWSER_SPEECH_MODEL_ID,
      manifestSha256: BROWSER_SPEECH_MODEL_DIGEST,
    })))
    expect((await base.value.listModels())[0].installed).toBe(true)

    const missing = BROWSER_SPEECH_MODEL_FILES.at(-1)!
    base.caches.cache.entries.delete(missing.url)
    const degraded = (await base.value.listModels())[0]
    expect(degraded.installed).toBe(false)
    expect(degraded.missing_files).toContain(missing.name)
  })

  it('recovers a verified partial cache without fetching it again and never writes a marker on failure', async () => {
    const base = store(new MemoryCaches(), vi.fn(async () => new Response(new Uint8Array([1, 2, 3]), {
      headers: { 'content-length': '3' },
    })))
    const first = BROWSER_SPEECH_MODEL_FILES[0]
    base.caches.cache.entries.set(first.url, storedFile(first.bytes, first.sha256))

    await expect(base.value.downloadModel(BROWSER_SPEECH_MODEL_ID)).rejects.toMatchObject({
      code: 'model_size_mismatch',
    })

    expect(base.fetch).toHaveBeenCalledOnce()
    expect(base.fetch).toHaveBeenCalledWith(BROWSER_SPEECH_MODEL_FILES[1].url, expect.any(Object))
    expect(base.caches.cache.entries.has(first.url)).toBe(true)
    expect(base.caches.cache.entries.has(BROWSER_SPEECH_MARKER_URL)).toBe(false)
    expect((await base.value.listModels())[0].installed).toBe(false)
  })

  it('streams a chunked response when Content-Length is not exposed', async () => {
    const base = store(new MemoryCaches(), vi.fn(async () => new Response(new Uint8Array([1, 2, 3]))))
    await expect(base.value.downloadModel(BROWSER_SPEECH_MODEL_ID)).rejects.toMatchObject({
      code: 'model_size_mismatch',
    })
    expect(base.caches.cache.putCount).toBe(1)
    expect(base.caches.cache.entries.has(BROWSER_SPEECH_MARKER_URL)).toBe(false)
  })

  it('deletes only the versioned browser model cache', async () => {
    const base = store()
    base.caches.cache.entries.set(BROWSER_SPEECH_MARKER_URL, new Response('{}'))
    const result = await base.value.deleteModel(BROWSER_SPEECH_MODEL_ID)
    expect(result.installed).toBe(false)
    expect(base.caches.cache.entries.size).toBe(0)
  })
})
