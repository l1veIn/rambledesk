<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { Fragment } from '@tiptap/pm/model'
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
    isImageMediaType,
  } from './attachmentMarkdown'
  import { feedbackEditorExtensions } from './feedbackEditorExtensions'
  import { PENDING_SPEECH_NODE } from './pendingSpeech'

  export let markdown = ''
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onChange: (markdown: string) => void = () => {}

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let applyingExternalChange = false
  let editorMarkdown = ''
  let insertionPosition = 0
  let historyLocked = false
  let openAttachmentHandler = (_attachmentId: string) => {}
  $: openAttachmentHandler = onOpenAttachment

  onMount(() => {
    editor = new Editor({
      element: editorHost,
      extensions: feedbackEditorExtensions(),
      content: markdown,
      contentType: 'markdown',
      editable: !disabled,
      editorProps: {
        attributes: {
          class: 'feedback-prose',
          'aria-label': t($locale, 'Markdown rich-text feedback body'),
          'data-placeholder': t($locale, 'Record what you saw, what felt smooth, and where you paused.'),
        },
        handleKeyDown: (_view, event) => {
          if (!historyLocked) return false
          if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
            event.preventDefault()
            return true
          }
          return false
        },
        handleClick: (view, pos, event) => {
          const target = event.target as HTMLElement | null
          const chip = target?.closest?.('a.attachment-file-chip')
          if (!chip) return false
          const attachmentId = chip.getAttribute('data-attachment-id')
          if (!attachmentId) return false
          event.preventDefault()
          event.stopPropagation()
          openAttachmentHandler(attachmentId)
          return true
        },
      },
      onCreate: () => {
        editorMarkdown = editor?.getMarkdown() ?? markdown
        insertionPosition = editor?.state.doc.content.size ?? 0
        hydrateAttachmentImages()
      },
      onUpdate: ({ editor: updatedEditor }) => {
        historyLocked = editorHasCleaningSpeech(updatedEditor)
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
    editor.view.dom.setAttribute('aria-label', t($locale, 'Markdown rich-text feedback body'))
    editor.view.dom.setAttribute('data-placeholder', t($locale, 'Record what you saw, what felt smooth, and where you paused.'))
  }
  $: if (editor && markdown !== editorMarkdown) applyMarkdown(markdown)
  $: if (editor) {
    previews
    hydrateAttachmentImages()
  }

  function applyMarkdown(nextMarkdown: string) {
    if (!editor) return
    applyingExternalChange = true
    try {
      editor.commands.setContent(nextMarkdown, {
        contentType: 'markdown',
        emitUpdate: false,
      })
      editorMarkdown = nextMarkdown
      insertionPosition = Math.min(insertionPosition, editor.state.doc.content.size)
      hydrateAttachmentImages()
    } catch (cause) {
      console.error('[richEditor] applyMarkdown failed', cause)
    } finally {
      applyingExternalChange = false
    }
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

  export function applyExternalMarkdown(nextMarkdown: string): boolean {
    if (!editor) return false
    if (nextMarkdown === editorMarkdown) return true
    applyMarkdown(nextMarkdown)
    return true
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    if (!editor || attachments.length === 0) return false
    const referencedIds = new Set<string>()
    editor.state.doc.descendants((node) => {
      if (node.type.name !== 'image' && node.type.name !== 'attachmentFile') return
      const attachmentId =
        node.attrs.attachmentId ?? attachmentIdFromUrl(node.attrs.src)
      if (attachmentId) referencedIds.add(attachmentId)
    })
    const content = attachments
      .filter((attachment) => !referencedIds.has(attachment.attachment_id))
      .flatMap((attachment) => {
        if (isImageMediaType(attachment.media_type)) {
          return [
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
          ]
        }
        return [
          {
            type: 'paragraph',
            content: [
              {
                type: 'attachmentFile',
                attrs: {
                  attachmentId: attachment.attachment_id,
                  fileName: attachment.file_name,
                  mediaType: attachment.media_type,
                },
              },
            ],
          },
          { type: 'paragraph' },
        ]
      })
    if (content.length === 0) return false
    const position = Math.min(
      Math.max(insertionPosition, 0),
      editor.state.doc.content.size,
    )
    const inserted = editor.commands.insertContentAt(position, content)
    if (inserted) insertionPosition = editor.state.selection.from
    return inserted
  }

  function editorHasCleaningSpeech(target = editor) {
    if (!target) return false
    let cleaning = false
    target.state.doc.descendants((node) => {
      if (node.type.name === PENDING_SPEECH_NODE && node.attrs.status === 'cleaning') {
        cleaning = true
      }
    })
    return cleaning
  }

  export function appendTranscript(text: string, options?: { pending?: boolean }) {
    const transcript = text.trim()
    if (!editor || !transcript || disabled) return
    editor.commands.insertContentAt(editor.state.doc.content.size, {
      type: options?.pending ? PENDING_SPEECH_NODE : 'paragraph',
      attrs: options?.pending ? { status: 'pending' } : undefined,
      content: [{ type: 'text', text: transcript }],
    })
  }

  export function isSpeechCleaning() {
    return editorHasCleaningSpeech()
  }

  export function beginSpeechCleanup(): string {
    if (!editor) return ''
    const ranges: Array<{ from: number; text: string }> = []
    editor.state.doc.descendants((node, position) => {
      if (node.type.name === PENDING_SPEECH_NODE && node.attrs.status === 'pending') {
        ranges.push({ from: position, text: node.textContent })
      }
    })
    if (ranges.length === 0) return ''
    let transaction = editor.state.tr
    for (const range of [...ranges].reverse()) {
      const node = transaction.doc.nodeAt(range.from)
      if (!node) continue
      transaction = transaction.setNodeMarkup(range.from, undefined, {
        ...node.attrs,
        status: 'cleaning',
      })
    }
    transaction.setMeta('speechCleanup', true)
    editor.view.dispatch(transaction)
    historyLocked = true
    return ranges.map((range) => range.text).join('\n\n')
  }

  export function finishSpeechCleanup(cleaned: string | null): void {
    if (!editor) return
    const ranges: Array<{ from: number; to: number; text: string }> = []
    editor.state.doc.descendants((node, position) => {
      if (node.type.name === PENDING_SPEECH_NODE && node.attrs.status === 'cleaning') {
        ranges.push({ from: position, to: position + node.nodeSize, text: node.textContent })
      }
    })
    historyLocked = false
    if (ranges.length === 0) return
    const from = ranges[0].from
    const to = ranges[ranges.length - 1].to
    const replacement = (cleaned?.trim() || ranges.map((range) => range.text).join('\n\n'))
      .split(/\n{2,}/)
      .map((paragraph) =>
        editor!.schema.nodes.paragraph.create(
          null,
          paragraph ? editor!.schema.text(paragraph) : undefined,
        ),
      )
    const transaction = editor.state.tr
      .replaceWith(from, to, Fragment.from(replacement))
      .setMeta('speechCleanup', true)
    editor.view.dispatch(transaction)
  }

  export function moveCursorAfterCleaningSpeech() {
    if (!editor) return
    let end = 0
    editor.state.doc.descendants((node, position) => {
      if (node.type.name === PENDING_SPEECH_NODE && node.attrs.status === 'cleaning') {
        end = position + node.nodeSize
      }
    })
    if (end > 0 && insertionPosition < end) insertionPosition = end
  }

  export function insertQuotedBlock(lines: string[]) {
    if (!editor || disabled || lines.length === 0) return false
    const content = lines.map((line) => ({
      type: 'paragraph' as const,
      content: line ? [{ type: 'text' as const, text: line }] : [],
    }))
    const position = Math.min(Math.max(insertionPosition, 0), editor.state.doc.content.size)
    const inserted = editor.commands.insertContentAt(position, [
      { type: 'blockquote', content },
      { type: 'paragraph' },
    ])
    if (inserted) insertionPosition = editor.state.selection.from
    return inserted
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
      } else if (
        node.type.name === 'attachmentFile' &&
        node.attrs.attachmentId === attachmentId
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
  <div class="flex h-10 shrink-0 items-center gap-1 overflow-x-auto border-b bg-muted/30 px-2" aria-label={t($locale, 'Document formatting')}>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Bold')}
      title={t($locale, 'Bold')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBold().run()}
    >
      <Bold />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Italic')}
      title={t($locale, 'Italic')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleItalic().run()}
    >
      <Italic />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Heading 2')}
      title={t($locale, 'Heading 2')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
    >
      <Heading2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Bullet list')}
      title={t($locale, 'Bullet list')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
    >
      <List />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Quote')}
      title={t($locale, 'Quote')}
      disabled={disabled}
      onclick={() => editor?.chain().focus().toggleBlockquote().run()}
    >
      <Quote />
    </Button>
    <span class="flex-1"></span>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Undo')}
      title={t($locale, 'Undo')}
      disabled={disabled || historyLocked || !editor?.can().undo()}
      onclick={() => editor?.chain().focus().undo().run()}
    >
      <Undo2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Redo')}
      title={t($locale, 'Redo')}
      disabled={disabled || historyLocked || !editor?.can().redo()}
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

  .editor-host :global(.feedback-prose p.speech-pending) {
    border-radius: 4px;
  }

  .editor-host :global(.feedback-prose p.speech-cleaning) {
    border-radius: 4px;
    background: color-mix(in srgb, var(--muted) 80%, transparent);
    opacity: 0.78;
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

  /* The table node view always wraps the table, whatever `renderWrapper` says. */
  .editor-host :global(.feedback-prose .tableWrapper) {
    margin: 1em 0;
    overflow-x: auto;
  }

  .editor-host :global(.feedback-prose table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.95em;
  }

  .editor-host :global(.feedback-prose th),
  .editor-host :global(.feedback-prose td) {
    position: relative;
    border: 1px solid var(--border);
    padding: 0.5em 0.7em;
    text-align: left;
    vertical-align: top;
  }

  .editor-host :global(.feedback-prose th) {
    background: var(--muted);
    font-weight: 650;
  }

  .editor-host :global(.feedback-prose th > p),
  .editor-host :global(.feedback-prose td > p) {
    margin: 0;
  }

  /* prosemirror-tables marks a multi-cell selection with this class. */
  .editor-host :global(.feedback-prose .selectedCell::after) {
    position: absolute;
    z-index: 2;
    inset: 0;
    background: color-mix(in oklab, var(--primary) 18%, transparent);
    content: '';
    pointer-events: none;
  }

  .editor-host :global(.feedback-prose ul[data-type='taskList']) {
    margin: 0 0 0.9em;
    padding: 0;
    list-style: none;
  }

  /* Task items carry `data-checked`, not `data-type`; scope through the list. */
  .editor-host :global(.feedback-prose ul[data-type='taskList'] > li) {
    display: flex;
    align-items: flex-start;
    gap: 0.55em;
    margin: 0.28em 0;
  }

  .editor-host :global(.feedback-prose ul[data-type='taskList'] > li > label) {
    margin-top: 0.34em;
  }

  .editor-host :global(.feedback-prose ul[data-type='taskList'] > li > div) {
    min-width: 0;
    flex: 1;
  }

  .editor-host :global(.feedback-prose ul[data-type='taskList'] > li > div > p) {
    margin: 0;
  }

  .editor-host :global(.feedback-prose ul[data-type='taskList'] ul[data-type='taskList']) {
    margin: 0.3em 0 0;
  }

  .editor-host :global(.feedback-prose.ProseMirror-focused) {
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--ring) 30%, transparent);
  }

  .editor-host :global(.feedback-prose a.attachment-file-chip) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0 2px;
    padding: 2px 10px 2px 4px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: color-mix(in oklab, var(--muted) 55%, transparent);
    color: var(--foreground);
    font-family: ui-sans-serif, system-ui, sans-serif;
    font-size: 12px;
    line-height: 1.4;
    text-decoration: none;
    cursor: pointer;
    vertical-align: middle;
  }

  .editor-host :global(.feedback-prose a.attachment-file-chip:hover) {
    border-color: var(--primary);
  }

  .editor-host :global(.attachment-file-chip-ext) {
    padding: 1px 5px;
    border-radius: 999px;
    background: var(--primary);
    color: var(--primary-foreground);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.03em;
  }
</style>
