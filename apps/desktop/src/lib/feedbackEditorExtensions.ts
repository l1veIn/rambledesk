import { Node, type AnyExtension } from '@tiptap/core'
import Image from '@tiptap/extension-image'
import { TableKit } from '@tiptap/extension-table'
import TaskItem from '@tiptap/extension-task-item'
import TaskList from '@tiptap/extension-task-list'
import { Markdown } from '@tiptap/markdown'
import StarterKit from '@tiptap/starter-kit'

import { attachmentIdFromUrl, attachmentMarkdownUrl } from './attachmentMarkdown'

const AttachmentImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      attachmentId: {
        default: null,
        parseHTML: (element) => element.getAttribute('data-attachment-id'),
        renderHTML: (attributes) =>
          attributes.attachmentId
            ? { 'data-attachment-id': attributes.attachmentId }
            : {},
      },
    }
  },

  renderMarkdown: (node) => {
    const attachmentId =
      node.attrs?.attachmentId ?? attachmentIdFromUrl(node.attrs?.src)
    const src = attachmentId
      ? attachmentMarkdownUrl(attachmentId)
      : (node.attrs?.src ?? '')
    const alt = node.attrs?.alt ?? ''
    const title = node.attrs?.title ?? ''
    return title ? `![${alt}](${src} "${title}")` : `![${alt}](${src})`
  },
})

// A clickable chip for a non-image attachment, serialized as a markdown
// link `[fileName](attachment://id)`. A custom marked tokenizer claims
// `attachment://` links before StarterKit's built-in Link mark so they
// parse back into this node instead of a link mark.
const AttachmentFile = Node.create({
  name: 'attachmentFile',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      attachmentId: {
        default: null,
        parseHTML: (element) => element.getAttribute('data-attachment-id'),
        renderHTML: (attributes) =>
          attributes.attachmentId
            ? { 'data-attachment-id': attributes.attachmentId }
            : {},
      },
      fileName: {
        default: '',
        parseHTML: (element) => element.getAttribute('data-file-name'),
        renderHTML: (attributes) =>
          attributes.fileName ? { 'data-file-name': attributes.fileName } : {},
      },
      mediaType: {
        default: '',
        parseHTML: (element) => element.getAttribute('data-media-type'),
        renderHTML: (attributes) =>
          attributes.mediaType ? { 'data-media-type': attributes.mediaType } : {},
      },
    }
  },

  markdownTokenName: 'attachmentFile',
  markdownTokenizer: {
    name: 'attachmentFile',
    level: 'inline',
    start: (src) => src.indexOf('['),
    tokenize: (src) => {
      const match = /^\[([^\]]*)\]\(attachment:\/\/([0-9a-fA-F-]+)\)/.exec(src)
      if (!match) return undefined
      return { type: 'attachmentFile', raw: match[0], text: match[1], attachmentId: match[2] }
    },
  },
  parseMarkdown: (token, helpers) =>
    helpers.createNode('attachmentFile', {
      attachmentId: token.attachmentId,
      fileName: token.text || token.attachmentId,
      mediaType: '',
    }),

  renderHTML({ node }) {
    const attachmentId = node.attrs.attachmentId ?? ''
    const fileName = node.attrs.fileName || attachmentId || 'attachment'
    const ext = (fileName.split('.').pop() || 'FILE').toUpperCase().slice(0, 4)
    return [
      'a',
      {
        href: attachmentMarkdownUrl(attachmentId),
        'data-attachment-id': attachmentId,
        'data-file-name': fileName,
        'data-media-type': node.attrs.mediaType ?? '',
        class: 'attachment-file-chip',
        contenteditable: 'false',
      },
      ['span', { class: 'attachment-file-chip-ext' }, ext],
      ['span', { class: 'attachment-file-chip-label' }, fileName],
    ]
  },

  parseHTML() {
    return [
      {
        tag: 'a[href^="attachment://"]',
        getAttrs: (element) => {
          const href = element.getAttribute('href') ?? ''
          return {
            attachmentId:
              element.getAttribute('data-attachment-id') ??
              attachmentIdFromUrl(href) ??
              null,
            fileName:
              element.getAttribute('data-file-name') ??
              element.textContent?.trim() ??
              '',
            mediaType: element.getAttribute('data-media-type') ?? '',
          }
        },
      },
    ]
  },

  renderMarkdown: (node) => {
    const url = attachmentMarkdownUrl(node.attrs?.attachmentId ?? '')
    const label = (node.attrs?.fileName || node.attrs?.attachmentId || '').replace(
      /([\[\]])/g,
      '\\$1',
    )
    return `[${label}](${url})`
  },
})

/**
 * Extensions behind the rich feedback editor.
 *
 * A human can ramble in tables and checklists, and a host can hand a cooked
 * draft back with either. Tables and task items only survive the markdown
 * round trip when the schema knows those nodes: without them the parser drops
 * a whole table on the floor and flattens `- [ ]` into a plain bullet. Keep
 * this in step with `workbench/MarkdownPreview.svelte`, which renders the same
 * markdown read-only.
 */
export function feedbackEditorExtensions(): AnyExtension[] {
  return [
    StarterKit.configure({
      heading: { levels: [2, 3] },
    }),
    TableKit,
    TaskList,
    TaskItem.configure({ nested: true }),
    AttachmentImage,
    AttachmentFile,
    Markdown,
  ]
}
