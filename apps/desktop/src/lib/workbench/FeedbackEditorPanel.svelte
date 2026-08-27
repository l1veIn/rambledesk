<script lang="ts">
  import {
    AlertCircle,
    Check,
    ChefHat,
    CloudCog,
    FileText,
    LoaderCircle,
    Sparkles,
    Undo2,
  } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import RichFeedbackEditor from '$lib/RichFeedbackEditor.svelte'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import SessionDraftEditor from './SessionDraftEditor.svelte'
  import type { AttachmentView, FeedbackWorkspaceView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { hasCookedPublishedVariant } from '$lib/publishedFeedback'
  import MarkdownPreview from './MarkdownPreview.svelte'
  import type { FeedbackEditorHandle, SavePhase } from './types'
  import { actionChannelFor } from './actionChannelState'

  export let workspace: FeedbackWorkspaceView
  export let draftBody = ''
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let cooking = false
  export let cookedDraftReady = false
  export let cookedPreviewModel = ''
  export let locked = false
  export let cleanupCount = 0
  export let pendingCleanupCount = 0
  export let tidyBusy = false
  export let onTidyNow: () => void = () => {}
  export let cookedMarkdown = ''
  export let uncookedMarkdown = ''
  export let formatTime: (value: string | null | undefined) => string
  export let onChange: (markdown: string) => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let draftEditors: Array<{
    requestId: string
    initialDocumentJson: string
    initialMarkdown: string
  }> = []
  export let visibleRequestId = ''
  export let onDraftChangeFor: (requestId: string, snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onEditorReady: (requestId: string, editor: FeedbackEditorHandle | null) => void = () => {}
  export let onPrepareNonSpeechInsert: (requestId: string) => void = () => {}

  let richEditor: RichFeedbackEditor
  const sessionEditors: Record<string, RichFeedbackEditor> = {}
  let publishedView: 'cooked' | 'uncooked' = 'cooked'

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: editingDisabled = readOnly || locked
  $: hasCookedVariant = readOnly && hasCookedPublishedVariant(cookedMarkdown, uncookedMarkdown)
  $: displayedMarkdown =
    readOnly
      ? hasCookedVariant && publishedView === 'uncooked'
        ? uncookedMarkdown
        : cookedMarkdown || uncookedMarkdown
      : draftBody

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function saveLabel() {
    if (savePhase === 'saving') return tr('Saving…')
    if (savePhase === 'unsaved') return tr('Waiting to autosave')
    if (savePhase === 'error') return tr('Save failed')
    return `${tr('Saved')} · r${savedRevision}`
  }

  function visibleEditor() {
    return sessionEditors[visibleRequestId] ?? richEditor
  }

  function captureSessionEditor(requestId: string, editor: FeedbackEditorHandle | null) {
    if (import.meta.env.DEV) {
      console.log('[ramble-cleanup] panel editor-ready request=', requestId, 'editor=', editor != null)
    }
    if (editor) sessionEditors[requestId] = editor as RichFeedbackEditor
    else delete sessionEditors[requestId]
    onEditorReady(requestId, editor)
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    if (visibleRequestId) onPrepareNonSpeechInsert(visibleRequestId)
    return visibleEditor()?.insertAttachments(attachments) ?? false
  }

  export function applyExternalMarkdown(markdown: string): boolean {
    return visibleEditor()?.applyExternalMarkdown(markdown) ?? false
  }

  export function insertQuotedBlock(lines: string[]) {
    if (visibleRequestId) onPrepareNonSpeechInsert(visibleRequestId)
    return visibleEditor()?.insertQuotedBlock?.(lines) ?? false
  }

  export function appendTranscript(
    text: string,
    options?: Parameters<FeedbackEditorHandle['appendTranscript']>[1],
  ) {
    visibleEditor()?.appendTranscript(text, options)
  }

  export function beginSpeechCleanup(
    segments: Parameters<NonNullable<FeedbackEditorHandle['beginSpeechCleanup']>>[0],
  ) {
    visibleEditor()?.beginSpeechCleanup?.(segments)
  }

  export function finishSpeechCleanup(
    segments: Parameters<NonNullable<FeedbackEditorHandle['finishSpeechCleanup']>>[0],
    cleaned: string | null,
  ) {
    visibleEditor()?.finishSpeechCleanup?.(segments, cleaned)
  }

  export function isSpeechCleaning() {
    return visibleEditor()?.isSpeechCleaning?.() ?? false
  }

  export function moveCursorAfterCleaningSpeech() {
    visibleEditor()?.moveCursorAfterCleaningSpeech?.()
  }

  export function appendClipboardCapture(text: string, label: string) {
    return visibleEditor()?.appendClipboardCapture(text, label) ?? false
  }

  export function appendCapturedAttachment(attachment: AttachmentView, label: string) {
    return visibleEditor()?.appendCapturedAttachment(attachment, label) ?? false
  }

  export function removeAttachmentReference(attachmentId: string) {
    visibleEditor()?.removeAttachmentReference(attachmentId)
  }
</script>

<section
  class={[
    'flex h-full min-h-0 flex-1 flex-col p-5 transition-colors',
    dragActive ? 'bg-primary/5 ring-2 ring-inset ring-primary/30' : '',
  ]}
>
  <header class="mb-3 flex items-center gap-3">
    <div class="min-w-0 flex-1">
      <h2 class="m-0 text-xs font-medium">{tr('Feedback document')}</h2>
      <p class="m-0 mt-0.5 text-[10px] text-muted-foreground">
        {readOnly ? tr('This request is closed. The document is read-only.') : tr('Record observations, problems, and suggestions.')}
      </p>
    </div>
    {#if cleanupCount > 0 && !readOnly}
      <span class="shrink-0 text-[10px] text-muted-foreground">
        {tr('Cleaned {count} times', { count: cleanupCount })}
      </span>
    {/if}
    {#if pendingCleanupCount > 0 && !readOnly}
      <Button
        variant="outline"
        size="sm"
        class="shrink-0 gap-1.5 h-7 px-2 text-[10px]"
        disabled={tidyBusy}
        title={tr('Tidy pending speech')}
        onclick={onTidyNow}
      >
        <Sparkles class="size-3.5" />
        {tr('Tidy now')}
      </Button>
    {/if}
    {#if hasCookedVariant}
      <div class="ml-auto flex items-center gap-1 rounded-md border bg-muted/30 p-0.5">
        <Button
          variant={publishedView === 'cooked' ? 'secondary' : 'ghost'}
          size="sm"
          class={publishedView === 'cooked' ? 'h-7 px-2 text-[10px]' : 'size-7 p-0'}
          aria-label="Cooked"
          title="Cooked"
          onclick={() => (publishedView = 'cooked')}
        >
          <Sparkles data-icon={publishedView === 'cooked' ? 'inline-start' : undefined} />
          {#if publishedView === 'cooked'}Cooked{/if}
        </Button>
        <Button
          variant={publishedView === 'uncooked' ? 'secondary' : 'ghost'}
          size="sm"
          class={publishedView === 'uncooked' ? 'h-7 px-2 text-[10px]' : 'size-7 p-0'}
          aria-label="Uncooked"
          title="Uncooked"
          onclick={() => (publishedView = 'uncooked')}
        >
          <FileText data-icon={publishedView === 'uncooked' ? 'inline-start' : undefined} />
          {#if publishedView === 'uncooked'}Uncooked{/if}
        </Button>
      </div>
    {/if}
  </header>

  {#if cookedDraftReady}
    <div
      class="mb-2 flex items-center gap-2 rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-[10px] text-foreground"
      aria-live="polite"
    >
      <Sparkles class="size-3.5 shrink-0 text-primary" />
      <span class="min-w-0 flex-1 truncate">
        {cookedPreviewModel ? tr('Cooked with {model}; submitting will use this version.', { model: cookedPreviewModel }) : tr('Cooked; submitting will use this version.')}
      </span>
      <Button
        variant="outline"
        size="sm"
        class="h-6 shrink-0 gap-1 px-2 text-[10px]"
        onclick={onRestoreOriginal}
      >
        <Undo2 class="size-3" />
        {tr('Restore original')}
      </Button>
    </div>
  {/if}

  <div class="relative flex min-h-0 flex-1">
    {#if readOnly}
      <MarkdownPreview
        markdown={displayedMarkdown}
        previews={attachmentPreviews}
        {onOpenAttachment}
      />
    {:else if draftEditors.length === 0}
      <RichFeedbackEditor
        bind:this={richEditor}
        markdown={displayedMarkdown}
        previews={attachmentPreviews}
        disabled={editingDisabled}
        {onOpenAttachment}
        getCurrentActionIndex={() => actionChannelFor(visibleRequestId)}
        onChange={(snapshot) => {
          if (!editingDisabled) onChange(snapshot.bodyMarkdown)
        }}
      />
    {:else}
      {#each draftEditors as session (session.requestId)}
        <div
          class={session.requestId === visibleRequestId
            ? 'relative flex min-h-0 flex-1'
            : 'hidden'}
        >
          <SessionDraftEditor
            requestId={session.requestId}
            documentJson={session.initialDocumentJson}
            markdown={session.initialMarkdown}
            previews={attachmentPreviews}
            disabled={session.requestId === visibleRequestId ? editingDisabled : false}
            {onOpenAttachment}
            onReady={captureSessionEditor}
            onChange={(snapshot) => {
              if (session.requestId === visibleRequestId && editingDisabled) return
              onDraftChangeFor(session.requestId, snapshot)
            }}
          />
        </div>
      {/each}
    {/if}

    {#if cooking}
      <div
        class="absolute inset-0 z-10 grid place-items-center rounded-md border bg-background/85 p-6 text-center backdrop-blur-sm"
        aria-live="assertive"
        aria-busy="true"
      >
        <div class="max-w-xs">
          <span class="mx-auto grid size-12 place-items-center rounded-full bg-primary/10 text-primary">
            <ChefHat class="size-6 animate-pulse" />
          </span>
          <strong class="mt-3 block text-sm font-medium">{tr('Cooking…')}</strong>
          <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
            {tr('Organizing the raw feedback while preserving the uncooked source. Please wait.')}
          </p>
          <LoaderCircle class="mx-auto mt-3 size-4 animate-spin text-primary" />
        </div>
      </div>
    {/if}
  </div>

  <footer class="mt-2 flex items-center gap-3 text-[9px] text-muted-foreground">
    <span>{tr('{count} characters', { count: draftBody.length.toLocaleString($locale) })}</span>
    <span>Markdown</span>
    <Badge
      variant={savePhase === 'error' ? 'destructive' : 'secondary'}
      class="ml-auto h-6 gap-1 px-2 text-[9px]"
      aria-live="polite"
    >
      {#if savePhase === 'saving'}
        <LoaderCircle class="size-3 animate-spin" />
      {:else if savePhase === 'error'}
        <AlertCircle class="size-3" />
      {:else if savePhase === 'unsaved'}
        <CloudCog class="size-3" />
      {:else}
        <Check class="size-3" />
      {/if}
      {saveLabel()}
    </Badge>
    <span>{formatTime(workspace.draft.updated_at)}</span>
  </footer>
</section>
