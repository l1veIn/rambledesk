export const CLEANUP_STABLE_THRESHOLD = 3
export const CLEANUP_CHAR_THRESHOLD = 500
export const CLEANUP_SILENCE_MS = 3_000
export const CLEANUP_TIMEOUT_MS = 30_000

export type CleanupTrigger = 'stable-count' | 'char-count' | 'silence' | 'non-speech' | 'settle'

export function pendingCharCount(pieces: readonly string[]): number {
  return pieces.reduce((sum, piece) => sum + piece.trim().length, 0)
}

export function shouldStartCleanup(input: {
  enabled: boolean
  busy: boolean
  pendingPieces: readonly string[]
  trigger: CleanupTrigger
}): boolean {
  if (!input.enabled || input.busy) return false
  if (input.pendingPieces.length === 0) return false
  if (input.trigger === 'non-speech' || input.trigger === 'settle') {
    return true
  }
  if (input.trigger === 'silence') {
    return false
  }
  if (input.trigger === 'stable-count') {
    return input.pendingPieces.length >= CLEANUP_STABLE_THRESHOLD
  }
  return pendingCharCount(input.pendingPieces) >= CLEANUP_CHAR_THRESHOLD
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
