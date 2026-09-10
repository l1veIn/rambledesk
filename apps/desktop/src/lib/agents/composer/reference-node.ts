// Adapted from Codeg composer/nodes/reference-node.ts and reference-text.ts at 3ebdfed.
// SPDX-License-Identifier: Apache-2.0
// RambleDesk: native DOM rendering, only host-supplied file references, no React or Codeg URI routes.
import { Node, mergeAttributes } from '@tiptap/core'
import type { ComposerReference } from './types'

export function referenceToMarkdown(reference: ComposerReference): string {
  const label = (reference.label || reference.id).replace(/\s*[\r\n]+\s*/g, ' ').replace(/[\\`*_~[\]()<>]/g, '\\$&')
  const uri = reference.uri.replace(/[\r\n]+/g, '')
  if (!uri.toLowerCase().startsWith('file:')) return label
  const destination = /[\s()<>\\]/.test(uri) ? `<${uri.replace(/[\\<>]/g, '\\$&')}>` : uri
  return `[${label}](${destination})`
}

export const Reference = Node.create({
  name: 'reference', group: 'inline', inline: true, atom: true, selectable: true,
  addAttributes() {
    return {
      id: { default: '', parseHTML: (el) => el.getAttribute('data-ref-id') ?? '', renderHTML: (attrs) => ({ 'data-ref-id': attrs.id }) },
      label: { default: '', parseHTML: (el) => el.getAttribute('data-label') ?? '', renderHTML: (attrs) => ({ 'data-label': attrs.label }) },
      uri: {
        default: '',
        parseHTML: (el) => { const uri = el.getAttribute('data-uri') ?? ''; return uri.toLowerCase().startsWith('file:') ? uri : '' },
        renderHTML: (attrs) => ({ 'data-uri': attrs.uri }),
      },
    }
  },
  parseHTML: () => [{ tag: 'span[data-ramble-reference]' }],
  renderHTML: ({ node, HTMLAttributes }) => ['span', mergeAttributes(HTMLAttributes, {
    'data-ramble-reference': '', class: 'ramble-composer-reference', contenteditable: 'false', title: node.attrs.uri,
  }), node.attrs.label || node.attrs.id],
  renderText: ({ node }) => referenceToMarkdown(node.attrs as ComposerReference),
})
