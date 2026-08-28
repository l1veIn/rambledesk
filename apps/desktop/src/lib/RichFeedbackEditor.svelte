<script lang="ts">
  import { Editor, type JSONContent } from '@tiptap/core'
  import {
    Bold,
    Heading2,
    Italic,
    List,
    Quote,
    Redo2,
    Sparkles,
    Undo2,
  } from '@lucide/svelte'
  import { EditorState, type Transaction } from '@tiptap/pm/state'
  import { onMount } from 'svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { actionBlockquoteNode, isEmptyActionGroup } from './actionBlockquote'
  import {
    attachmentIdFromUrl,
    attachmentMarkdownUrl,
    isImageMediaType,
  } from './attachmentMarkdown'
  import {
    attachmentNodes,
    clipboardNodes,
    speechNodes,
    type DraftOperation,
  } from './draftOperations'
  import type { AttachmentView } from './feedback'
  import {
    snapshotFeedbackDraftDocument,
    snapshotFeedbackDraftMarkdown,
    type FeedbackDraftSnapshot,
  } from './feedbackDraftDocument'
  import { feedbackEditorExtensions } from './feedbackEditorExtensions'
  import { t } from './i18n'
  import { locale } from './preferences'
  import {
    CLEANUP_STATE_ATTR,
    SPEECH_SEGMENT_ID_ATTR,
    speechCleanupCandidates,
    type SpeechCleanupSegment,
  } from './speechBlockMetadata'

  export let document: JSONContent | null = null
  export let editorEpoch = 0
  export let markdown = ''
  export let overlayMarkdown: string | null = null
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onTidy: () => void = () => {}
  export let tidyBusy = false

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let applyingExternalChange = false
  let editorMarkdown = ''
  let loadedEpoch = -1
  let loadedOverlay: string | null = null
  let insertionPosition = 0
  let pendingCount = 0
  let openAttachmentHandler = (_attachmentId: string) => {}
  $: openAttachmentHandler = onOpenAttachment

  onMount(() => {
    editor = new Editor({
      element: editorHost,
      extensions: feedbackEditorExtensions(),
      content: document ?? markdown,
      ...(document ? {} : { contentType: 'markdown' as const }),
      editable: !disabled,
      editorProps: {
        attributes: {
          class: 'feedback-prose',
          'aria-label': t($locale, 'Markdown rich-text feedback body'),
          'data-placeholder': t($locale, 'Record what you saw, what felt smooth, and where you paused.'),
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
        loadedEpoch = editorEpoch
        loadedOverlay = overlayMarkdown
        insertionPosition = editor?.state.doc.content.size ?? 0
        pendingCount = editor ? speechCleanupCandidates(editor.getJSON()).length : 0
        hydrateAttachmentImages()
      },
      onUpdate: ({ editor: updatedEditor }) => {
        if (applyingExternalChange) return
        emitSnapshot(updatedEditor)
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
  $: if (editor && overlayMarkdown != null && overlayMarkdown !== loadedOverlay) {
    applyMarkdown(overlayMarkdown)
    loadedOverlay = overlayMarkdown
  }
  $: if (editor && overlayMarkdown == null && editorEpoch !== loadedEpoch) {
    applyDocument(document ?? { type: 'doc', content: [{ type: 'paragraph' }] })
    loadedEpoch = editorEpoch
    loadedOverlay = null
  }
  $: if (editor) {
    previews
    hydrateAttachmentImages()
  }

  function emitSnapshot(source: Editor) {
    const json = source.getJSON()
    const snapshot = snapshotFeedbackDraftDocument(json)
    editorMarkdown = snapshot.bodyMarkdown
    pendingCount = speechCleanupCandidates(json).length
    onChange(snapshot)
  }

  function trimTrailingEmptyActionGroups(
    transaction: Transaction,
    keepActionId?: string | null,
  ): Transaction {
    let next = transaction
    while (next.doc.childCount > 0) {
      const last = next.doc.lastChild
      if (!last || !isEmptyActionGroup(last.toJSON())) break
      if (keepActionId && last.attrs.actionId === keepActionId) break
      next = next.delete(next.doc.content.size - last.nodeSize, next.doc.content.size)
    }
    return next
  }

  function resetHistory() {
    if (!editor) return
    editor.view.updateState(
      EditorState.create({
        schema: editor.state.schema,
        doc: editor.state.doc,
        plugins: editor.state.plugins,
      }),
    )
  }

  function applyDocument(nextDocument: JSONContent) {
    if (!editor) return
    applyingExternalChange = true
    try {
      editor.commands.setContent(nextDocument, { emitUpdate: false })
      resetHistory()
      editorMarkdown = editor.getMarkdown()
      insertionPosition = Math.min(insertionPosition, editor.state.doc.content.size)
      hydrateAttachmentImages()
      pendingCount = speechCleanupCandidates(editor.getJSON()).length
    } catch (cause) {
      console.error('[richEditor] applyDocument failed', cause)
    } finally {
      applyingExternalChange = false
    }
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
    const snapshot = snapshotFeedbackDraftMarkdown(nextMarkdown)
    if (snapshot.bodyMarkdown === editorMarkdown) return true
    applyMarkdown(nextMarkdown)
    onChange(snapshot)
    return true
  }

  export function applyExternalDocument(nextDocument: JSONContent): boolean {
    if (!editor) return false
    applyDocument(nextDocument)
    emitSnapshot(editor)
    return true
  }

  export function applyDraftOperation(operation: DraftOperation): boolean {
    if (!editor || disabled) return false
    let transaction = trimTrailingEmptyActionGroups(
      editor.state.tr,
      operation.kind === 'startActionGroup'
        ? operation.action.actionId
        : operation.action?.actionId,
    )
    if (operation.kind === 'startActionGroup') {
      const last = transaction.doc.lastChild
      if (last?.type.name === 'blockquote' && last.attrs.actionId === operation.action.actionId) {
        if (transaction.docChanged) {
          editor.view.dispatch(transaction)
          emitSnapshot(editor)
        }
        return true
      }
      const node = editor.schema.nodeFromJSON(actionBlockquoteNode(operation.action))
      transaction = transaction.insert(transaction.doc.content.size, node)
      editor.view.dispatch(transaction)
      emitSnapshot(editor)
      return true
    }
    const nodes =
      operation.kind === 'appendSpeech'
        ? speechNodes(operation.segmentId, operation.text)
        : operation.kind === 'appendClipboardText'
          ? clipboardNodes(operation.text, operation.label)
          : attachmentNodes(
              operation.attachment,
              operation.label,
              previews[operation.attachment.attachment_id],
            )
    const action = operation.action
    const last = transaction.doc.lastChild
    const insertAt =
      action && last?.type.name === 'blockquote' && last.attrs.actionId === action.actionId
        ? transaction.doc.content.size - 1
        : transaction.doc.content.size
    const content = action && insertAt === transaction.doc.content.size
      ? actionBlockquoteNode(action, nodes)
      : nodes
    if (transaction.docChanged) editor.view.dispatch(transaction)
    return editor.commands.insertContentAt(insertAt, content)
  }

  export function pendingSpeechSegments(): SpeechCleanupSegment[] {
    if (!editor) return []
    return speechCleanupCandidates(editor.getJSON())
  }

  export function replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean {
    if (!editor || replacements.length === 0) return false
    const wanted = new Map(replacements.map((item) => [item.segmentId, item]))
    const targets: Array<{
      from: number
      to: number
      pos: number
      attrs: Record<string, unknown>
      nextText: string
    }> = []
    editor.state.doc.descendants((node, position) => {
      if (node.type.name !== 'paragraph') return
      const segmentId = node.attrs[SPEECH_SEGMENT_ID_ATTR]
      const replacement = typeof segmentId === 'string' ? wanted.get(segmentId) : undefined
      if (!replacement) return
      if (node.textContent.trim() !== replacement.originalText.trim()) return
      targets.push({
        from: position + 1,
        to: position + node.nodeSize - 1,
        pos: position,
        attrs: { ...node.attrs, [CLEANUP_STATE_ATTR]: 'cleaned' },
        nextText: replacement.nextText,
      })
    })
    if (targets.length === 0) return false
    let transaction = editor.state.tr
    for (const target of targets.reverse()) {
      transaction = transaction.replaceWith(
        target.from,
        target.to,
        editor.schema.text(target.nextText),
      )
      transaction = transaction.setNodeMarkup(target.pos, undefined, target.attrs)
    }
    editor.view.dispatch(transaction)
    emitSnapshot(editor)
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
      variant={pendingCount > 0 ? 'secondary' : 'ghost'}
      size="sm"
      class="h-7 shrink-0 gap-1 px-2 text-[10px]"
      aria-label={t($locale, 'Tidy now')}
      title={pendingCount > 0
        ? t($locale, 'Tidy {count} pending speech segments', { count: pendingCount })
        : t($locale, 'Tidy pending speech segments. It appears here after Ramble writes a transcript.')}
      disabled={disabled || tidyBusy || pendingCount === 0}
      onclick={() => onTidy()}
    >
      <Sparkles class="size-3.5" />
      {tidyBusy ? t($locale, 'Tidying…') : t($locale, 'Tidy now')}
      {#if pendingCount > 0}
        <Badge variant="secondary" class="h-4 px-1 text-[9px]">{pendingCount}</Badge>
      {/if}
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Undo')}
      title={t($locale, 'Undo')}
      disabled={disabled || !editor?.can().undo()}
      onclick={() => editor?.chain().focus().undo().run()}
    >
      <Undo2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Redo')}
      title={t($locale, 'Redo')}
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

  .editor-host :global(.feedback-prose blockquote[data-action-id]) {
    border-left-color: var(--foreground);
    color: var(--foreground);
    background: color-mix(in oklab, var(--muted) 40%, transparent);
  }

  .editor-host :global(.feedback-prose p[data-cleanup-state='pending']) {
    box-shadow: inset 3px 0 0 color-mix(in oklab, var(--primary) 55%, transparent);
    padding-left: 10px;
  }

  .editor-host :global(.feedback-prose p[data-cleanup-state='cleaned']) {
    box-shadow: inset 3px 0 0 color-mix(in oklab, var(--muted-foreground) 35%, transparent);
    padding-left: 10px;
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
