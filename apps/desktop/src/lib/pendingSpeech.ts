import { Node, mergeAttributes } from '@tiptap/core'
import { Plugin } from '@tiptap/pm/state'
import { ReplaceStep } from '@tiptap/pm/transform'

export const PENDING_SPEECH_NODE = 'pendingSpeech'

function nodeText(node: { text?: string | null; content?: readonly unknown[] | null }): string {
  if (node.text) return node.text
  return (node.content ?? []).map((child) => nodeText(child as never)).join('')
}

function rangesOverlap(leftFrom: number, leftTo: number, rightFrom: number, rightTo: number) {
  return leftFrom < rightTo && rightFrom < leftTo
}

export const PendingSpeech = Node.create({
  name: PENDING_SPEECH_NODE,
  group: 'block',
  content: 'inline*',
  defining: true,

  addAttributes() {
    return {
      status: {
        default: 'pending',
        parseHTML: (element) => element.getAttribute('data-speech-status') || 'pending',
        renderHTML: (attributes) =>
          attributes.status ? { 'data-speech-status': attributes.status } : {},
      },
    }
  },

  parseHTML() {
    return [{ tag: `p[data-speech-status]` }]
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      'p',
      mergeAttributes(HTMLAttributes, {
        'data-speech-status': node.attrs.status,
        class:
          node.attrs.status === 'cleaning' ? 'speech-cleaning' : 'speech-pending',
        contenteditable: node.attrs.status === 'cleaning' ? 'false' : null,
        'data-speech-hint': node.attrs.status === 'cleaning' ? '整理中' : null,
      }),
      0,
    ]
  },

  renderMarkdown: (node) => `${nodeText(node).trim()}\n\n`,

  addProseMirrorPlugins() {
    return [
      new Plugin({
        filterTransaction(transaction, state) {
          if (transaction.getMeta('speechCleanup') || !transaction.docChanged) return true
          const cleaning: Array<{ from: number; to: number }> = []
          state.doc.descendants((node, position) => {
            if (node.type.name === PENDING_SPEECH_NODE && node.attrs.status === 'cleaning') {
              cleaning.push({ from: position, to: position + node.nodeSize })
            }
          })
          if (cleaning.length === 0) return true
          return !transaction.steps.some((step) => {
            if (!(step instanceof ReplaceStep)) return false
            return cleaning.some((range) =>
              rangesOverlap(step.from, step.to, range.from, range.to),
            )
          })
        },
      }),
    ]
  },
})
