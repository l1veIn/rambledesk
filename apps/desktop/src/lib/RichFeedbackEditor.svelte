<script lang="ts">
  import { Editor } from '@tiptap/core'
  import { Bold, Italic, Redo2, Undo2 } from '@lucide/svelte'
  import { onMount } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import type { AttachmentView } from './feedback'
  import { t } from './i18n'
  import { locale } from './preferences'
  import {
    ATTACHMENT_PLACEHOLDER_IMAGE,
    attachmentIdFromUrl,
    attachmentMarkdownUrl,
    isImageMediaType,
  } from './attachmentMarkdown'
  import {
    feedbackEditorExtensions,
    parseFeedbackMarkdown,
  } from './feedbackEditorExtensions'
  import {
    restoreFeedbackDraftDocument,
    snapshotFeedbackDraftDocument,
    type FeedbackDraftSnapshot,
  } from './feedbackDraftDocument'
  import {
    CLEANUP_STATE_ATTR,
    INPUT_SOURCE_ATTR,
    SPEECH_CLEANUP_TRANSACTION_META,
    SPEECH_SEGMENT_ID_ATTR,
    asrParagraphAttrs,
    isSpeechCleanupInFlight,
    setSpeechCleanupInFlight,
    type CleanupState,
    type SpeechCleanupSegment,
  } from './speechBlockMetadata'
  import { ACTION_CHANNEL_ATTR } from './workbench/actionChannel'
  import { alignCleanupParts, parseLabeledOutput } from './workbench/speechCleanupPolicy'

  export let markdown = ''
  export let documentJson: string | null = null
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let acceptExternalMarkdown = true
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  /**
   * Live source of the currently selected Action channel. Programmatic inserts
   * (speech, captures, attachments, pasted blocks) stamp with whatever the
   * owning session reports right now — never a copy stored inside this editor
   * instance, which can silently desync after remounts.
   */
  export let getCurrentActionIndex: () => number | null = () => null

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let applyingExternalChange = false
  let editorMarkdown = ''
  let historyLocked = false
  let canUndo = false
  let canRedo = false
  let openAttachmentHandler = (_attachmentId: string) => {}
  $: openAttachmentHandler = onOpenAttachment

  onMount(() => {
    editor = new Editor({
      element: editorHost,
      extensions: feedbackEditorExtensions(),
      content: restoreFeedbackDraftDocument(documentJson, markdown),
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
        const snapshot = editor
          ? snapshotFeedbackDraftDocument(editor.getJSON())
          : { documentJson: documentJson ?? '', bodyMarkdown: markdown }
        editorMarkdown = snapshot.bodyMarkdown
        syncHistoryButtons()
        hydrateAttachmentImages()
      },
      onTransaction: () => {
        syncHistoryButtons()
      },
      onUpdate: ({ editor: updatedEditor }) => {
        historyLocked = editorHasCleaningSpeech(updatedEditor)
        syncHistoryButtons()
        // Normalize attachment srcs on every change so an image never stays
        // on the unsupported attachment:// scheme across renders.
        hydrateAttachmentImages()
        if (applyingExternalChange) return
        const snapshot = snapshotFeedbackDraftDocument(updatedEditor.getJSON())
        editorMarkdown = snapshot.bodyMarkdown
        onChange(snapshot)
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
  $: if (acceptExternalMarkdown && editor && markdown !== editorMarkdown) applyMarkdown(markdown)
  $: if (editor) {
    previews
    hydrateAttachmentImages()
  }

  function applyMarkdown(nextMarkdown: string, emitChange = false) {
    if (!editor) return
    applyingExternalChange = true
    try {
      editor.commands.setContent(parseFeedbackMarkdown(nextMarkdown), {
        emitUpdate: false,
      })
      const snapshot = snapshotFeedbackDraftDocument(editor.getJSON())
      editorMarkdown = snapshot.bodyMarkdown
      hydrateAttachmentImages()
      if (emitChange) onChange(snapshot)
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
      const next = preview ?? ATTACHMENT_PLACEHOLDER_IMAGE
      if (!next || (node.attrs.attachmentId === attachmentId && node.attrs.src === next)) {
        return
      }
      transaction = transaction.setNodeMarkup(position, undefined, {
        ...node.attrs,
        attachmentId,
        src: next,
      })
      changed = true
    })
    if (!changed) return
    applyingExternalChange = true
    editor.view.dispatch(transaction)
    const snapshot = snapshotFeedbackDraftDocument(editor.getJSON())
    editorMarkdown = snapshot.bodyMarkdown
    applyingExternalChange = false
  }

  export function applyExternalMarkdown(nextMarkdown: string): boolean {
    if (!editor) return false
    if (nextMarkdown === editorMarkdown) return true
    applyMarkdown(nextMarkdown, true)
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
      .map((attachment) => {
        if (isImageMediaType(attachment.media_type)) {
          return {
            type: 'image',
            attrs: actionAttrs({
              src:
                previews[attachment.attachment_id] ??
                ATTACHMENT_PLACEHOLDER_IMAGE,
              alt: attachment.file_name,
              attachmentId: attachment.attachment_id,
            }),
          }
        }
        return {
          type: 'paragraph',
          attrs: actionAttrs(),
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
        }
      })
    if (content.length === 0) return false
    editor.commands.insertContentAt(documentEnd(), content)
    return true
  }

  function syncHistoryButtons() {
    canUndo = Boolean(editor && !historyLocked && editor.can().undo())
    canRedo = Boolean(editor && !historyLocked && editor.can().redo())
  }

  function editorHasCleaningSpeech(target = editor) {
    if (!target) return false
    let cleaning = false
    target.state.doc.descendants((node) => {
      const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
      if (
        typeof segmentId === 'string' &&
        isSpeechCleanupInFlight(target.state, segmentId)
      ) {
        cleaning = true
      }
    })
    return cleaning
  }

  export function actionAttrs(extra: Record<string, unknown> = {}) {
    const actionIndex = getCurrentActionIndex()
    return actionIndex != null ? { ...extra, [ACTION_CHANNEL_ATTR]: actionIndex } : extra
  }

  export function setActionChannel(index: number | null): void {
    if (!editor) return
    const { $from } = editor.state.selection
    const parent = $from.parent
    if (!parent.isTextblock || parent.textContent.trim() !== '') return
    const position = $from.before()
    const node = editor.state.doc.nodeAt(position)
    if (!node) return
    editor.view.dispatch(
      editor.state.tr.setNodeMarkup(position, undefined, {
        ...node.attrs,
        [ACTION_CHANNEL_ATTR]: index,
      }),
    )
  }

  export function appendTranscript(
    text: string,
    options?: { asr?: { segmentId: string; cleanupState: CleanupState } },
  ) {
    const transcript = text.trim()
    if (!editor || !transcript || disabled) return
    editor.commands.insertContentAt(editor.state.doc.content.size, {
      type: 'paragraph',
      attrs: actionAttrs(
        options?.asr
          ? asrParagraphAttrs(options.asr.segmentId, options.asr.cleanupState)
          : {},
      ),
      content: [{ type: 'text', text: transcript }],
    })
  }

  export function isSpeechCleaning() {
    const cleaning = editorHasCleaningSpeech()
    if (import.meta.env.DEV) {
      console.log('[ramble-cleanup] editor isSpeechCleaning=', cleaning)
    }
    return cleaning
  }

  export function beginSpeechCleanup(segments: SpeechCleanupSegment[]): void {
    if (!editor || segments.length === 0) return
    editor.view.dispatch(
      setSpeechCleanupInFlight(
        editor.state.tr,
        segments.map((segment) => segment.segmentId),
        true,
      ),
    )
    historyLocked = true
    if (import.meta.env.DEV) {
      console.log(
        '[ramble-cleanup] editor beginSpeechCleanup segments=',
        segments.map((segment) => segment.segmentId),
      )
    }
  }

  export function finishSpeechCleanup(
    segments: SpeechCleanupSegment[],
    cleaned: string | null,
  ): void {
    if (!editor) return
    const items: Array<{
      from: number
      to: number
      text: string
      attrs: Record<string, unknown>
      segmentId: string
    }> = []
    const segmentIds = new Set(segments.map((segment) => segment.segmentId))
    editor.state.doc.descendants((node, position) => {
      const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
      if (
        node.type.name === 'paragraph' &&
        typeof segmentId === 'string' &&
        segmentIds.has(segmentId)
      ) {
        items.push({
          from: position,
          to: position + node.nodeSize,
          text: node.textContent,
          attrs: { ...node.attrs },
          segmentId,
        })
      }
    })
    historyLocked = false
    if (items.length === 0) {
      editor.view.dispatch(
        setSpeechCleanupInFlight(
          editor.state.tr,
          segments.map((segment) => segment.segmentId),
          false,
        ),
      )
      syncHistoryButtons()
      return
    }
    const parts =
      parseLabeledOutput(cleaned ?? '', items.length) ??
      alignCleanupParts(
        items.map((item) => item.text),
        cleaned,
      )
    const originalById = new Map(segments.map((segment) => [segment.segmentId, segment.text]))
    const allUnchanged = items.every((item) => originalById.get(item.segmentId) === item.text)
    const contiguous = items.every(
      (item, index) => index === 0 || items[index - 1]!.to === item.from,
    )
    let transaction = editor.state.tr
    if (cleaned != null && allUnchanged && contiguous && items.length > 0 && editor) {
      // Batch-whole replacement: the batch was untouched and its segments are
      // consecutive, so the whole range becomes whatever the model returned —
      // one block when it merged the batch, several when it kept the count.
      const blocks =
        parts ??
        cleaned
          .split(/\n{2,}/)
          .map((block) => block.trim())
          .filter(Boolean)
      const schema = editor.schema
      const paragraphs = (blocks.length > 0 ? blocks : [cleaned]).map((text) =>
        schema.nodes.paragraph.create(
          { ...items[0]!.attrs, [INPUT_SOURCE_ATTR]: 'asr', [CLEANUP_STATE_ATTR]: 'cleaned' },
          text ? schema.text(text) : undefined,
        ),
      )
      transaction = transaction.replaceWith(
        items[0]!.from,
        items[items.length - 1]!.to,
        paragraphs,
      )
      if (import.meta.env.DEV) {
        console.log(
          '[ramble-cleanup] editor batch-replace blocks=',
          paragraphs.length,
          'items=',
          items.length,
          'range=',
          items[0]!.from,
          '-',
          items[items.length - 1]!.to,
        )
      }
    } else if (parts == null && cleaned != null) {
      // The model merged the batch into fewer blocks: replace the first
      // segment with the whole cleaned text and drop the rest.
      for (let index = items.length - 1; index >= 0; index -= 1) {
        const item = items[index]!
        if (index === 0) {
          transaction = transaction.replaceWith(
            item.from,
            item.to,
            editor.schema.nodes.paragraph.create(
              {
                ...item.attrs,
                [INPUT_SOURCE_ATTR]: 'asr',
                [CLEANUP_STATE_ATTR]: 'cleaned',
              },
              cleaned ? editor.schema.text(cleaned) : undefined,
            ),
          )
        } else {
          transaction = transaction.delete(item.from, item.to)
        }
      }
    } else {
      for (let index = items.length - 1; index >= 0; index -= 1) {
        const item = items[index]
        const original = originalById.get(item.segmentId)
        const unchanged = original === item.text
        const state: CleanupState = !unchanged
          ? 'skipped'
          : cleaned == null
            ? 'failed'
            : 'cleaned'
        const segmentIndex = segments.findIndex((segment) => segment.segmentId === item.segmentId)
        const text = cleaned != null && unchanged
          ? (parts?.[segmentIndex] ?? item.text)
          : item.text
        const attrs = {
          ...item.attrs,
          [INPUT_SOURCE_ATTR]: 'asr',
          [CLEANUP_STATE_ATTR]: state,
        }
        transaction = transaction.replaceWith(
          item.from,
          item.to,
          editor.schema.nodes.paragraph.create(
            attrs,
            text ? editor.schema.text(text) : undefined,
          ),
        )
      }
    }
    transaction.setMeta(SPEECH_CLEANUP_TRANSACTION_META, true)
    setSpeechCleanupInFlight(
      transaction,
      segments.map((segment) => segment.segmentId),
      false,
    )
    editor.view.dispatch(transaction)
    syncHistoryButtons()
  }

  /**
   * Programmatic inserts (screenshots, clipboard captures, files, pasted
   * blocks) always land at the end of the document: a Ramble is authored
   * voice-first and reorders nothing, while any finer placement remains a
   * direct manual edit in the editor.
   */
  function documentEnd(): number {
    return editor?.state.doc.content.size ?? 0
  }

  export function moveCursorAfterCleaningSpeech() {
    if (!editor) return
    let position = editor.state.selection.from
    editor.state.doc.descendants((node, from) => {
      const segmentId = node.attrs?.[SPEECH_SEGMENT_ID_ATTR]
      if (
        typeof segmentId !== 'string' ||
        !isSpeechCleanupInFlight(editor!.state, segmentId)
      ) return
      const to = from + node.nodeSize
      if (position > from && position < to) position = to
    })
    editor.commands.setTextSelection(
      Math.min(Math.max(position, 0), editor.state.doc.content.size),
    )
  }

  export function insertQuotedBlock(lines: string[]) {
    if (!editor || disabled || lines.length === 0) return false
    const content = lines.map((line) => ({
      type: 'paragraph' as const,
      content: line ? [{ type: 'text' as const, text: line }] : [],
    }))
    return editor.commands.insertContentAt(documentEnd(), [
      { type: 'blockquote', attrs: actionAttrs(), content },
    ])
  }

  export function insertMarkdownAtCaret(markdown: string) {
    const block = markdown.trim()
    if (!editor || disabled || !block) return false
    const parsed = parseFeedbackMarkdown(block)
    const stamped = {
      ...parsed,
      content: (parsed.content ?? []).map((node) => ({
        ...node,
        attrs: actionAttrs(node.attrs ?? {}),
      })),
    }
    return editor.commands.insertContentAt(documentEnd(), stamped.content ?? [])
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
    return editor.commands.insertContentAt(documentEnd(), [
      {
        type: 'blockquote',
        attrs: actionAttrs(),
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
    ])
  }

  export function appendCapturedAttachment(
    attachment: AttachmentView,
    label: string,
  ) {
    if (!editor || disabled) return false
    return editor.commands.insertContentAt(documentEnd(), [
      {
        type: 'blockquote',
        attrs: actionAttrs(),
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
        attrs: actionAttrs({
          src:
            previews[attachment.attachment_id] ??
            ATTACHMENT_PLACEHOLDER_IMAGE,
          alt: attachment.file_name,
          attachmentId: attachment.attachment_id,
        }),
      },
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
    <span class="flex-1"></span>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Undo')}
      title={t($locale, 'Undo')}
      disabled={disabled || !canUndo}
      onclick={() => editor?.chain().focus().undo().run()}
    >
      <Undo2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={t($locale, 'Redo')}
      title={t($locale, 'Redo')}
      disabled={disabled || !canRedo}
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
    border-left: 1px solid var(--muted-foreground);
    border-radius: 4px;
    background: color-mix(in srgb, var(--muted) 35%, transparent);
    padding-left: 8px;
  }

  .editor-host :global(.feedback-prose p.speech-cleaning::after) {
    color: var(--muted-foreground);
    content: '  ·  ' attr(data-speech-hint);
    font-size: 11px;
  }

  .editor-host :global(.feedback-prose .action-channel-item) {
    background: color-mix(in srgb, var(--muted) 72%, transparent);
    border-radius: 0;
    margin-bottom: 0;
    padding: 0 14px;
  }

  .editor-host :global(.feedback-prose .action-channel-item.action-channel-lead) {
    border-radius: 10px 10px 0 0;
    padding-top: 10px;
  }

  .editor-host :global(.feedback-prose .action-channel-item.action-channel-group-solo) {
    border-radius: 10px;
    margin-bottom: 0.9em;
    padding-bottom: 10px;
  }

  .editor-host :global(.feedback-prose .action-channel-item.action-channel-group-end) {
    border-radius: 0 0 10px 10px;
    margin-bottom: 0.9em;
    padding-bottom: 10px;
  }

  .editor-host :global(.feedback-prose .action-channel-lead[data-action-index]::before) {
    color: var(--primary);
    content: '@ Action' attr(data-action-index) ' ';
    font-size: 0.8em;
    font-weight: 600;
    pointer-events: none;
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

  /* Inside an Action channel block the image/quote is part of the continuous
     background: reduce the outer whitespace so it hugs the block edge. */
  .editor-host :global(.feedback-prose .action-channel-item img) {
    margin: 10px 0;
  }

  .editor-host :global(.feedback-prose .action-channel-item blockquote) {
    margin: 10px 0;
    padding: 10px 12px;
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
