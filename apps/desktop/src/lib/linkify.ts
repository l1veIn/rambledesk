export type TextSegment = { type: 'text'; value: string } | { type: 'url'; value: string }

const URL_PATTERN = /https?:\/\/[^\s<>"'）】」』，。、；：！？]+/gi
const TRAILING_PUNCTUATION = /[),.;:!?]+$/u

export function isSafeHttpUrl(href: string) {
  try {
    const url = new URL(href)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

export function splitTextWithUrls(text: string): TextSegment[] {
  const segments: TextSegment[] = []
  const pattern = new RegExp(URL_PATTERN.source, URL_PATTERN.flags)
  let cursor = 0
  for (const match of text.matchAll(pattern)) {
    const raw = match[0] ?? ''
    const index = match.index ?? 0
    const href = raw.replace(TRAILING_PUNCTUATION, '')
    if (!href || !isSafeHttpUrl(href)) continue
    if (index > cursor) segments.push({ type: 'text', value: text.slice(cursor, index) })
    segments.push({ type: 'url', value: href })
    cursor = index + href.length
  }
  if (cursor < text.length) segments.push({ type: 'text', value: text.slice(cursor) })
  return segments.length > 0 ? segments : [{ type: 'text', value: text }]
}
