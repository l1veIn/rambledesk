export type BriefBlockKind = 'what_happened' | 'action' | 'context'

export type BriefBlock = {
  id: string
  kind: BriefBlockKind
  quote: string
}

export type BriefNoteSource = {
  whatHappened: string
  actions: Array<{ id: string; instruction: string }>
  contextRefs: Array<{ label: string; uri: string }>
}

export function briefBlocks(source: BriefNoteSource): BriefBlock[] {
  const blocks: BriefBlock[] = []
  splitParagraphs(source.whatHappened).forEach((quote, index) => {
    blocks.push({ id: `what_happened:${index}`, kind: 'what_happened', quote })
  })
  for (const action of source.actions) {
    const quote = action.instruction.trim()
    if (!quote) continue
    blocks.push({ id: `action:${action.id}`, kind: 'action', quote })
  }
  source.contextRefs.forEach((ref, index) => {
    const quote = [ref.label.trim(), ref.uri.trim()].filter((part) => part.length > 0).join('\n')
    if (!quote) return
    blocks.push({ id: `context:${index}`, kind: 'context', quote })
  })
  return blocks
}

export function rambleRequestIdAfterIdleNote(
  ramblePhase: string,
  existingRequestId: string,
  workspaceRequestId: string,
): string {
  if (ramblePhase === 'idle') return workspaceRequestId
  return existingRequestId || workspaceRequestId
}

export function findBriefBlock(blocks: BriefBlock[], blockId: string): BriefBlock | undefined {
  return blocks.find((block) => block.id === blockId)
}

export type RambleClip = {
  id: string
  text: string
  processing?: boolean
}

export type ClipFlyFrom = {
  left: number
  top: number
  width: number
  height: number
}

export function clipFlyTransform(
  from: ClipFlyFrom,
  to: ClipFlyFrom,
): { x: number; y: number; scale: number } {
  const fromCx = from.left + from.width / 2
  const fromCy = from.top + from.height / 2
  const toCx = to.left + to.width / 2
  const toCy = to.top + to.height / 2
  const scale =
    to.width > 0 && from.width > 0 ? Math.max(1, Math.min(2.2, from.width / to.width)) : 1.2
  return {
    x: fromCx - toCx,
    y: fromCy - toCy,
    scale,
  }
}

const CAPTURE_ZWSP = '\u200b'

export function wrapCapture(id: string, markdown: string): string {
  const inner = markdown.trim()
  return `[${CAPTURE_ZWSP}](rambledesk-capture://${id})\n\n${inner}\n\n[${CAPTURE_ZWSP}](rambledesk-capture://${id}/end)`
}

export function replaceCapture(body: string, id: string, nextMarkdown: string): string {
  const startTok = `rambledesk-capture://${id}`
  const endTok = `rambledesk-capture://${id}/end`
  const startTokAt = body.indexOf(startTok)
  const endTokAt = body.indexOf(endTok, startTokAt + startTok.length)
  if (startTokAt < 0 || endTokAt < 0) return body
  const startFrom = captureLineStart(body, startTokAt)
  const endTo = captureLineEnd(body, endTokAt)
  return `${body.slice(0, startFrom)}${wrapCapture(id, nextMarkdown)}${body.slice(endTo)}`
}

function captureLineStart(body: string, tokenIndex: number): number {
  const lineStart = body.lastIndexOf('\n', tokenIndex - 1)
  const from = lineStart < 0 ? 0 : lineStart + 1
  const bracket = body.lastIndexOf('[', tokenIndex)
  return bracket >= from ? bracket : from
}

function captureLineEnd(body: string, tokenIndex: number): number {
  const close = body.indexOf(')', tokenIndex)
  const after = close < 0 ? tokenIndex : close + 1
  return body[after] === '\n' ? after + 1 : after
}

export function extractNoteBody(markdown: string): string {
  const lines = markdown.split('\n')
  let index = 0
  while (index < lines.length && (lines[index].startsWith('>') || lines[index].trim() === '')) {
    index += 1
  }
  return lines.slice(index).join('\n').trim() || markdown.trim()
}

export function parseCaptures(body: string): {
  clips: RambleClip[]
  notes: Record<string, string[]>
} {
  const clips: RambleClip[] = []
  const notes: Record<string, string[]> = {}
  const endRe = /rambledesk-capture:\/\/(.+?)\/end/g
  let match: RegExpExecArray | null
  while ((match = endRe.exec(body))) {
    const id = match[1]
    const startTokAt = body.lastIndexOf(`rambledesk-capture://${id})`, match.index)
    if (startTokAt < 0) continue
    const inner = body.slice(captureLineEnd(body, startTokAt), captureLineStart(body, match.index)).trim()
    if (id.startsWith('ramble:')) {
      clips.push({ id, text: inner })
      continue
    }
    if (!id.startsWith('note:')) continue
    const withoutPrefix = id.slice('note:'.length)
    const lastColon = withoutPrefix.lastIndexOf(':')
    if (lastColon <= 0) continue
    const blockId = withoutPrefix.slice(0, lastColon)
    const note = extractNoteBody(inner)
    if (!note) continue
    notes[blockId] = [...(notes[blockId] ?? []), note]
  }
  return { clips, notes }
}

export function nextSavedTranscript(draft: string, saved: string): string | null {
  const next = draft.trim()
  if (!next || next === saved.trim()) return null
  return next
}

export function blockNoteCaptureId(blockId: string): string {
  return `note:${blockId}:0`
}

export function tooltipFixedStyle(
  anchor: { left: number; top: number; width: number; height: number },
  placement: 'top' | 'bottom',
  align: 'left' | 'right',
): { top: number; left: number; transform: string } {
  if (placement === 'bottom') {
    return {
      top: anchor.top + anchor.height + 8,
      left: align === 'right' ? anchor.left + anchor.width : anchor.left,
      transform: align === 'right' ? 'translateX(-100%)' : 'none',
    }
  }
  return {
    top: anchor.top - 8,
    left: align === 'right' ? anchor.left + anchor.width : anchor.left,
    transform: align === 'right' ? 'translate(-100%, -100%)' : 'translateY(-100%)',
  }
}

export function capturedTranscriptMarkdown(text: string): string {
  return text
    .trim()
    .split(/\n+/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
    .join('\n\n')
}

export function replaceLastBlock(body: string, previous: string, next: string): string {
  return replaceNthBlock(body, previous, next, Number.MAX_SAFE_INTEGER)
}

export function sameCaptureOccurrence(texts: string[], index: number): number {
  const target = (texts[index] ?? '').trim()
  if (!target) return 0
  let count = 0
  for (let i = 0; i < index; i += 1) {
    if ((texts[i] ?? '').trim() === target) count += 1
  }
  return count
}

export function replaceNthBlock(
  body: string,
  previous: string,
  next: string,
  occurrence: number,
): string {
  const nextTrim = next.trim()
  if (!previous.trim() || previous.trim() === nextTrim) return body
  for (const candidate of markdownVariants(previous)) {
    const replaced = replaceNthOccurrence(body, candidate, nextTrim, occurrence)
    if (replaced !== body) return replaced
  }
  return body
}

function replaceNthOccurrence(
  body: string,
  previous: string,
  next: string,
  occurrence: number,
): string {
  let from = 0
  let found = -1
  let seen = 0
  while (seen <= occurrence) {
    found = body.indexOf(previous, from)
    if (found < 0) {
      if (occurrence === Number.MAX_SAFE_INTEGER && seen > 0) {
        const last = body.lastIndexOf(previous)
        if (last < 0) return body
        return `${body.slice(0, last)}${next}${body.slice(last + previous.length)}`
      }
      return body
    }
    if (seen === occurrence) {
      return `${body.slice(0, found)}${next}${body.slice(found + previous.length)}`
    }
    from = found + previous.length
    seen += 1
  }
  return body
}

export function replaceRambleClip(clips: RambleClip[], clipId: string, text: string): RambleClip[] {
  const cleaned = text.trim()
  if (!cleaned) return clips
  return clips.map((clip) =>
    clip.id === clipId ? { id: clip.id, text: cleaned } : clip,
  )
}

export function replaceBlockNote(
  notes: Record<string, string[]>,
  blockId: string,
  index: number,
  text: string,
): Record<string, string[]> {
  const current = notes[blockId] ?? []
  if (index < 0 || index >= current.length) return notes
  const cleaned = text.trim()
  if (!cleaned) return notes
  const next = [...current]
  next[index] = cleaned
  return { ...notes, [blockId]: next }
}

function markdownVariants(text: string): string[] {
  const trimmed = text.trim()
  const paragraphs = capturedTranscriptMarkdown(trimmed)
  const lines = trimmed
    .split(/\n+/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
    .join('\n')
  return [...new Set([trimmed, paragraphs, lines].filter((part) => part.length > 0))]
}

export function joinTranscriptChunks(chunks: string[]): string {
  return chunks
    .map((chunk) => chunk.trim())
    .filter((chunk) => chunk.length > 0)
    .join('\n')
}

export function mergeLiveTranscript(chunks: string[], partial: string): string {
  const extra = partial.trim()
  const parts = chunks.map((chunk) => chunk.trim()).filter((chunk) => chunk.length > 0)
  if (!extra) return joinTranscriptChunks(parts)
  const last = parts[parts.length - 1]
  if (!last) return extra
  if (last === extra || last.includes(extra)) return joinTranscriptChunks(parts)
  if (extra.includes(last) || extra.startsWith(last)) {
    return joinTranscriptChunks([...parts.slice(0, -1), extra])
  }
  return joinTranscriptChunks([...parts, extra])
}

export function isCaptureTooltipEvent(
  target: EventTarget | { closest?: (selector: string) => unknown; parentElement?: unknown } | null,
): boolean {
  let current: { closest?: (selector: string) => unknown; parentElement?: unknown } | null =
    target && typeof target === 'object'
      ? (target as { closest?: (selector: string) => unknown; parentElement?: unknown })
      : null
  while (current) {
    if (typeof current.closest === 'function' && current.closest('[data-capture-tooltip]')) return true
    current = (current.parentElement as typeof current) ?? null
  }
  return false
}

export function appendRambleClip(
  clips: RambleClip[],
  text: string,
  id = `ramble:${clips.length}`,
  processing = false,
): RambleClip[] {
  const cleaned = text.trim()
  if (!processing && !cleaned) return clips
  return [...clips, { id, text: cleaned, ...(processing ? { processing: true } : {}) }]
}

export function appendBlockNote(
  notes: Record<string, string[]>,
  blockId: string,
  note: string,
): Record<string, string[]> {
  const cleaned = note.trim()
  if (!cleaned) return notes
  const current = notes[blockId] ?? []
  if (current.length === 0) return { ...notes, [blockId]: [cleaned] }
  return { ...notes, [blockId]: [`${current[0].trim()}\n${cleaned}`] }
}

export function quotedNoteMarkdown(quote: string, note: string): string {
  const cleanedNote = note.trim()
  if (!cleanedNote) return ''
  const quoted = quote
    .trim()
    .split(/\r?\n/)
    .map((line) => `> ${line}`)
    .join('\n')
  return `${quoted}\n\n${cleanedNote}`
}

function splitParagraphs(text: string): string[] {
  return text
    .split(/\n{2,}/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
}
