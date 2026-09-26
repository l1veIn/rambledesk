import { Extension, type Editor } from '@tiptap/core'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

export type FieldSpeechDecoration = {
  segmentId: string
  /** ProseMirror positions, after projecting the field's hidden attachment references. */
  from: number
  to: number
  state?: 'pending' | 'cleaned'
}
type SpeechDecorationState = { decorations: DecorationSet }
export const FIELD_SPEECH_PLUGIN_KEY = new PluginKey<SpeechDecorationState>('fieldSpeechOrigins')

function decorationsFor(document: ProseMirrorNode, ranges: readonly FieldSpeechDecoration[], tidyingIds: readonly string[]): DecorationSet {
  const tidying = new Set(tidyingIds)
  return DecorationSet.create(document, ranges.flatMap((range) => {
    if (range.state === 'cleaned' || range.from < 1 || range.to <= range.from || range.to >= document.content.size) return []
    const busy = tidying.has(range.segmentId)
    return [Decoration.widget(range.from, (view) => {
      const marker = view.dom.ownerDocument.createElement('span')
      marker.className = `speech-origin-marker${busy ? ' speech-segment-tidying' : ''}`
      marker.dataset.speechSegmentId = range.segmentId
      marker.dataset.cleanupState = 'pending'
      if (busy) marker.dataset.tidying = 'true'
      marker.setAttribute('aria-hidden', 'true')
      marker.setAttribute('contenteditable', 'false')
      return marker
    }, { side: -1, key: `${range.segmentId}:${busy ? 'tidying' : 'pending'}` })]
  }))
}

/** View-only provenance; microphone widgets never enter field text or its clipboard serialization. */
export const FieldSpeechDecorations = Extension.create({
  name: 'fieldSpeechDecorations',
  addProseMirrorPlugins() {
    return [new Plugin<SpeechDecorationState>({
      key: FIELD_SPEECH_PLUGIN_KEY,
      state: {
        init: () => ({ decorations: DecorationSet.empty }),
        apply(transaction, previous) {
          const update = transaction.getMeta(FIELD_SPEECH_PLUGIN_KEY) as {
            ranges: readonly FieldSpeechDecoration[]; tidyingIds: readonly string[]
          } | undefined
          return { decorations: update
            ? decorationsFor(transaction.doc, update.ranges, update.tidyingIds)
            : previous.decorations.map(transaction.mapping, transaction.doc) }
        },
      },
      props: { decorations: (state) => FIELD_SPEECH_PLUGIN_KEY.getState(state)?.decorations ?? DecorationSet.empty },
    })]
  },
})

export function setFieldSpeechSegments(editor: Editor, ranges: readonly FieldSpeechDecoration[], tidyingIds: readonly string[] = []): void {
  if (editor.isDestroyed || !FIELD_SPEECH_PLUGIN_KEY.getState(editor.state)) return
  editor.view.dispatch(editor.state.tr.setMeta(FIELD_SPEECH_PLUGIN_KEY, { ranges, tidyingIds }).setMeta('addToHistory', false))
}
