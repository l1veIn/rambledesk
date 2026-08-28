import type { JSONContent } from '@tiptap/core'

import {
  ACTION_ID_ATTR,
  ACTION_INDEX_ATTR,
  actionSeparatorIndex,
  actionStampOf,
  actionTitleText,
  hydrateActionBlockquotes,
  isActionSeparatorNode,
  parseActionTitle,
  stripActionStamp,
} from './actionBlockquote'
import { attachmentMarkdownUrl } from './attachmentMarkdown'
import {
  parseFeedbackMarkdown,
  serializeFeedbackMarkdown,
} from './feedbackEditorExtensions'
import {
  ASR_INPUT_SOURCE,
  CLEANUP_STATE_ATTR,
  INPUT_SOURCE_ATTR,
  SPEECH_SEGMENT_ID_ATTR,
} from './speechBlockMetadata'

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

function compactAttrs(
  attrs: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (!attrs) return undefined
  const next = Object.fromEntries(
    Object.entries(attrs).filter(([, value]) => value != null && value !== ''),
  )
  return Object.keys(next).length > 0 ? next : undefined
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

function normalizeSpeechSegmentIds(doc: JSONContent): JSONContent {
  const reservedIds = new Set<string>()

  function reserve(node: JSONContent) {
    const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
    if (typeof segmentId === 'string' && segmentId.length > 0) {
      reservedIds.add(segmentId)
    }
    node.content?.forEach(reserve)
  }
  reserve(doc)

  const seenIds = new Set<string>()
  let restoredIndex = 0

  function nextRestoredId(): string {
    do {
      restoredIndex += 1
    } while (reservedIds.has(`restored-asr-${restoredIndex}`))
    const segmentId = `restored-asr-${restoredIndex}`
    reservedIds.add(segmentId)
    return segmentId
  }

  function normalize(node: JSONContent): JSONContent {
    const content = node.content?.map(normalize)
    const attrs = node.attrs ?? {}
    const currentId = attrs[SPEECH_SEGMENT_ID_ATTR]
    const isSpeechParagraph =
      node.type === 'paragraph' &&
      (attrs[INPUT_SOURCE_ATTR] === ASR_INPUT_SOURCE ||
        (typeof currentId === 'string' && currentId.length > 0))

    if (!isSpeechParagraph) {
      return content ? { ...node, content } : node
    }

    const segmentId =
      typeof currentId === 'string' && currentId.length > 0 && !seenIds.has(currentId)
        ? currentId
        : nextRestoredId()
    seenIds.add(segmentId)
    const cleanupState =
      attrs[CLEANUP_STATE_ATTR] === 'cleaned' ? 'cleaned' : 'pending'

    return {
      ...node,
      attrs: {
        ...attrs,
        [SPEECH_SEGMENT_ID_ATTR]: segmentId,
        [INPUT_SOURCE_ATTR]: ASR_INPUT_SOURCE,
        [CLEANUP_STATE_ATTR]: cleanupState,
      },
      ...(content ? { content } : {}),
    }
  }

  return normalize(doc)
}

function stampLegacyActionSeparators(doc: JSONContent): JSONContent {
  const migrated: JSONContent[] = []
  let current: number | null = null
  for (const node of doc.content ?? []) {
    if (isActionSeparatorNode(node)) {
      current = actionSeparatorIndex(node)
      continue
    }
    if (current != null && actionStampOf(node) == null) {
      migrated.push({
        ...node,
        attrs: { ...node.attrs, [ACTION_INDEX_ATTR]: current },
      })
    } else {
      migrated.push(node)
    }
  }
  return { ...doc, content: migrated }
}

function wrapActionStamps(doc: JSONContent): JSONContent {
  const next: JSONContent[] = []
  const content = doc.content ?? []
  let index = 0
  while (index < content.length) {
    const node = content[index]
    if (node.type === 'blockquote') {
      next.push(node)
      index += 1
      continue
    }
    const stamp = actionStampOf(node)
    if (!stamp) {
      next.push(stripActionStamp(node))
      index += 1
      continue
    }
    const group: JSONContent[] = []
    while (index < content.length) {
      const candidate = content[index]
      if (candidate.type === 'blockquote') break
      const candidateStamp = actionStampOf(candidate)
      if (
        !candidateStamp ||
        candidateStamp.actionId !== stamp.actionId ||
        candidateStamp.actionIndex !== stamp.actionIndex
      ) {
        break
      }
      group.push(stripActionStamp(candidate))
      index += 1
    }
    const title = parseActionTitle(
      (group[0]?.content ?? []).map((child) => child.text ?? '').join(''),
    )
    if (!title) {
      group.unshift({
        type: 'paragraph',
        content: [
          {
            type: 'text',
            text: actionTitleText({
              actionId: stamp.actionId,
              actionIndex: stamp.actionIndex,
              title: '',
            }),
            marks: [{ type: 'bold' }],
          },
        ],
      })
    }
    next.push({
      type: 'blockquote',
      attrs: {
        [ACTION_ID_ATTR]: stamp.actionId,
        [ACTION_INDEX_ATTR]: stamp.actionIndex,
      },
      content: group,
    })
  }
  return { ...doc, content: next }
}

export function migrateFeedbackDraftDocument(
  doc: JSONContent,
  recognizeLegacySeparators = false,
): JSONContent {
  const speech = migrateLegacySpeechNodes(doc)
  return normalizeSpeechSegmentIds(
    hydrateActionBlockquotes(
      wrapActionStamps(
        recognizeLegacySeparators ? stampLegacyActionSeparators(speech) : speech,
      ),
    ),
  )
}

function canonicalizeNode(node: JSONContent): JSONContent {
  const content = node.content?.map(canonicalizeNode)
  const attrs = compactAttrs(node.attrs as Record<string, unknown> | undefined)
  if (node.type === 'image' && typeof attrs?.attachmentId === 'string') {
    return {
      ...node,
      attrs: {
        ...attrs,
        src: attachmentMarkdownUrl(attrs.attachmentId as string),
      },
      ...(content ? { content } : {}),
    }
  }
  return {
    ...node,
    ...(attrs ? { attrs } : { attrs: undefined }),
    ...(content ? { content } : {}),
  }
}

export function decodeFeedbackDraftDocument(
  source: string | null | undefined,
): JSONContent | null {
  if (!source) return null
  try {
    const parsed = JSON.parse(source) as Partial<PersistedFeedbackDraftDocument>
    if (!isDocument(parsed.doc)) return null
    if (
      parsed.schemaVersion !== FEEDBACK_DRAFT_DOCUMENT_VERSION &&
      parsed.schemaVersion !== LEGACY_FEEDBACK_DRAFT_DOCUMENT_VERSION
    ) {
      return null
    }
    return migrateFeedbackDraftDocument(
      parsed.doc,
      parsed.schemaVersion === LEGACY_FEEDBACK_DRAFT_DOCUMENT_VERSION,
    )
  } catch {
    return null
  }
}

export function snapshotFeedbackDraftDocument(doc: JSONContent): FeedbackDraftSnapshot {
  const canonical = canonicalizeNode(migrateFeedbackDraftDocument(doc))
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
  return (
    decodeFeedbackDraftDocument(documentJson) ??
    migrateFeedbackDraftDocument(parseFeedbackMarkdown(bodyMarkdown), true)
  )
}

export function snapshotFeedbackDraftMarkdown(bodyMarkdown: string): FeedbackDraftSnapshot {
  return snapshotFeedbackDraftDocument(parseFeedbackMarkdown(bodyMarkdown))
}

export function appendMarkdownBlockToDocument(
  doc: JSONContent,
  markdown: string,
): JSONContent {
  const incoming = parseFeedbackMarkdown(markdown)
  return {
    type: 'doc',
    content: [...(doc.content ?? []), ...(incoming.content ?? [])],
  }
}

export function updateFeedbackDraftDocument(
  snapshot: FeedbackDraftSnapshot,
  update: (doc: JSONContent) => JSONContent,
): FeedbackDraftSnapshot {
  return snapshotFeedbackDraftDocument(update(restoreFeedbackDraftDocument(snapshot.documentJson, snapshot.bodyMarkdown)))
}
