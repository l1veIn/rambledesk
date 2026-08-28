import { Extension, type JSONContent } from '@tiptap/core'

export const ACTION_ID_ATTR = 'actionId'
export const ACTION_INDEX_ATTR = 'actionIndex'

export type ActionIdentity = {
  actionId: string
  actionIndex: number
  title: string
}

function paragraphText(node: JSONContent): string {
  return (node.content ?? []).map((child) => child.text ?? '').join('').trim()
}

const ACTION_TITLE_PATTERN = /^@Action\s+(\d+)(?:\s*[·•\-]\s*(.*))?$/i
const ACTION_SEPARATOR_PATTERN = /^-{16,}(?:\s+Action\s+(\d+)\s+)?-{16,}$/

export function actionTitleText(identity: ActionIdentity): string {
  const label = identity.title.trim()
  return label
    ? `@Action ${identity.actionIndex + 1} · ${label}`
    : `@Action ${identity.actionIndex + 1}`
}

export function parseActionTitle(text: string): { actionIndex: number; title: string } | null {
  const match = ACTION_TITLE_PATTERN.exec(text.trim())
  if (!match) return null
  const actionIndex = Number(match[1]) - 1
  if (!Number.isInteger(actionIndex) || actionIndex < 0) return null
  return { actionIndex, title: (match[2] ?? '').trim() }
}

export function isActionBlockquote(node: JSONContent): boolean {
  return node.type === 'blockquote' && typeof node.attrs?.[ACTION_ID_ATTR] === 'string'
}

export function actionBlockquoteNode(
  identity: ActionIdentity,
  content: JSONContent[] = [],
): JSONContent {
  return {
    type: 'blockquote',
    attrs: {
      [ACTION_ID_ATTR]: identity.actionId,
      [ACTION_INDEX_ATTR]: identity.actionIndex,
    },
    content: [
      {
        type: 'paragraph',
        content: [{ type: 'text', text: actionTitleText(identity), marks: [{ type: 'bold' }] }],
      },
      ...content,
    ],
  }
}

export function stripActionStamp(node: JSONContent): JSONContent {
  if (node.attrs?.[ACTION_INDEX_ATTR] == null && node.attrs?.[ACTION_ID_ATTR] == null) {
    return node
  }
  const {
    [ACTION_INDEX_ATTR]: _index,
    [ACTION_ID_ATTR]: _id,
    ...attrs
  } = node.attrs ?? {}
  return {
    ...node,
    attrs: Object.keys(attrs).length > 0 ? attrs : undefined,
  }
}

export function actionStampOf(node: JSONContent): { actionId: string; actionIndex: number } | null {
  const actionId = node.attrs?.[ACTION_ID_ATTR]
  const actionIndex = node.attrs?.[ACTION_INDEX_ATTR]
  if (typeof actionId === 'string' && actionId && typeof actionIndex === 'number') {
    return { actionId, actionIndex }
  }
  if (typeof actionIndex === 'number' && actionIndex > 0) {
    return { actionId: `legacy-action-${actionIndex}`, actionIndex: actionIndex - 1 }
  }
  return null
}

export function isActionSeparatorNode(node: JSONContent): boolean {
  if (node.type === 'horizontalRule') return true
  if (node.type !== 'paragraph') return false
  return ACTION_SEPARATOR_PATTERN.test(paragraphText(node))
}

export function actionSeparatorIndex(node: JSONContent): number | null {
  if (node.type === 'horizontalRule') return null
  const match = ACTION_SEPARATOR_PATTERN.exec(paragraphText(node))
  if (!match?.[1]) return null
  const channel = Number(match[1])
  return Number.isInteger(channel) && channel > 0 ? channel : null
}

export function hydrateActionBlockquotes(doc: JSONContent): JSONContent {
  const content = (doc.content ?? []).map((node) => {
    if (node.type !== 'blockquote' || isActionBlockquote(node)) return node
    const title = parseActionTitle(paragraphText(node.content?.[0] ?? {}))
    if (!title) return node
    return {
      ...node,
      attrs: {
        ...node.attrs,
        [ACTION_ID_ATTR]: `legacy-action-${title.actionIndex + 1}`,
        [ACTION_INDEX_ATTR]: title.actionIndex,
      },
    }
  })
  return { ...doc, content }
}

export const ActionBlockquote = Extension.create({
  name: 'actionBlockquote',

  addGlobalAttributes() {
    return [
      {
        types: ['blockquote'],
        attributes: {
          [ACTION_ID_ATTR]: {
            default: null,
            parseHTML: (element) => element.getAttribute('data-action-id'),
            renderHTML: (attributes) =>
              typeof attributes[ACTION_ID_ATTR] === 'string' && attributes[ACTION_ID_ATTR]
                ? { 'data-action-id': attributes[ACTION_ID_ATTR] }
                : {},
          },
          [ACTION_INDEX_ATTR]: {
            default: null,
            parseHTML: (element) => {
              const value = element.getAttribute('data-action-index')
              if (value == null) return null
              const parsed = Number(value)
              return Number.isInteger(parsed) && parsed >= 0 ? parsed : null
            },
            renderHTML: (attributes) =>
              typeof attributes[ACTION_INDEX_ATTR] === 'number'
                ? { 'data-action-index': String(attributes[ACTION_INDEX_ATTR]) }
                : {},
          },
        },
      },
    ]
  },
})
