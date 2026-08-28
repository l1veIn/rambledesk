import type { JSONContent } from '@tiptap/core'

import {
  ACTION_ID_ATTR,
  isActionBlockquote,
  isEmptyActionGroup,
  parseActionTitle,
} from './actionBlockquote'

export type CollectedActionGroupContent = {
  document: JSONContent
  groupCount: number
}

function nodeText(node: JSONContent): string {
  return (node.content ?? []).map((child) => child.text ?? nodeText(child)).join('')
}

function actionGroupBody(node: JSONContent): JSONContent[] {
  const content = node.content ?? []
  const firstNode = content[0]
  return firstNode && parseActionTitle(nodeText(firstNode)) ? content.slice(1) : content
}

/** Collect every non-empty top-level Action group, preserving document order. */
export function collectActionGroupContent(
  document: JSONContent | null | undefined,
): Map<string, CollectedActionGroupContent> {
  const collected = new Map<string, CollectedActionGroupContent>()

  for (const node of document?.content ?? []) {
    if (!isActionBlockquote(node) || isEmptyActionGroup(node)) continue

    const actionId = node.attrs?.[ACTION_ID_ATTR]
    if (typeof actionId !== 'string' || !actionId) continue

    const body = actionGroupBody(node)
    if (body.length === 0) continue

    const existing = collected.get(actionId)
    if (existing) {
      existing.document.content?.push(...body)
      existing.groupCount += 1
      continue
    }

    collected.set(actionId, {
      document: { type: 'doc', content: [...body] },
      groupCount: 1,
    })
  }

  return collected
}
