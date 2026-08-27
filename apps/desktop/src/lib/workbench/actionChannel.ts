import type { JSONContent } from '@tiptap/core'

export const ACTION_CHANNEL_ATTR = 'actionIndex'

const ACTION_SEPARATOR_SIDE = '------------------------'

/**
 * A line like `---------------- Action 2 ----------------` (or the plain dashes
 * for the default channel). It is only meaningful as a markdown projection of
 * node attributes; when it appears in stored markdown (e.g. drafts saved
 * before structured documents existed) and has to be parsed back into the
 * editor, it must become a channel stamp instead of visible text.
 */
const ACTION_SEPARATOR_PATTERN = /^-{16,}(?:\s+Action\s+(\d+)\s+)?-{16,}$/

export function actionChannelSeparator(index: number | null): string {
  return index == null
    ? `${ACTION_SEPARATOR_SIDE}${ACTION_SEPARATOR_SIDE}`
    : `${ACTION_SEPARATOR_SIDE} Action ${index} ${ACTION_SEPARATOR_SIDE}`
}

export type ActionChannelBlock = {
  actionIndex: number | null
  markdown: string
}

export function joinActionChannelMarkdown(blocks: ActionChannelBlock[]): string {
  const parts: string[] = []
  let current: number | null | undefined
  for (const block of blocks) {
    const markdown = block.markdown.trim()
    if (!markdown) continue
    if (current === undefined) {
      if (block.actionIndex != null) parts.push(actionChannelSeparator(block.actionIndex))
    } else if (block.actionIndex !== current) {
      parts.push(actionChannelSeparator(block.actionIndex))
    }
    current = block.actionIndex
    parts.push(markdown)
  }
  return parts.join('\n\n')
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

function paragraphText(node: JSONContent): string | null {
  if (node.type !== 'paragraph') return null
  return (node.content ?? [])
    .map((child) => child.text ?? '')
    .join('')
    .trim()
}

/**
 * Replaces action-separator lines in a parsed document with channel stamps.
 *
 * Legacy drafts (no persisted document JSON) carry the channel boundaries as
 * markdown divider lines; parsing them back as paragraphs would show the
 * divider inside the editor. Drop the divider and stamp the following blocks
 * with the channel it opened, so the grouping survives and the divider is
 * re-derived on the next markdown export.
 *
 * The default-channel separator (`--------------------------------`) parses as a
 * horizontal rule; the editor has no horizontal-rule affordance, so such a
 * rule is treated as "back to the default channel" and dropped instead of
 * showing a stray line.
 */
export function migrateActionChannelSeparators(doc: JSONContent): JSONContent {
  const content = doc.content ?? []
  let current: number | null = null
  let strip: boolean | null = null
  const migrated: JSONContent[] = []
  for (const node of content) {
    if (node.type === 'horizontalRule') {
      // The default-channel separator parses as a horizontal rule; the editor
      // has no horizontal-rule affordance, so treat it as "back to the default
      // channel" and drop the stray line.
      current = null
      strip = true
      continue
    }
    const text = paragraphText(node)
    const match = text == null ? null : ACTION_SEPARATOR_PATTERN.exec(text)
    if (match) {
      const raw = match[1]
      const channel = raw == null ? Number.NaN : Number(raw)
      current = Number.isInteger(channel) && channel > 0 ? channel : null
      strip = current == null
      continue
    }
    if (strip === true) {
      migrated.push(withoutActionIndex(node))
    } else {
      migrated.push(
        current != null && actionIndexOf(node) == null ? stampActionIndex(node, current) : node,
      )
    }
  }
  return { ...doc, content: migrated }
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
