import { Extension, type JSONContent } from '@tiptap/core'
import { Plugin, PluginKey, type EditorState, type Transaction } from '@tiptap/pm/state'
import { ReplaceStep } from '@tiptap/pm/transform'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

export const SPEECH_SEGMENT_ID_ATTR = 'speechSegmentId'
export const INPUT_SOURCE_ATTR = 'inputSource'
export const CLEANUP_STATE_ATTR = 'cleanupState'
export const ASR_INPUT_SOURCE = 'asr'

export type CleanupState = 'pending' | 'cleaned' | 'failed' | 'skipped'

export type SpeechCleanupSegment = {
  segmentId: string
  text: string
}

type InFlightMeta = {
  segmentIds: string[]
  active: boolean
}

type SpeechBlockPluginState = {
  inFlight: Set<string>
}

export const SPEECH_CLEANUP_TRANSACTION_META = 'speechCleanup'
export const speechBlockPluginKey = new PluginKey<SpeechBlockPluginState>('speechBlockMetadata')

function nodeText(node: JSONContent): string {
  if (typeof node.text === 'string') return node.text
  return (node.content ?? []).map(nodeText).join('')
}

function cleanupState(value: unknown): CleanupState | null {
  return value === 'pending' || value === 'cleaned' || value === 'failed' || value === 'skipped'
    ? value
    : null
}

function presentSegmentIds(state: EditorState): Set<string> {
  const ids = new Set<string>()
  state.doc.descendants((node) => {
    const id = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
    if (typeof id === 'string' && id) ids.add(id)
  })
  return ids
}

function rangesOverlap(leftFrom: number, leftTo: number, rightFrom: number, rightTo: number) {
  return leftFrom < rightTo && rightFrom < leftTo
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

export function setSpeechCleanupInFlight(
  transaction: Transaction,
  segmentIds: string[],
  active: boolean,
): Transaction {
  return transaction.setMeta(speechBlockPluginKey, { segmentIds, active } satisfies InFlightMeta)
}

export function isSpeechCleanupInFlight(state: EditorState, segmentId: string): boolean {
  return speechBlockPluginKey.getState(state)?.inFlight.has(segmentId) === true
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

  addProseMirrorPlugins() {
    return [
      new Plugin<SpeechBlockPluginState>({
        key: speechBlockPluginKey,
        state: {
          init: () => ({ inFlight: new Set() }),
          apply(transaction, previous, _oldState, newState) {
            const meta = transaction.getMeta(speechBlockPluginKey) as InFlightMeta | undefined
            const next = new Set(previous.inFlight)
            if (meta) {
              for (const segmentId of meta.segmentIds) {
                if (meta.active) next.add(segmentId)
                else next.delete(segmentId)
              }
            }
            const present = presentSegmentIds(newState)
            return { inFlight: new Set([...next].filter((segmentId) => present.has(segmentId))) }
          },
        },
        filterTransaction(transaction, state) {
          if (transaction.getMeta(SPEECH_CLEANUP_TRANSACTION_META) || !transaction.docChanged) {
            return true
          }
          const inFlight = speechBlockPluginKey.getState(state)?.inFlight ?? new Set<string>()
          if (inFlight.size === 0) return true
          const locked: Array<{ from: number; to: number }> = []
          state.doc.descendants((node, position) => {
            const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
            if (typeof segmentId === 'string' && inFlight.has(segmentId)) {
              locked.push({ from: position, to: position + node.nodeSize })
            }
          })
          return !transaction.steps.some((step) => {
            if (!(step instanceof ReplaceStep)) return false
            return locked.some((range) => rangesOverlap(step.from, step.to, range.from, range.to))
          })
        },
        props: {
          decorations(state) {
            const inFlight = speechBlockPluginKey.getState(state)?.inFlight ?? new Set<string>()
            const decorations: Decoration[] = []
            state.doc.descendants((node, position) => {
              if (node.type.name !== 'paragraph') return
              const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
              if (typeof segmentId === 'string' && inFlight.has(segmentId)) {
                decorations.push(
                  Decoration.node(position, position + node.nodeSize, {
                    class: 'speech-cleaning',
                    contenteditable: 'false',
                    'data-speech-hint': '整理中',
                  }),
                )
              } else if (node.attrs?.[CLEANUP_STATE_ATTR] === 'cleaned') {
                decorations.push(
                  Decoration.node(position, position + node.nodeSize, {
                    class: 'speech-cleaned',
                    'data-speech-hint': '已整理',
                    title: '已整理',
                  }),
                )
              }
            })
            return DecorationSet.create(state.doc, decorations)
          },
        },
      }),
    ]
  },
})
