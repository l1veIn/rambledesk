import type { JSONContent } from '@tiptap/core'

import { attachmentMarkdownUrl } from './attachmentMarkdown'
import {
  parseFeedbackMarkdown,
  serializeFeedbackMarkdown,
} from './feedbackEditorExtensions'
import { ACTION_CHANNEL_ATTR } from './workbench/actionChannel'

export const FEEDBACK_DRAFT_DOCUMENT_VERSION = 1

type PersistedFeedbackDraftDocument = {
  schemaVersion: number
  doc: JSONContent
}

export type FeedbackDraftSnapshot = {
  documentJson: string
  bodyMarkdown: string
}

function isDocument(value: unknown): value is JSONContent {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  return record.type === 'doc' && (record.content === undefined || Array.isArray(record.content))
}

function canonicalizeNode(node: JSONContent): JSONContent {
  const content = node.content?.map(canonicalizeNode)
  if (
    node.type === 'paragraph' &&
    !content?.length &&
    node.attrs?.[ACTION_CHANNEL_ATTR] != null
  ) {
    const { [ACTION_CHANNEL_ATTR]: _currentChannel, ...attrs } = node.attrs
    return {
      ...node,
      attrs: Object.keys(attrs).length > 0 ? attrs : undefined,
      content,
    }
  }
  if (node.type !== 'image' || typeof node.attrs?.attachmentId !== 'string') {
    return content ? { ...node, content } : node
  }
  return {
    ...node,
    attrs: {
      ...node.attrs,
      src: attachmentMarkdownUrl(node.attrs.attachmentId),
    },
    ...(content ? { content } : {}),
  }
}

export function decodeFeedbackDraftDocument(source: string | null | undefined): JSONContent | null {
  if (!source) return null
  try {
    const parsed = JSON.parse(source) as Partial<PersistedFeedbackDraftDocument>
    if (parsed.schemaVersion !== FEEDBACK_DRAFT_DOCUMENT_VERSION || !isDocument(parsed.doc)) {
      return null
    }
    return parsed.doc
  } catch {
    return null
  }
}

export function snapshotFeedbackDraftDocument(doc: JSONContent): FeedbackDraftSnapshot {
  const canonical = canonicalizeNode(doc)
  return {
    documentJson: JSON.stringify({
      schemaVersion: FEEDBACK_DRAFT_DOCUMENT_VERSION,
      doc: canonical,
    } satisfies PersistedFeedbackDraftDocument),
    bodyMarkdown: serializeFeedbackMarkdown(canonical),
  }
}

export function restoreFeedbackDraftDocument(
  documentJson: string | null | undefined,
  bodyMarkdown: string,
): JSONContent {
  return decodeFeedbackDraftDocument(documentJson) ?? parseFeedbackMarkdown(bodyMarkdown)
}

export function snapshotFeedbackDraftMarkdown(bodyMarkdown: string): FeedbackDraftSnapshot {
  return snapshotFeedbackDraftDocument(parseFeedbackMarkdown(bodyMarkdown))
}

export function updateFeedbackDraftDocument(
  snapshot: FeedbackDraftSnapshot,
  update: (doc: JSONContent) => JSONContent,
): FeedbackDraftSnapshot {
  const doc =
    decodeFeedbackDraftDocument(snapshot.documentJson) ?? parseFeedbackMarkdown(snapshot.bodyMarkdown)
  return snapshotFeedbackDraftDocument(update(doc))
}
