import type { JSONContent } from '@tiptap/core'

import { attachmentMarkdownUrl } from './attachmentMarkdown'
import {
  parseFeedbackMarkdown,
  serializeFeedbackMarkdown,
} from './feedbackEditorExtensions'
import {
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
  ASR_INPUT_SOURCE,
} from './speechBlockMetadata'
import { ACTION_CHANNEL_ATTR, migrateActionChannelSeparators } from './workbench/actionChannel'

export const FEEDBACK_DRAFT_DOCUMENT_VERSION = 2
const LEGACY_FEEDBACK_DRAFT_DOCUMENT_VERSION = 1

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

function migrateLegacySpeechNodes(doc: JSONContent): JSONContent {
  let segmentIndex = 0

  function migrate(node: JSONContent): JSONContent {
    const content = node.content?.map(migrate)
    if (node.type !== 'pendingSpeech' && node.type !== 'cleanedSpeech') {
      return content ? { ...node, content } : node
    }
    segmentIndex += 1
    const { status: _legacyStatus, ...legacyAttrs } = node.attrs ?? {}
    return {
      ...node,
      type: 'paragraph',
      attrs: {
        ...legacyAttrs,
        [SPEECH_SEGMENT_ID_ATTR]: `legacy-asr-${segmentIndex}`,
        [INPUT_SOURCE_ATTR]: ASR_INPUT_SOURCE,
        [CLEANUP_STATE_ATTR]: node.type === 'cleanedSpeech' ? 'cleaned' : 'pending',
      },
      ...(content ? { content } : {}),
    }
  }

  return migrate(doc)
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
    if (!isDocument(parsed.doc)) {
      return null
    }
    if (parsed.schemaVersion === FEEDBACK_DRAFT_DOCUMENT_VERSION) {
      return migrateActionChannelSeparators(parsed.doc)
    }
    if (parsed.schemaVersion === LEGACY_FEEDBACK_DRAFT_DOCUMENT_VERSION) {
      return migrateActionChannelSeparators(migrateLegacySpeechNodes(parsed.doc))
    }
    return null
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
