import { Extension, type Editor, type JSONContent } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

export const SPEECH_SEGMENT_ID_ATTR = 'speechSegmentId'
export const INPUT_SOURCE_ATTR = 'inputSource'
export const CLEANUP_STATE_ATTR = 'cleanupState'
export const ASR_INPUT_SOURCE = 'asr'

export type CleanupState = 'pending' | 'cleaned'

export type SpeechCleanupSegment = {
  segmentId: string
  text: string
}

type SpeechTidyingDecorationState = {
  segmentIds: ReadonlySet<string>
  decorations: DecorationSet
}

export const SPEECH_TIDYING_PLUGIN_KEY = new PluginKey<SpeechTidyingDecorationState>(
  'speechTidying',
)

function nodeText(node: JSONContent): string {
  if (typeof node.text === 'string') return node.text
  return (node.content ?? []).map(nodeText).join('')
}

function cleanupState(value: unknown): CleanupState | null {
  return value === 'pending' || value === 'cleaned' ? value : null
}

export function speechCleanupCandidates(doc: JSONContent): SpeechCleanupSegment[] {
  const segments: SpeechCleanupSegment[] = []
  const seen = new Set<string>()

  function visit(node: JSONContent) {
    const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
    if (
      node.type === 'paragraph' &&
      node.attrs?.[INPUT_SOURCE_ATTR] === ASR_INPUT_SOURCE &&
      node.attrs?.[CLEANUP_STATE_ATTR] === 'pending' &&
      typeof segmentId === 'string' &&
      segmentId.length > 0 &&
      !seen.has(segmentId)
    ) {
      const text = nodeText(node).trim()
      if (text) {
        seen.add(segmentId)
        segments.push({ segmentId, text })
      }
    }
    node.content?.forEach(visit)
  }

  visit(doc)
  return segments
}

export function asrParagraphAttrs(
  segmentId: string,
  state: CleanupState,
): Record<string, string> {
  return {
    [SPEECH_SEGMENT_ID_ATTR]: segmentId,
    [INPUT_SOURCE_ATTR]: ASR_INPUT_SOURCE,
    [CLEANUP_STATE_ATTR]: state,
  }
}

export const SpeechBlockMetadata = Extension.create({
  name: 'speechBlockMetadata',

  addGlobalAttributes() {
    return [
      {
        types: ['paragraph'],
        attributes: {
          [SPEECH_SEGMENT_ID_ATTR]: {
            default: null,
            keepOnSplit: false,
            parseHTML: (element) => element.getAttribute('data-speech-segment-id'),
            renderHTML: (attributes) =>
              typeof attributes[SPEECH_SEGMENT_ID_ATTR] === 'string' &&
              attributes[SPEECH_SEGMENT_ID_ATTR]
                ? { 'data-speech-segment-id': attributes[SPEECH_SEGMENT_ID_ATTR] }
                : {},
          },
          [INPUT_SOURCE_ATTR]: {
            default: null,
            keepOnSplit: false,
            parseHTML: (element) =>
              element.getAttribute('data-input-source') === ASR_INPUT_SOURCE
                ? ASR_INPUT_SOURCE
                : null,
            renderHTML: (attributes) =>
              attributes[INPUT_SOURCE_ATTR] === ASR_INPUT_SOURCE
                ? { 'data-input-source': ASR_INPUT_SOURCE }
                : {},
          },
          [CLEANUP_STATE_ATTR]: {
            default: null,
            keepOnSplit: false,
            parseHTML: (element) => cleanupState(element.getAttribute('data-cleanup-state')),
            renderHTML: (attributes) => {
              const state = cleanupState(attributes[CLEANUP_STATE_ATTR])
              return state ? { 'data-cleanup-state': state } : {}
            },
          },
        },
      },
    ]
  },
})

function tidyingDecorations(
  doc: Parameters<typeof DecorationSet.create>[0],
  segmentIds: ReadonlySet<string>,
): DecorationSet {
  const decorations: Decoration[] = []
  doc.descendants((node, position) => {
    if (
      node.type.name === 'paragraph' &&
      typeof node.attrs[SPEECH_SEGMENT_ID_ATTR] === 'string' &&
      segmentIds.has(node.attrs[SPEECH_SEGMENT_ID_ATTR])
    ) {
      decorations.push(
        Decoration.node(position, position + node.nodeSize, {
          class: 'speech-segment-tidying',
          'data-tidying': 'true',
        }),
      )
    }
  })
  return DecorationSet.create(doc, decorations)
}

/** Transient view-only state: it never changes or persists the draft document. */
export const SpeechTidyingDecorations = Extension.create({
  name: 'speechTidyingDecorations',

  addProseMirrorPlugins() {
    return [
      new Plugin<SpeechTidyingDecorationState>({
        key: SPEECH_TIDYING_PLUGIN_KEY,
        state: {
          init: (_, state) => ({
            segmentIds: new Set(),
            decorations: DecorationSet.empty,
          }),
          apply: (transaction, previous) => {
            const meta = transaction.getMeta(SPEECH_TIDYING_PLUGIN_KEY)
            const segmentIds = Array.isArray(meta)
              ? new Set(meta.filter((value): value is string => typeof value === 'string'))
              : previous.segmentIds
            if (!Array.isArray(meta) && !transaction.docChanged) return previous
            return {
              segmentIds,
              decorations: tidyingDecorations(transaction.doc, segmentIds),
            }
          },
        },
        props: {
          decorations: (state) =>
            SPEECH_TIDYING_PLUGIN_KEY.getState(state)?.decorations ?? null,
        },
      }),
    ]
  },
})

export function setTidyingSpeechSegments(editor: Editor, segmentIds: readonly string[]) {
  editor.view.dispatch(
    editor.state.tr.setMeta(SPEECH_TIDYING_PLUGIN_KEY, [...new Set(segmentIds)]),
  )
}
