import type { JSONContent } from '@tiptap/core'

export const ACTION_CHANNEL_ATTR = 'actionIndex'

const ACTION_SEPARATOR_SIDE = '------------------------'

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
