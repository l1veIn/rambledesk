<script lang="ts">
  import { Editor } from '@tiptap/core'
  import Image from '@tiptap/extension-image'
  import { Markdown } from '@tiptap/markdown'
  import StarterKit from '@tiptap/starter-kit'
  import {
    Bold,
    Heading2,
    Italic,
    List,
    Quote,
    Redo2,
    Undo2,
  } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { Button } from '$lib/components/ui/button'
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

<div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border bg-background">
  <div class="flex h-10 shrink-0 items-center gap-1 overflow-x-auto border-b bg-muted/30 px-2" aria-label={t($locale, '正文格式')}>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '加粗')}
      title={t($locale, '加粗')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBold().run()}
    >
      <Bold />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '斜体')}
      title={t($locale, '斜体')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleItalic().run()}
    >
      <Italic />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '二级标题')}
      title={t($locale, '二级标题')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
    >
      <Heading2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '无序列表')}
      title={t($locale, '无序列表')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
    >
      <List />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '引用')}
      title={t($locale, '引用')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBlockquote().run()}
    >
      <Quote />
    </Button>
    <span class="flex-1"></span>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '撤销')}
      title={t($locale, '撤销')}
      disabled={disabled || !editor?.can().undo()}
      onclick={() => editor?.chain().focus().undo().run()}
    >
      <Undo2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, '重做')}
      title={t($locale, '重做')}
      disabled={disabled || !editor?.can().redo()}
      onclick={() => editor?.chain().focus().redo().run()}
    >
      <Redo2 />
    </Button>
  </div>
  <div class="editor-host min-h-0 flex-1 overflow-y-auto overscroll-contain" bind:this={editorHost}></div>
</div>

<style>
  .editor-host :global(.feedback-prose) {
    min-height: 100%;
    padding: clamp(20px, 2.5vw, 34px);
    color: var(--foreground);
    font-family: ui-serif, Georgia, "Noto Serif SC", "Songti SC", serif;
    font-size: 14px;
    line-height: 1.78;
    outline: none;
  }

  .editor-host :global(.feedback-prose:empty::before) {
    float: left;
    height: 0;
    color: var(--muted-foreground);
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
    color: var(--foreground);
    font-family: ui-sans-serif, system-ui, sans-serif;
    line-height: 1.3;
  }

  .editor-host :global(.feedback-prose blockquote) {
    margin: 1em 0;
    padding: 10px 14px;
    border-left: 3px solid var(--primary);
    color: var(--muted-foreground);
    background: color-mix(in oklab, var(--muted) 65%, transparent);
  }

  .editor-host :global(.feedback-prose img) {
    display: block;
    width: auto;
    max-width: min(100%, 900px);
    max-height: 620px;
    margin: 18px auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    object-fit: contain;
    background: var(--muted);
  }

  .editor-host :global(.feedback-prose.ProseMirror-focused) {
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--ring) 30%, transparent);
  }
</style>
