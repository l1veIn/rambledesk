// Adapted from Codeg src/components/chat/composer/editor-config.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: uses installed core/pm/StarterKit; a local placeholder avoids a new package.
import { Extension, type Extensions } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { Plugin } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'
import { InactiveSelectionHighlight } from './inactive-selection'
import { QuoteLineDecoration } from './quote-decoration'
import { Reference } from './reference-node'

export function buildComposerExtensions(options: { placeholder?: () => string } = {}): Extensions {
  return [
    StarterKit.configure({
      blockquote: false, bold: false, bulletList: false, code: false, codeBlock: false,
      heading: false, horizontalRule: false, italic: false, link: false, listItem: false,
      listKeymap: false, orderedList: false, strike: false, underline: false,
    }),
    Reference, InactiveSelectionHighlight, QuoteLineDecoration,
    Extension.create({
      name: 'composerPlaceholder',
      addProseMirrorPlugins() {
        return [new Plugin({ props: { decorations: (state) => {
          if (!this.editor.isEditable || state.doc.childCount !== 1 || state.doc.firstChild?.content.size !== 0) return null
          return DecorationSet.create(state.doc, [Decoration.node(0, state.doc.firstChild.nodeSize, {
            class: 'ramble-composer-empty', 'data-placeholder': options.placeholder?.() ?? '',
          })])
        } } })]
      },
    }),
  ]
}
