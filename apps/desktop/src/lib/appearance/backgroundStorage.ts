/** Local-only background storage. Images never enter shared settings or server requests. */
export type SavedBackground = { blob: Blob; name: string; revision: string }
export const MAX_BACKGROUND_BYTES = 16 * 1024 * 1024
export const MAX_BACKGROUND_PIXELS = 40_000_000

export function backgroundImageType(bytes: Uint8Array): string | null {
  const text = (start: number, length: number) => String.fromCharCode(...bytes.slice(start, start + length))
  if (bytes.length >= 8 && [137, 80, 78, 71, 13, 10, 26, 10].every((value, i) => bytes[i] === value)) return 'image/png'
  if (bytes[0] === 255 && bytes[1] === 216 && bytes[2] === 255) return 'image/jpeg'
  if (text(0, 6) === 'GIF87a' || text(0, 6) === 'GIF89a') return 'image/gif'
  if (text(0, 4) === 'RIFF' && text(8, 4) === 'WEBP') return 'image/webp'
  return null
}

export async function validateBackground(file: File): Promise<Blob> {
  if (!file.size || file.size > MAX_BACKGROUND_BYTES) throw new Error('background_size')
  const type = backgroundImageType(new Uint8Array(await file.slice(0, 16).arrayBuffer()))
  if (!type) throw new Error('background_format')
  const blob = new Blob([file], { type })
  const url = URL.createObjectURL(blob)
  try {
    const image = new Image()
    image.src = url
    try { await image.decode() } catch { throw new Error('background_decode') }
    if (!image.naturalWidth || !image.naturalHeight || image.naturalWidth * image.naturalHeight > MAX_BACKGROUND_PIXELS) {
      throw new Error('background_dimensions')
    }
  } finally {
    URL.revokeObjectURL(url)
  }
  return blob
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('rambledesk-appearance', 1)
    let failed = false
    const fail = () => {
      failed = true
      reject(new Error('background_storage'))
    }
    request.onupgradeneeded = () => request.result.createObjectStore('backgrounds')
    request.onerror = fail
    request.onblocked = fail
    request.onsuccess = () => {
      if (failed) request.result.close()
      else {
        request.result.onversionchange = () => request.result.close()
        resolve(request.result)
      }
    }
  })
}

async function transaction<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await openDatabase()
  try {
    return await new Promise<T>((resolve, reject) => {
      const transaction = db.transaction('backgrounds', mode)
      const request = run(transaction.objectStore('backgrounds'))
      transaction.oncomplete = () => resolve(request.result)
      transaction.onerror = transaction.onabort = () => reject(transaction.error ?? new Error('background_storage'))
    })
  } finally { db.close() }
}

export async function readBackground(): Promise<SavedBackground | null> {
  return await transaction<SavedBackground | undefined>('readonly', store => store.get('current')) ?? null
}

export async function writeBackground(value: SavedBackground | null): Promise<void> {
  if (value) await transaction('readwrite', store => store.put(value, 'current'))
  else await transaction('readwrite', store => store.delete('current'))
}
