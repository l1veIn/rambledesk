import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

import { ACTION_CHANNEL_ATTR } from './workbench/actionChannel'

declare module '@tiptap/core' {
  interface Storage {
    actionChannel: {
      currentIndex: number | null
    }
  }
}

export const ACTION_CHANNEL_NODES = [
  'paragraph',
  'heading',
  'blockquote',
  'image',
  'bulletList',
  'orderedList',
  'taskList',
  'codeBlock',
  'table',
  'horizontalRule',
] as const

export const ActionChannel = Extension.create({
  name: 'actionChannel',

  addStorage() {
    return { currentIndex: null as number | null }
  },

  addGlobalAttributes() {
    return [
      {
        types: [...ACTION_CHANNEL_NODES],
        attributes: {
          [ACTION_CHANNEL_ATTR]: {
            default: null,
            keepOnSplit: true,
            parseHTML: (element) => {
              const raw = element.getAttribute('data-action-index')
              if (!raw) return null
              const index = Number(raw)
              return Number.isInteger(index) && index > 0 ? index : null
            },
            renderHTML: (attributes) => {
              const index = attributes[ACTION_CHANNEL_ATTR]
              if (typeof index !== 'number' || index < 1) return {}
              return { 'data-action-index': String(index) }
            },
          },
        },
      },
    ]
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey('actionChannelLead'),
        props: {
          decorations(state) {
            // Group consecutive nodes that share one channel into a single
            // visual block: the first node of a group carries the lead marker,
            // every node is an `action-channel-item`, and CSS merges the items
            // into one continuous background.
            const nodes: { index: number; from: number; to: number }[] = []
            state.doc.forEach((node, pos) => {
              const index =
                typeof node.attrs?.[ACTION_CHANNEL_ATTR] === 'number'
                  ? node.attrs[ACTION_CHANNEL_ATTR]
                  : null
              if (index != null) {
                nodes.push({ index, from: pos, to: pos + node.nodeSize })
              }
            })
            const decorations: Decoration[] = []
            let i = 0
            while (i < nodes.length) {
              const groupIndex = nodes[i]!.index
              let j = i
              while (j + 1 < nodes.length && nodes[j + 1]?.index === groupIndex) j++
              for (let k = i; k <= j; k++) {
                const node = nodes[k]!
                const classes = ['action-channel-item']
                if (k === i) {
                  classes.push('action-channel-lead')
                  if (k === j) classes.push('action-channel-group-solo')
                } else if (k === j) {
                  classes.push('action-channel-group-end')
                } else {
                  classes.push('action-channel-group-mid')
                }
                decorations.push(
                  Decoration.node(node.from, node.to, { class: classes.join(' ') }),
                )
              }
              i = j + 1
            }
            return DecorationSet.create(state.doc, decorations)
          },
        },
      }),
    ]
  },
})
