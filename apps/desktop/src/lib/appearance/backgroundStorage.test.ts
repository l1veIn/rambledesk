import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  backgroundImageType, MAX_BACKGROUND_BYTES, MAX_BACKGROUND_PIXELS,
  readBackground, validateBackground, writeBackground,
} from './backgroundStorage'

const png = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 0])
const file = (bytes: BlobPart = png, type = 'image/png') => new File([bytes], 'background.png', { type })
const settle = async () => { for (let index = 0; index < 5; index++) await Promise.resolve() }
let width = 800
let height = 600
const decode = vi.fn<() => Promise<void>>()

beforeEach(() => {
  width = 800
  height = 600
  decode.mockReset().mockResolvedValue(undefined)
  vi.stubGlobal('Image', class {
    src = ''
    naturalWidth = width
    naturalHeight = height
    decode = decode
  })
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:validation')
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
})
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals() })

describe('local background image validation', () => {
  it('recognizes supported raster bytes instead of trusting extensions or MIME declarations', async () => {
    expect(backgroundImageType(png)).toBe('image/png')
    expect(backgroundImageType(new Uint8Array([255, 216, 255, 224]))).toBe('image/jpeg')
    expect(backgroundImageType(new TextEncoder().encode('GIF87a'))).toBe('image/gif')
    expect(backgroundImageType(new TextEncoder().encode('GIF89a'))).toBe('image/gif')
    expect(backgroundImageType(new TextEncoder().encode('RIFF0000WEBP'))).toBe('image/webp')
    const result = await validateBackground(file(png, 'image/svg+xml'))
    expect(result.type).toBe('image/png')
    expect(await result.arrayBuffer()).toEqual(await file().arrayBuffer())
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:validation')
  })

  it('rejects SVG, HTML, unsupported signatures and truncated headers before creating a URL', async () => {
    for (const bytes of [new Uint8Array([255, 216]), new TextEncoder().encode('<svg xmlns="http://www.w3.org/2000/svg"/>'),
      new TextEncoder().encode('<html>image</html>'), new TextEncoder().encode('not an image')]) {
      expect(backgroundImageType(bytes)).toBeNull()
      await expect(validateBackground(file(bytes))).rejects.toThrow('background_format')
    }
    expect(URL.createObjectURL).not.toHaveBeenCalled()
    expect(decode).not.toHaveBeenCalled()
  })

  it('enforces the input byte budget before decoding', async () => {
    await expect(validateBackground(file(''))).rejects.toThrow('background_size')
    await expect(validateBackground(file(new Uint8Array(MAX_BACKGROUND_BYTES + 1)))).rejects.toThrow('background_size')
    expect(URL.createObjectURL).not.toHaveBeenCalled()
    expect(decode).not.toHaveBeenCalled()
  })

  it('enforces decoded dimensions and releases validation URLs on both rejection paths', async () => {
    width = MAX_BACKGROUND_PIXELS + 1
    height = 1
    await expect(validateBackground(file())).rejects.toThrow('background_dimensions')
    width = 0
    await expect(validateBackground(file())).rejects.toThrow('background_dimensions')
    width = 800
    decode.mockRejectedValueOnce(new Error('corrupt stream'))
    await expect(validateBackground(file())).rejects.toThrow('background_decode')
    expect(URL.revokeObjectURL).toHaveBeenCalledTimes(3)
  })
})

function databaseHarness() {
  const operation = { result: undefined as unknown }
  const store = { get: vi.fn(() => operation), put: vi.fn(() => operation), delete: vi.fn(() => operation) }
  const transaction = {
    objectStore: vi.fn(() => store), error: null as Error | null,
    oncomplete: null as (() => void) | null, onerror: null as (() => void) | null, onabort: null as (() => void) | null,
  }
  const db = { close: vi.fn(), createObjectStore: vi.fn(), transaction: vi.fn(() => transaction) }
  const request = {
    result: db, error: null as Error | null,
    onsuccess: null as (() => void) | null, onerror: null as (() => void) | null,
    onblocked: null as (() => void) | null, onupgradeneeded: null as (() => void) | null,
  }
  const open = vi.fn(() => request)
  vi.stubGlobal('indexedDB', { open })
  return { operation, store, transaction, db, request, open }
}

describe('background storage transaction boundaries', () => {
  it('acknowledges an image replacement only after the write transaction commits', async () => {
    const harness = databaseHarness()
    const value = { blob: new Blob(['image']), name: 'image.png', revision: 'revision' }
    const completed = vi.fn()
    const pending = writeBackground(value).then(completed)
    harness.request.onupgradeneeded?.()
    expect(harness.db.createObjectStore).toHaveBeenCalledWith('backgrounds')
    harness.request.onsuccess?.()
    await settle()
    expect(harness.db.transaction).toHaveBeenCalledWith('backgrounds', 'readwrite')
    expect(harness.store.put).toHaveBeenCalledWith(value, 'current')
    expect(completed).not.toHaveBeenCalled()
    expect(harness.db.close).not.toHaveBeenCalled()
    harness.transaction.oncomplete?.()
    await pending
    expect(completed).toHaveBeenCalledOnce()
    expect(harness.db.close).toHaveBeenCalledOnce()
  })

  it('reads the committed current image and maps missing values to null', async () => {
    for (const value of [undefined, { blob: new Blob(['image']), name: 'saved.png', revision: 'saved' }]) {
      const harness = databaseHarness()
      harness.operation.result = value
      const pending = readBackground()
      harness.request.onsuccess?.()
      await settle()
      expect(harness.store.get).toHaveBeenCalledWith('current')
      harness.transaction.oncomplete?.()
      await expect(pending).resolves.toBe(value ?? null)
      expect(harness.db.close).toHaveBeenCalledOnce()
    }
  })

  it('propagates aborted writes and closes the database without claiming success', async () => {
    const harness = databaseHarness()
    const pending = writeBackground(null)
    const rejected = expect(pending).rejects.toThrow('quota')
    harness.request.onsuccess?.()
    await settle()
    expect(harness.store.delete).toHaveBeenCalledWith('current')
    harness.transaction.error = new Error('quota')
    harness.transaction.onabort?.()
    await rejected
    expect(harness.db.close).toHaveBeenCalledOnce()
  })

  it('closes a connection if transaction creation itself fails', async () => {
    const harness = databaseHarness()
    harness.db.transaction.mockImplementation(() => { throw new Error('database closed') })
    const pending = readBackground()
    const rejected = expect(pending).rejects.toThrow('database closed')
    harness.request.onsuccess?.()
    await rejected
    expect(harness.db.close).toHaveBeenCalledOnce()
  })

  it('releases a connection that opens after a blocked-open error was already reported', async () => {
    const harness = databaseHarness()
    const pending = readBackground()
    const rejected = expect(pending).rejects.toThrow('background_storage')
    harness.request.onblocked?.()
    await rejected
    harness.request.onsuccess?.()
    await settle()
    expect(harness.db.transaction).not.toHaveBeenCalled()
    expect(harness.db.close).toHaveBeenCalledOnce()
  })
})
