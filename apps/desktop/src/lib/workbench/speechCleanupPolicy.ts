export const CLEANUP_STABLE_THRESHOLD = 3
export const CLEANUP_CHAR_THRESHOLD = 500
export const CLEANUP_SILENCE_MS = 3_000
export const CLEANUP_TIMEOUT_MS = 30_000

export type CleanupTrigger = 'stable-count' | 'char-count' | 'silence' | 'non-speech' | 'settle'

export function pendingCharCount(pieces: readonly string[]): number {
  return pieces.reduce((sum, piece) => sum + piece.trim().length, 0)
}

export type PendingSpeechSnapshot = {
  count: number
  chars: number
  texts: string[]
}

export function shouldStartCleanup(input: {
  enabled: boolean
  busy: boolean
  pendingCount: number
  pendingChars: number
  trigger: CleanupTrigger
}): boolean {
  if (!input.enabled || input.busy) return false
  if (input.pendingCount === 0) return false
  if (input.trigger === 'non-speech' || input.trigger === 'settle') {
    return true
  }
  if (input.trigger === 'silence') {
    return false
  }
  if (input.trigger === 'stable-count') {
    return input.pendingCount >= CLEANUP_STABLE_THRESHOLD
  }
  return input.pendingChars >= CLEANUP_CHAR_THRESHOLD
}

/** Map a batch cleanup result back onto the original speech nodes, or skip it. */
export function alignCleanupParts(originals: readonly string[], cleaned: string | null): string[] | null {
  if (!cleaned) return null
  const parts = cleaned
    .split(/\n{2,}/)
    .map((part) => part.trim())
    .filter(Boolean)
  if (parts.length !== originals.length) return null
  return parts
}

/** Drop model output that grew into an answer instead of a light tidy. */
export function acceptCleanupResult(original: string, cleaned: string): string {
  const source = original.trim()
  const result = cleaned.trim()
  if (!result) return source
  const extra = Math.max(8, Math.floor(source.length * 0.2))
  if (result.length > source.length + extra) return source
  return result
}
