<script lang="ts">
  import { Editor } from '@tiptap/core'
  import Image from '@tiptap/extension-image'
  import { Markdown } from '@tiptap/markdown'
  import StarterKit from '@tiptap/starter-kit'
  import { onMount } from 'svelte'

  import type { AttachmentView } from './feedback'
  import { t } from './i18n'
  import { locale } from './preferences'
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
          'aria-label': t($locale, 'Markdown 富文本反馈正文'),
          'data-placeholder': t($locale, '记录你看见了什么、哪里顺畅、哪里让你停顿。'),
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
  $: if (editor) {
    $locale
    editor.view.dom.setAttribute('aria-label', t($locale, 'Markdown 富文本反馈正文'))
    editor.view.dom.setAttribute('data-placeholder', t($locale, '记录你看见了什么、哪里顺畅、哪里让你停顿。'))
  }
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
  <div class="format-toolbar" aria-label={t($locale, '正文格式')}>
    <button
      type="button"
      aria-label={t($locale, '加粗')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBold().run()}
    >B</button>
    <button
      type="button"
      aria-label={t($locale, '斜体')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleItalic().run()}
    ><i>I</i></button>
    <button
      type="button"
      aria-label={t($locale, '二级标题')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
    >H2</button>
    <button
      type="button"
      aria-label={t($locale, '无序列表')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
    >• {t($locale, '列表')}</button>
    <button
      type="button"
      aria-label={t($locale, '引用')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBlockquote().run()}
    >“ {t($locale, '引用')}</button>
    <span></span>
    <button
      type="button"
      aria-label={t($locale, '撤销')}
      disabled={disabled || !editor?.can().undo()}
      onclick={() => editor?.chain().focus().undo().run()}
    >↶</button>
    <button
      type="button"
      aria-label={t($locale, '重做')}
      disabled={disabled || !editor?.can().redo()}
      onclick={() => editor?.chain().focus().redo().run()}
    >↷</button>
  </div>
  <div class="editor-host" bind:this={editorHost}></div>
</div>

<style>
  .rich-editor {
    width: 100%;
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--line, #d4e0ec);
    border-radius: 10px;
    background: var(--editor-paper, #fff);
  }

  .format-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 7px 9px;
    border-bottom: 1px solid var(--line-soft, #e0e8f0);
    background: var(--surface, #f7f9fc);
    overflow-x: auto;
  }

  .format-toolbar span {
    flex: 1;
  }

  .format-toolbar button {
    min-width: 32px;
    height: 28px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--ink-soft, #526a84);
    background: transparent;
    font: inherit;
    font-size: 10px;
    cursor: pointer;
  }

  .format-toolbar button:hover:not(:disabled) {
    border-color: #bed4e9;
    color: var(--blue-strong, #2775ca);
    background: var(--blue-soft, #eaf3fc);
  }

  .format-toolbar button:disabled {
    cursor: default;
    opacity: 0.38;
  }

  .editor-host :global(.feedback-prose) {
    min-height: clamp(360px, 52vh, 620px);
    padding: clamp(20px, 2.5vw, 34px);
    color: var(--ink, #263a50);
    font-family: Georgia, "Noto Serif SC", "Songti SC", serif;
    font-size: 14px;
    line-height: 1.78;
    outline: none;
  }

  .editor-host :global(.feedback-prose:empty::before) {
    float: left;
    height: 0;
    color: var(--ink-faint, #9aa8b7);
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
    color: var(--ink, #203550);
    font-family: "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", sans-serif;
    line-height: 1.3;
  }

  .editor-host :global(.feedback-prose blockquote) {
    margin: 1em 0;
    padding-left: 14px;
    padding: 10px 14px;
    border-left: 3px solid #6fa8dc;
    border-radius: 0 8px 8px 0;
    color: var(--ink-soft, #52677e);
    background: var(--surface-tint, #f3f7fb);
  }

  .editor-host :global(.feedback-prose img) {
    display: block;
    width: auto;
    max-width: min(100%, 900px);
    max-height: 620px;
    margin: 18px auto;
    border: 1px solid #ccdae7;
    border-radius: 10px;
    object-fit: contain;
    background: var(--surface-tint, #eef3f7);
    box-shadow: 0 10px 30px rgb(40 72 106 / 10%);
  }

  .editor-host :global(.feedback-prose.ProseMirror-focused) {
    box-shadow: inset 0 0 0 1px rgb(79 143 211 / 18%);
  }
</style>
