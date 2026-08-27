export const DEFAULT_CLEANUP_SEGMENT_THRESHOLD = 3
export const DEFAULT_CLEANUP_CHAR_THRESHOLD = 500
export const DEFAULT_CLEANUP_IDLE_MS = 20_000
export const DEFAULT_CLEANUP_TIMEOUT_MS = 30_000

export type CleanupTrigger =
  | 'segment-count'
  | 'char-count'
  | 'idle'
  | 'non-speech'
  | 'settle'
  | 'manual'

export type SpeechCleanupThresholds = {
  segmentThreshold: number
  charThreshold: number
}

export function shouldStartCleanup(input: {
  enabled: boolean
  busy: boolean
  pendingCount: number
  pendingChars: number
  trigger: CleanupTrigger
  thresholds: SpeechCleanupThresholds
}): boolean {
  // The manual button works with or without auto tidy enabled; every
  // automatic trigger requires the toggle.
  if (input.trigger !== 'manual' && !input.enabled) return false
  if (input.busy) return false
  if (input.pendingCount === 0) return false
  if (input.trigger === 'idle' || input.trigger === 'non-speech' || input.trigger === 'settle') {
    return true
  }
  if (input.trigger === 'manual') return true
  if (input.trigger === 'segment-count') {
    return input.pendingCount >= input.thresholds.segmentThreshold
  }
  return input.pendingChars >= input.thresholds.charThreshold
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

/**
 * Precisely maps a labeled model output back onto the original nodes.
 *
 * The tidy prompt asks for exactly the same number of blocks, in the same
 * order, each starting with `[n]` (`[1] ...`). When that contract holds, every
 * block can be refilled into its own node — no fragile counting of blank
 * lines. Any deviation (merged blocks, dropped/misordered labels) returns null
 * and the caller falls back to the whole-batch collapse.
 */
export function parseLabeledOutput(text: string, count: number): string[] | null {
  if (count <= 0) return null
  const source = text.trim()
  const pattern = /(?:^|[\r\n])\s*\[(\d+)\][ \t]*/g
  const matches: { index: number; pos: number; end: number }[] = []
  let match: RegExpExecArray | null
  while ((match = pattern.exec(source)) != null) {
    matches.push({ index: Number(match[1]), pos: match.index, end: pattern.lastIndex })
  }
  if (matches.length !== count) return null
  if (matches[0]!.pos !== 0) return null
  const parts: string[] = []
  for (let i = 0; i < matches.length; i++) {
    const current = matches[i]!
    if (current.index !== i + 1) return null
    const end = i + 1 < matches.length ? matches[i + 1]!.pos : source.length
    const block = source.slice(current.end, end).trim()
    if (!block) return null
    parts.push(block)
  }
  return parts
}

/** Drop model output that grew into an answer instead of a light tidy. */
export function acceptCleanupResult(original: string, cleaned: string): string {
  const source = original.trim()
  const result = normalizeCleanupNewlines(cleaned.trim())
  if (!result) return source
  const extra = Math.max(8, Math.floor(source.length * 0.2))
  if (result.length > source.length + extra) return source
  return result
}

/**
 * Models sometimes return JSON-escaped newlines (a literal backslash-n) instead
 * of real line breaks; normalize both so batch splitting (\n{2,}) and the
 * per-segment alignment see the real structure.
 */
export function normalizeCleanupNewlines(text: string): string {
  return text.replace(/\\r\\n/g, '\n').replace(/\\n/g, '\n').replace(/\r\n/g, '\n')
}
