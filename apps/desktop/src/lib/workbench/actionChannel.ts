import type { JSONContent } from '@tiptap/core'

export const ACTION_CHANNEL_ATTR = 'actionIndex'

const ACTION_MARKER = /^@ Action (\d+)$/
const DEFAULT_MARKER = /^@$/

export function parseActionChannelLine(line: string): number | null | undefined {
  const trimmed = line.trim()
  if (DEFAULT_MARKER.test(trimmed)) return null
  const match = ACTION_MARKER.exec(trimmed)
  if (match) return Number(match[1])
  return undefined
}

export function actionChannelMarker(index: number | null): string {
  return index == null ? '@' : `@ Action ${index}`
}

export type ActionChannelBlock = {
  actionIndex: number | null
  markdown: string
}

export function splitActionChannelMarkdown(source: string): ActionChannelBlock[] {
  const text = source.replace(/^\uFEFF/, '')
  if (!text.trim()) return []
  const lines = text.split('\n')
  const segments: ActionChannelBlock[] = []
  let current: number | null = null
  let buffer: string[] = []

  function flush() {
    const markdown = buffer.join('\n').trim()
    buffer = []
    if (markdown) segments.push({ actionIndex: current, markdown })
  }

  for (const line of lines) {
    const marker = parseActionChannelLine(line)
    if (marker !== undefined) {
      flush()
      current = marker
      continue
    }
    buffer.push(line)
  }
  flush()
  return segments
}

export function joinActionChannelMarkdown(blocks: ActionChannelBlock[]): string {
  const parts: string[] = []
  let current: number | null | undefined
  for (const block of blocks) {
    const markdown = block.markdown.trim()
    if (!markdown) continue
    if (current === undefined) {
      if (block.actionIndex != null) parts.push(actionChannelMarker(block.actionIndex))
    } else if (block.actionIndex !== current) {
      parts.push(actionChannelMarker(block.actionIndex))
    }
    current = block.actionIndex
    parts.push(markdown)
  }
  return parts.join('\n\n')
}

export function appendActionChannelBlock(
  body: string,
  block: string,
  actionIndex: number | null,
): string {
  const markdown = block.trim()
  if (!markdown) return body.trimEnd()
  return joinActionChannelMarkdown([
    ...splitActionChannelMarkdown(body),
    { actionIndex, markdown },
  ])
}

function withoutActionIndex(node: JSONContent): JSONContent {
  if (node.attrs?.[ACTION_CHANNEL_ATTR] == null) return node
  const { [ACTION_CHANNEL_ATTR]: _removed, ...attrs } = node.attrs
  return { ...node, attrs: Object.keys(attrs).length > 0 ? attrs : undefined }
}

export function stampActionIndex(node: JSONContent, actionIndex: number | null): JSONContent {
  if (actionIndex == null) return node
  return {
    ...node,
    attrs: { ...node.attrs, [ACTION_CHANNEL_ATTR]: actionIndex },
  }
}

export function actionIndexOf(node: JSONContent): number | null {
  const value = node.attrs?.[ACTION_CHANNEL_ATTR]
  return typeof value === 'number' && value > 0 ? value : null
}

export function serializeDocWithActionChannels(
  doc: JSONContent,
  serialize: (doc: JSONContent) => string,
): string {
  const blocks = (doc.content ?? []).map((node) => ({
    actionIndex: actionIndexOf(node),
    markdown: serialize({ type: 'doc', content: [withoutActionIndex(node)] }).trim(),
  }))
  return joinActionChannelMarkdown(blocks)
}

export function parseDocWithActionChannels(
  markdown: string,
  parse: (markdown: string) => JSONContent,
): JSONContent {
  const segments = splitActionChannelMarkdown(markdown)
  if (segments.length === 0) {
    const parsed = parse(markdown)
    return parsed.content?.length ? parsed : { type: 'doc', content: [{ type: 'paragraph' }] }
  }
  const content: JSONContent[] = []
  for (const segment of segments) {
    for (const node of parse(segment.markdown).content ?? []) {
      content.push(stampActionIndex(node, segment.actionIndex))
    }
  }
  return {
    type: 'doc',
    content: content.length > 0 ? content : [{ type: 'paragraph' }],
  }
}
