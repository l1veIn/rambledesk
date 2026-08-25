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

export function joinTranscriptChunks(chunks: string[]): string {
  return chunks
    .map((chunk) => chunk.trim())
    .filter((chunk) => chunk.length > 0)
    .join('\n')
}

export function appendRambleClip(clips: RambleClip[], text: string): RambleClip[] {
  const cleaned = text.trim()
  if (!cleaned) return clips
  return [...clips, { id: `ramble:${clips.length}`, text: cleaned }]
}

export function appendBlockNote(
  notes: Record<string, string[]>,
  blockId: string,
  note: string,
): Record<string, string[]> {
  const cleaned = note.trim()
  if (!cleaned) return notes
  return { ...notes, [blockId]: [...(notes[blockId] ?? []), cleaned] }
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
