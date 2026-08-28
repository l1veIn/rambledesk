import type { JSONContent } from '@tiptap/core'

import {
  actionBlockquoteNode,
  isActionBlockquote,
  type ActionIdentity,
} from './actionBlockquote'
import { attachmentMarkdownUrl, isImageMediaType } from './attachmentMarkdown'
import type { AttachmentView } from './feedback'
import { asrParagraphAttrs } from './speechBlockMetadata'

export type ActiveAction = ActionIdentity | null

export type DraftOperation =
  | { kind: 'appendSpeech'; segmentId: string; text: string; action: ActiveAction }
  | { kind: 'appendClipboardText'; text: string; label: string; action: ActiveAction }
  | { kind: 'appendAttachment'; attachment: AttachmentView; label: string; action: ActiveAction }
  | { kind: 'startActionGroup'; action: ActionIdentity }

export function speechNodes(segmentId: string, text: string): JSONContent[] {
  return [
    {
      type: 'paragraph',
      attrs: asrParagraphAttrs(segmentId, 'pending'),
      content: [{ type: 'text', text }],
    },
  ]
}

export function clipboardNodes(text: string, label: string): JSONContent[] {
  const capturedContent = text.split(/\r?\n/).flatMap((line, index) => {
    const content: JSONContent[] = []
    if (index > 0) content.push({ type: 'hardBreak' })
    if (line) content.push({ type: 'text', text: line })
    return content
  })
  return [
    {
      type: 'blockquote',
      content: [
        {
          type: 'paragraph',
          content: [{ type: 'text', text: label, marks: [{ type: 'bold' }] }],
        },
        { type: 'paragraph', content: capturedContent },
      ],
    },
  ]
}

export function attachmentNodes(
  attachment: AttachmentView,
  label: string,
  previewSrc?: string,
): JSONContent[] {
  const media: JSONContent = isImageMediaType(attachment.media_type)
    ? {
        type: 'image',
        attrs: {
          src: previewSrc ?? attachmentMarkdownUrl(attachment.attachment_id),
          alt: attachment.file_name,
          attachmentId: attachment.attachment_id,
        },
      }
    : {
        type: 'paragraph',
        content: [
          {
            type: 'attachmentFile',
            attrs: {
              attachmentId: attachment.attachment_id,
              fileName: attachment.file_name,
              mediaType: attachment.media_type,
            },
          },
        ],
      }
  return [
    {
      type: 'blockquote',
      content: [
        {
          type: 'paragraph',
          content: [{ type: 'text', text: label, marks: [{ type: 'bold' }] }],
        },
      ],
    },
    media,
  ]
}

function lastNode(doc: JSONContent): JSONContent | undefined {
  const content = doc.content ?? []
  return content[content.length - 1]
}

function isOpenActionGroup(node: JSONContent | undefined, actionId: string): boolean {
  return Boolean(node && isActionBlockquote(node) && node.attrs?.[ 'actionId'] === actionId)
}

function appendNodes(
  doc: JSONContent,
  nodes: JSONContent[],
  action: ActiveAction,
): JSONContent {
  const content = [...(doc.content ?? [])]
  if (!action) {
    return { type: 'doc', content: [...content, ...nodes] }
  }
  const last = lastNode({ type: 'doc', content })
  if (isOpenActionGroup(last, action.actionId) && last) {
    content[content.length - 1] = {
      ...last,
      content: [...(last.content ?? []), ...nodes],
    }
    return { type: 'doc', content }
  }
  content.push(actionBlockquoteNode(action, nodes))
  return { type: 'doc', content }
}

export function applyDraftOperation(doc: JSONContent, operation: DraftOperation): JSONContent {
  switch (operation.kind) {
    case 'appendSpeech':
      return appendNodes(doc, speechNodes(operation.segmentId, operation.text), operation.action)
    case 'appendClipboardText':
      return appendNodes(
        doc,
        clipboardNodes(operation.text, operation.label),
        operation.action,
      )
    case 'appendAttachment':
      return appendNodes(
        doc,
        attachmentNodes(operation.attachment, operation.label),
        operation.action,
      )
    case 'startActionGroup': {
      const last = lastNode(doc)
      if (isOpenActionGroup(last, operation.action.actionId)) return doc
      return {
        type: 'doc',
        content: [...(doc.content ?? []), actionBlockquoteNode(operation.action)],
      }
    }
  }
}
