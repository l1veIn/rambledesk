<script lang="ts">
  import { Editor } from '@tiptap/core'
  import Image from '@tiptap/extension-image'
  import { Markdown } from '@tiptap/markdown'
  import StarterKit from '@tiptap/starter-kit'
  import { onMount } from 'svelte'

  import type { AttachmentView } from './feedback'
  import {
    attachmentIdFromUrl,
    attachmentMarkdownUrl,
  } from './attachmentMarkdown'

  export let markdown = ''
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let onChange: (markdown: string) => void = () => {}

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let applyingExternalChange = false
  let editorMarkdown = ''
  let insertionPosition = 0

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

  onMount(() => {
    editor = new Editor({
      element: editorHost,
      extensions: [
        StarterKit.configure({
          heading: { levels: [2, 3] },
        }),
        AttachmentImage,
        Markdown,
      ],
      content: markdown,
      contentType: 'markdown',
      editable: !disabled,
      editorProps: {
        attributes: {
          class: 'feedback-prose',
          'aria-label': 'Markdown 富文本反馈正文',
          'data-placeholder': '记录你看见了什么、哪里顺畅、哪里让你停顿。',
        },
      },
      onCreate: () => {
        editorMarkdown = editor?.getMarkdown() ?? markdown
        insertionPosition = editor?.state.doc.content.size ?? 0
        hydrateAttachmentImages()
      },
      onUpdate: ({ editor: updatedEditor }) => {
        if (applyingExternalChange) return
        const nextMarkdown = updatedEditor.getMarkdown()
        editorMarkdown = nextMarkdown
        onChange(nextMarkdown)
      },
      onSelectionUpdate: ({ editor: updatedEditor }) => {
        insertionPosition = updatedEditor.state.selection.from
      },
    })

    return () => {
      editor?.destroy()
      editor = null
    }
  })

  $: if (editor) editor.setEditable(!disabled)
  $: if (editor && markdown !== editorMarkdown) applyMarkdown(markdown)
  $: if (editor) {
    previews
    hydrateAttachmentImages()
  }

  function applyMarkdown(nextMarkdown: string) {
    if (!editor) return
    applyingExternalChange = true
    editor.commands.setContent(nextMarkdown, {
      contentType: 'markdown',
      emitUpdate: false,
    })
    editorMarkdown = nextMarkdown
    insertionPosition = Math.min(insertionPosition, editor.state.doc.content.size)
    hydrateAttachmentImages()
    applyingExternalChange = false
  }

  function hydrateAttachmentImages() {
    if (!editor) return
    let transaction = editor.state.tr
    let changed = false
    editor.state.doc.descendants((node, position) => {
      if (node.type.name !== 'image') return
      const attachmentId =
        node.attrs.attachmentId ?? attachmentIdFromUrl(node.attrs.src)
      if (!attachmentId) return
      const preview = previews[attachmentId]
      if (!preview || (node.attrs.attachmentId === attachmentId && node.attrs.src === preview)) {
        return
      }
      transaction = transaction.setNodeMarkup(position, undefined, {
        ...node.attrs,
        attachmentId,
        src: preview,
      })
      changed = true
    })
    if (!changed) return
    applyingExternalChange = true
    editor.view.dispatch(transaction)
    editorMarkdown = editor.getMarkdown()
    applyingExternalChange = false
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    if (!editor || attachments.length === 0) return false
    const referencedIds = new Set<string>()
    editor.state.doc.descendants((node) => {
      if (node.type.name !== 'image') return
      const attachmentId =
        node.attrs.attachmentId ?? attachmentIdFromUrl(node.attrs.src)
      if (attachmentId) referencedIds.add(attachmentId)
    })
    const content = attachments
      .filter((attachment) => !referencedIds.has(attachment.attachment_id))
      .flatMap((attachment) => [
        {
          type: 'image',
          attrs: {
            src:
              previews[attachment.attachment_id] ??
              attachmentMarkdownUrl(attachment.attachment_id),
            alt: attachment.file_name,
            attachmentId: attachment.attachment_id,
          },
        },
        { type: 'paragraph' },
      ])
    if (content.length === 0) return false
    const position = Math.min(
      Math.max(insertionPosition, 0),
      editor.state.doc.content.size,
    )
    const inserted = editor.commands.insertContentAt(position, content)
    if (inserted) insertionPosition = editor.state.selection.from
    return inserted
  }

  export function appendTranscript(text: string) {
    const transcript = text.trim()
    if (!editor || !transcript || disabled) return
    editor.commands.insertContentAt(editor.state.doc.content.size, {
      type: 'paragraph',
      content: [{ type: 'text', text: transcript }],
    })
  }

  export function appendClipboardCapture(text: string, label: string) {
    const captured = text.trim()
    if (!editor || !captured || disabled) return false
    const capturedContent = captured.split(/\r?\n/).flatMap((line, index) => {
      const content: Array<Record<string, unknown>> = []
      if (index > 0) content.push({ type: 'hardBreak' })
      if (line) content.push({ type: 'text', text: line })
      return content
    })
    return editor.commands.insertContentAt(editor.state.doc.content.size, [
      {
        type: 'blockquote',
        content: [
          {
            type: 'paragraph',
            content: [
              {
                type: 'text',
                text: label,
                marks: [{ type: 'bold' }],
              },
            ],
          },
          {
            type: 'paragraph',
            content: capturedContent,
          },
        ],
      },
      { type: 'paragraph' },
    ])
  }

  export function appendCapturedAttachment(
    attachment: AttachmentView,
    label: string,
  ) {
    if (!editor || disabled) return false
    return editor.commands.insertContentAt(editor.state.doc.content.size, [
      {
        type: 'blockquote',
        content: [
          {
            type: 'paragraph',
            content: [
              {
                type: 'text',
                text: label,
                marks: [{ type: 'bold' }],
              },
            ],
          },
        ],
      },
      {
        type: 'image',
        attrs: {
          src:
            previews[attachment.attachment_id] ??
            attachmentMarkdownUrl(attachment.attachment_id),
          alt: attachment.file_name,
          attachmentId: attachment.attachment_id,
        },
      },
      { type: 'paragraph' },
    ])
  }

  export function removeAttachmentReference(attachmentId: string) {
    if (!editor) return
    const ranges: Array<{ from: number; to: number }> = []
    editor.state.doc.descendants((node, position) => {
      if (
        node.type.name === 'image' &&
        (node.attrs.attachmentId ?? attachmentIdFromUrl(node.attrs.src)) ===
          attachmentId
      ) {
        ranges.push({ from: position, to: position + node.nodeSize })
      }
    })
    if (ranges.length === 0) return
    const transaction = ranges
      .reverse()
      .reduce(
        (next, range) => next.delete(range.from, range.to),
        editor.state.tr,
      )
    editor.view.dispatch(transaction)
  }
</script>

<div class="rich-editor">
  <div class="format-toolbar" aria-label="正文格式">
    <button
      type="button"
      aria-label="加粗"
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBold().run()}
    >B</button>
    <button
      type="button"
      aria-label="斜体"
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleItalic().run()}
    ><i>I</i></button>
    <button
      type="button"
      aria-label="二级标题"
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
    >H2</button>
    <button
      type="button"
      aria-label="无序列表"
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
    >• 列表</button>
    <button
      type="button"
      aria-label="引用"
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBlockquote().run()}
    >“ 引用</button>
    <span></span>
    <button
      type="button"
      aria-label="撤销"
      disabled={disabled || !editor?.can().undo()}
      onclick={() => editor?.chain().focus().undo().run()}
    >↶</button>
    <button
      type="button"
      aria-label="重做"
      disabled={disabled || !editor?.can().redo()}
      onclick={() => editor?.chain().focus().redo().run()}
    >↷</button>
  </div>
  <div class="editor-host" bind:this={editorHost}></div>
</div>

<style>
  .rich-editor {
    overflow: hidden;
    border: 1px solid #d9d4c8;
    border-radius: 14px;
    background: #fffefa;
  }

  .format-toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid #e8e3d8;
    background: #f7f4ed;
  }

  .format-toolbar span {
    flex: 1;
  }

  .format-toolbar button {
    min-width: 32px;
    height: 30px;
    padding: 0 9px;
    border: 1px solid transparent;
    border-radius: 7px;
    color: #4c493f;
    background: transparent;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .format-toolbar button:hover:not(:disabled) {
    border-color: #d7d0c2;
    background: #fffefa;
  }

  .format-toolbar button:disabled {
    cursor: default;
    opacity: 0.38;
  }

  .editor-host :global(.feedback-prose) {
    min-height: 280px;
    padding: 22px;
    color: #292821;
    font-size: 15px;
    line-height: 1.75;
    outline: none;
  }

  .editor-host :global(.feedback-prose:empty::before) {
    float: left;
    height: 0;
    color: #a29d92;
    content: attr(data-placeholder);
    pointer-events: none;
  }

  .editor-host :global(.feedback-prose > *:first-child) {
    margin-top: 0;
  }

  .editor-host :global(.feedback-prose p) {
    margin: 0 0 0.9em;
  }

  .editor-host :global(.feedback-prose h2),
  .editor-host :global(.feedback-prose h3) {
    margin: 1.4em 0 0.55em;
    line-height: 1.3;
  }

  .editor-host :global(.feedback-prose blockquote) {
    margin: 1em 0;
    padding-left: 14px;
    border-left: 3px solid #d57045;
    color: #625e54;
  }

  .editor-host :global(.feedback-prose img) {
    display: block;
    width: auto;
    max-width: min(100%, 900px);
    max-height: 620px;
    margin: 18px auto;
    border: 1px solid #ddd7ca;
    border-radius: 12px;
    object-fit: contain;
    background: #f2efe8;
    box-shadow: 0 8px 28px rgb(46 39 26 / 10%);
  }

  .editor-host :global(.feedback-prose.ProseMirror-focused) {
    box-shadow: inset 0 0 0 1px rgb(193 91 50 / 18%);
  }
</style>
