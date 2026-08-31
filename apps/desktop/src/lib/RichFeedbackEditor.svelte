<script lang="ts">
  import { Editor, type JSONContent } from '@tiptap/core'
  import {
    Bold,
    Heading2,
    Italic,
    List,
    Quote,
    Redo2,
    Undo2,
  } from '@lucide/svelte'
  import { Fragment } from '@tiptap/pm/model'
  import { EditorState, type Transaction } from '@tiptap/pm/state'
  import { onMount } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import {
    actionBlockquoteNode,
    isEmptyActionGroup,
    isEmptyParagraph,
  } from './actionBlockquote'
  import {
    attachmentIdFromUrl,
  } from './attachmentMarkdown'
  import {
    attachmentNodes,
    clipboardNodes,
    draftOperationAlreadyApplied,
    speechNodes,
    type DraftOperation,
  } from './draftOperations'
  import {
    snapshotFeedbackDraftDocument,
    type FeedbackDraftSnapshot,
  } from './feedbackDraftDocument'
  import { feedbackEditorExtensions } from './feedbackEditorExtensions'
  import { t } from './i18n'
  import { distinguishUntidiedText, locale } from './preferences'
  import {
    applySpeechCleanupResults,
    setTidyingSpeechSegments,
    speechCleanupCandidates,
    type SpeechCleanupSegment,
  } from './speechBlockMetadata'

  export let document: JSONContent | null = null
  export let editorEpoch = 0
  export let markdown = ''
  export let previews: Record<string, string> = {}
  export let disabled = false
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let tidyingSegmentIds: string[] = []

  let editorHost: HTMLDivElement
  let editor: Editor | null = null
  let applyingExternalChange = false
  let editorMarkdown = ''
  let loadedEpoch = -1
  let insertionPosition = 0
  let tidyingSignature = ''
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
        insertionPosition = editor?.state.doc.content.size ?? 0
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

  // Tiptap emits an update by default when editability changes. Busy/closed
  // state is not a document edit, so never turn that UI transition into a
  // dirty Feedback Draft.
  $: if (editor && editor.isEditable !== !disabled) editor.setEditable(!disabled, false)
  $: if (editor) {
    $locale
    editor.view.dom.setAttribute('aria-label', t($locale, 'Markdown rich-text feedback body'))
    editor.view.dom.setAttribute('data-placeholder', t($locale, 'Record what you saw, what felt smooth, and where you paused.'))
  }
  $: if (editor && editorEpoch !== loadedEpoch) {
    applyDocument(document ?? { type: 'doc', content: [{ type: 'paragraph' }] })
    loadedEpoch = editorEpoch
  }
  $: if (editor) {
    previews
    hydrateAttachmentImages()
  }
  $: if (editor) {
    const nextSignature = [...new Set(tidyingSegmentIds)].sort().join('\u0000')
    if (nextSignature !== tidyingSignature) {
      tidyingSignature = nextSignature
      setTidyingSpeechSegments(editor, tidyingSegmentIds)
    }
  }

  function emitSnapshot(source: Editor) {
    const json = source.getJSON()
    const snapshot = snapshotFeedbackDraftDocument(json)
    editorMarkdown = snapshot.bodyMarkdown
    onChange(snapshot)
  }

  function trimTrailingEmptyActionGroups(
    transaction: Transaction,
    keepActionId?: string | null,
    removeActionId?: string | null,
  ): Transaction {
    let next = transaction
    while (next.doc.childCount > 0) {
      const candidate = lastMeaningfulChild(next.doc)
      if (!candidate) break
      const json = candidate.node.toJSON()
      if (isEmptyActionGroup(json)) {
        if (keepActionId && candidate.node.attrs.actionId === keepActionId) break
        if (removeActionId && candidate.node.attrs.actionId !== removeActionId) break
        next = next.delete(candidate.pos, candidate.pos + candidate.node.nodeSize)
        continue
      }
      break
    }
    return next
  }

  function lastMeaningfulChild(doc: Transaction['doc']): { node: NonNullable<Transaction['doc']['lastChild']>; pos: number } | null {
    let found: { node: NonNullable<Transaction['doc']['lastChild']>; pos: number } | null = null
    doc.forEach((node, pos) => {
      if (isEmptyParagraph(node.toJSON())) return
      found = { node, pos }
    })
    return found
  }

  function insertJsonContent(transaction: Transaction, pos: number, nodes: JSONContent[]) {
    const schema = editor?.schema
    if (!schema || nodes.length === 0) return transaction
    const fragment = Fragment.fromArray(nodes.map((node) => schema.nodeFromJSON(node)))
    return transaction.insert(pos, fragment)
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
    } catch (cause) {
      console.error('[richEditor] applyDocument failed', cause)
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

  export function applyDraftOperation(operation: DraftOperation): boolean {
    if (!editor || disabled) return false
    const current = editor.getJSON()
    if (draftOperationAlreadyApplied(current, operation)) return true
    const keepActionId =
      operation.kind === 'startActionGroup'
        ? operation.action.actionId
        : operation.kind === 'clearActionGroup'
          ? null
          : operation.action?.actionId
    let transaction = trimTrailingEmptyActionGroups(
      editor.state.tr,
      keepActionId,
      operation.kind === 'clearActionGroup' ? operation.actionId : null,
    )
    if (operation.kind === 'clearActionGroup') {
      if (transaction.docChanged) editor.view.dispatch(transaction)
      return true
    }
    if (operation.kind === 'startActionGroup') {
      const last = lastMeaningfulChild(transaction.doc)
      if (
        last?.node.type.name === 'blockquote' &&
        last.node.attrs.actionId === operation.action.actionId
      ) {
        if (transaction.docChanged) editor.view.dispatch(transaction)
        return true
      }
      transaction = insertJsonContent(
        transaction,
        transaction.doc.content.size,
        [actionBlockquoteNode(operation.action)],
      )
      editor.view.dispatch(transaction)
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
    const last = lastMeaningfulChild(transaction.doc)
    if (action && last?.node.type.name === 'blockquote' && last.node.attrs.actionId === action.actionId) {
      transaction = insertJsonContent(transaction, last.pos + last.node.nodeSize - 1, nodes)
    } else if (action) {
      transaction = insertJsonContent(
        transaction,
        transaction.doc.content.size,
        [actionBlockquoteNode(action, nodes)],
      )
    } else {
      transaction = insertJsonContent(transaction, transaction.doc.content.size, nodes)
    }
    editor.view.dispatch(transaction)
    return true
  }

  export function pendingSpeechSegments(): SpeechCleanupSegment[] {
    if (!editor) return []
    return speechCleanupCandidates(editor.getJSON())
  }

  export function replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean {
    if (!editor || replacements.length === 0) return false
    const result = applySpeechCleanupResults(editor.getJSON(), replacements)
    if (!result.changed) return false
    const nextDocument = editor.schema.nodeFromJSON(result.document)
    editor.view.dispatch(
      editor.state.tr.replaceWith(0, editor.state.doc.content.size, nextDocument.content),
    )
    return true
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
  <div
    class="editor-host min-h-0 flex-1 overflow-y-auto overscroll-contain"
    class:distinguish-untidied={$distinguishUntidiedText}
    bind:this={editorHost}
  ></div>
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
    padding: 14px 18px 12px;
    border: 1px solid color-mix(in oklab, var(--primary) 16%, transparent);
    border-radius: calc(var(--radius) + 2px);
    color: var(--foreground);
    background: color-mix(in oklab, var(--primary) 9%, var(--background));
  }

  .editor-host :global(.feedback-prose blockquote[data-action-id] > p:first-child) {
    margin: 0 0 0.8em;
    padding-bottom: 0.65em;
    border-bottom: 1px solid color-mix(in oklab, var(--primary) 14%, transparent);
    font-family: ui-sans-serif, system-ui, sans-serif;
    font-size: 12px;
    font-weight: 700;
    line-height: 1.5;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .editor-host :global(.feedback-prose blockquote[data-action-id] > p:first-child > strong) {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .editor-host :global(.feedback-prose blockquote[data-action-id] > p:last-child) {
    margin-bottom: 0;
  }

  .editor-host.distinguish-untidied :global(.feedback-prose p[data-cleanup-state='pending']) {
    position: relative;
    padding-inline-start: 22px;
  }

  .editor-host.distinguish-untidied :global(.feedback-prose p[data-cleanup-state='pending']:not(.speech-segment-tidying)::before) {
    position: absolute;
    top: 0.42em;
    inset-inline-start: 1px;
    width: 14px;
    height: 14px;
    background-color: color-mix(in oklab, var(--primary) 58%, var(--muted-foreground));
    content: '';
    -webkit-mask: url("data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='black'%20d='M12%2014q1.25%200%202.125-.875T15%2011V5q0-1.25-.875-2.125T12%202q-1.25%200-2.125.875T9%205v6q0%201.25.875%202.125T12%2014Zm-1%207v-3.075q-2.6-.35-4.3-2.325T5%2011h2q0%202.075%201.463%203.537T12%2016q2.075%200%203.538-1.463T17%2011h2q0%202.625-1.7%204.6T13%2017.925V21Z'/%3E%3C/svg%3E") center / contain no-repeat;
    mask: url("data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20viewBox='0%200%2024%2024'%3E%3Cpath%20fill='black'%20d='M12%2014q1.25%200%202.125-.875T15%2011V5q0-1.25-.875-2.125T12%202q-1.25%200-2.125.875T9%205v6q0%201.25.875%202.125T12%2014Zm-1%207v-3.075q-2.6-.35-4.3-2.325T5%2011h2q0%202.075%201.463%203.537T12%2016q2.075%200%203.538-1.463T17%2011h2q0%202.625-1.7%204.6T13%2017.925V21Z'/%3E%3C/svg%3E") center / contain no-repeat;
  }

  .editor-host :global(.feedback-prose p.speech-segment-tidying) {
    position: relative;
    padding-left: 22px;
  }

  .editor-host :global(.feedback-prose p.speech-segment-tidying::before) {
    position: absolute;
    top: 0.48em;
    left: 1px;
    width: 12px;
    height: 12px;
    border: 2px solid color-mix(in oklab, var(--primary) 22%, transparent);
    border-top-color: var(--primary);
    border-radius: 999px;
    animation: speech-tidying-spin 0.75s linear infinite;
    content: '';
  }

  @keyframes speech-tidying-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .editor-host :global(.feedback-prose p.speech-segment-tidying::before) {
      animation-duration: 1.8s;
    }
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
