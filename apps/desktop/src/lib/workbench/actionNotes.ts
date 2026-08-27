import type { JSONContent } from '@tiptap/core'

import { decodeFeedbackDraftDocument } from '../feedbackDraftDocument'
import {
  parseFeedbackMarkdown,
  serializeFeedbackMarkdown,
} from '../feedbackEditorExtensions'
import { actionIndexOf } from './actionChannel'

function withoutActionIndex(node: JSONContent): JSONContent {
  if (node.attrs?.actionIndex == null) return node
  const { actionIndex: _removed, ...attrs } = node.attrs
  return { ...node, attrs: Object.keys(attrs).length > 0 ? attrs : undefined }
}

/**
 * Groups the draft blocks that are stamped with each Action number and
 * renders them as plain Markdown — one entry per Action, for the full-screen
 * task brief that shows the human's own notes under each action item.
 */
export function actionNotesFromDocument(
  documentJson: string | null | undefined,
  bodyMarkdown: string,
): Record<number, string> {
  const doc = decodeFeedbackDraftDocument(documentJson) ?? parseFeedbackMarkdown(bodyMarkdown)
  const groups = new Map<number, JSONContent[]>()
  for (const node of doc.content ?? []) {
    const index = actionIndexOf(node)
    if (index == null) continue
    const list = groups.get(index) ?? []
    list.push(node)
    groups.set(index, list)
  }
  const notes: Record<number, string> = {}
  for (const [index, nodes] of groups) {
    notes[index] = serializeFeedbackMarkdown({
      type: 'doc',
      content: nodes.map(withoutActionIndex),
    })
  }
  return notes
}
