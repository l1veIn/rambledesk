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
            const decorations: Decoration[] = []
            let previous: number | null = null
            state.doc.forEach((node, pos) => {
              const index =
                typeof node.attrs?.[ACTION_CHANNEL_ATTR] === 'number'
                  ? node.attrs[ACTION_CHANNEL_ATTR]
                  : null
              if (index != null && index !== previous) {
                decorations.push(
                  Decoration.node(pos, pos + node.nodeSize, { class: 'action-channel-lead' }),
                )
              }
              previous = index
            })
            return DecorationSet.create(state.doc, decorations)
          },
        },
      }),
    ]
  },
})
