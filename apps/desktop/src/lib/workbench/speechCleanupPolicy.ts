export function normalizeCleanupNewlines(text: string): string {
  return text.replace(/\\r\\n/g, '\n').replace(/\\n/g, '\n').replace(/\r\n/g, '\n')
}

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
  if (matches[0]!.pos !== 0 && source.slice(0, matches[0]!.pos).trim()) return null
  if (matches[0]!.pos !== 0) return null
  const parts: string[] = []
  for (let i = 0; i < matches.length; i++) {
    const current = matches[i]!
    if (current.index !== i + 1) return null
    const end = i + 1 < matches.length ? matches[i + 1]!.pos : source.length
    const block = source.slice(current.end, end).trim()
    parts.push(block)
  }
  return parts
}

/** Drop model output that grew into an answer instead of a light tidy. */
export function acceptCleanupResult(original: string, cleaned: string): string | null {
  const source = original.trim()
  const result = normalizeCleanupNewlines(cleaned.trim())
  if (!result) return ''
  const extra = Math.max(8, Math.floor(source.length * 0.2))
  if (result.length > source.length + extra) return null
  return result
}
