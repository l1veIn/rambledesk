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
  import type { JSONContent } from '@tiptap/core'

  import RichFeedbackEditor from '$lib/RichFeedbackEditor.svelte'
  import type { DraftOperation } from '$lib/draftOperations'
  import type { FeedbackWorkspaceView } from '$lib/feedback'
  import {
    decodeFeedbackDraftDocument,
    type FeedbackDraftSnapshot,
  } from '$lib/feedbackDraftDocument'
  import { tidySpeechSegments, type TidyConfig } from '$lib/lightCleanup'
  import {
    speechCleanupCandidates,
    type SpeechCleanupSegment,
  } from '$lib/speechBlockMetadata'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { shouldAutoTidy } from '$lib/tidyAuto'
  import { hasCookedPublishedVariant } from '$lib/publishedFeedback'
  import MarkdownPreview from './MarkdownPreview.svelte'
  import type { SavePhase } from './types'

  export let workspace: FeedbackWorkspaceView
  export let draftBody = ''
  export let editorDocument: JSONContent | null = null
  export let editorEpoch = 0
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let cooking = false
  export let cookedDraftReady = false
  export let cookedPreviewModel = ''
  export let cookedPreviewMarkdown = ''
  export let locked = false
  export let cookedMarkdown = ''
  export let uncookedMarkdown = ''
  export let formatTime: (value: string | null | undefined) => string
  export let onChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let tidyConfig: TidyConfig | null = null
  export let tidyAutoThreshold = 0
  export let onTidyError: (message: string) => void = () => {}
  export let onOpenTidySettings: () => void = () => {}

  let tidyBusy = false
  let pendingCount = 0
  let tidyingSegmentIds: string[] = []
  let lastAutoTidyAttempt = ''

  let richEditor: RichFeedbackEditor
  let publishedView: 'cooked' | 'uncooked' = 'cooked'

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: editingDisabled = readOnly || locked
  $: hasCookedVariant = readOnly && hasCookedPublishedVariant(cookedMarkdown, uncookedMarkdown)
  $: displayedMarkdown =
    cookedDraftReady
      ? cookedPreviewMarkdown
      : hasCookedVariant && publishedView === 'cooked'
      ? cookedMarkdown
      : hasCookedVariant && publishedView === 'uncooked'
        ? uncookedMarkdown
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

  export function applyDraftOperation(operation: DraftOperation): boolean {
    return richEditor?.applyDraftOperation(operation) ?? false
  }

  export function pendingSpeechSegments(): SpeechCleanupSegment[] {
    return richEditor?.pendingSpeechSegments() ?? []
  }

  export function replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean {
    return richEditor?.replaceSpeechSegments(replacements) ?? false
  }

  $: tidyReady = Boolean(tidyConfig?.apiKey.trim() && tidyConfig.model.trim())
  $: {
    editorEpoch
    editorDocument
    const candidates = richEditor?.pendingSpeechSegments() ??
      (editorDocument ? speechCleanupCandidates(editorDocument) : [])
    pendingCount = candidates.length
    const shouldRun = shouldAutoTidy(pendingCount, tidyAutoThreshold)
    if (!shouldRun) {
      lastAutoTidyAttempt = ''
    } else if (richEditor && tidyReady && !tidyBusy && !editingDisabled) {
      const attempt = autoTidyAttemptKey(candidates)
      if (attempt !== lastAutoTidyAttempt) {
        lastAutoTidyAttempt = attempt
        void tidyNow(true)
      }
    }
  }

  function autoTidyAttemptKey(candidates: SpeechCleanupSegment[]): string {
    return [
      workspace.request.request_id,
      String(tidyAutoThreshold),
      ...candidates.map((segment) => segment.segmentId),
    ].join('\u0000')
  }

  async function tidyNow(automatic = false) {
    if (tidyBusy || editingDisabled || !tidyConfig || !tidyReady) {
      if (!automatic && (!tidyConfig || !tidyReady)) {
        onTidyError(tr('Configure Tidy in Settings → Post-processing → Tidy first.'))
        onOpenTidySettings()
      }
      return
    }
    const requestId = workspace.request.request_id
    const epoch = editorEpoch
    const candidates = richEditor?.pendingSpeechSegments() ?? []
    if (candidates.length === 0) return
    lastAutoTidyAttempt = autoTidyAttemptKey(candidates)
    tidyingSegmentIds = candidates.map((segment) => segment.segmentId)
    tidyBusy = true
    try {
      const result = await tidySpeechSegments(candidates, tidyConfig)
      if (workspace.request.request_id !== requestId || editorEpoch !== epoch) return
      if (!result) {
        onTidyError(tr('Tidy did not write back because the model output did not match the original segments.'))
        return
      }
      richEditor?.replaceSpeechSegments(
        candidates.map((segment, index) => ({
          segmentId: segment.segmentId,
          originalText: segment.text,
          nextText: result[index] ?? segment.text,
        })),
      )
    } catch (cause) {
      onTidyError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      tidyingSegmentIds = []
      tidyBusy = false
    }
  }

  export function removeAttachmentReference(attachmentId: string) {
    richEditor?.removeAttachmentReference(attachmentId)
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
    {:else if !cookedDraftReady && !readOnly}
      <Button
        variant={pendingCount > 0 ? 'secondary' : 'ghost'}
        size="sm"
        class="ml-auto h-7 shrink-0 gap-1 px-2 text-[10px]"
        aria-label={tr('Tidy')}
        title={pendingCount > 0
          ? tr('Tidy {count} pending speech segments', { count: pendingCount })
          : tr('Tidy pending speech segments. It appears here after Ramble writes a transcript.')}
        disabled={editingDisabled || cooking || tidyBusy || pendingCount === 0}
        onclick={() => void tidyNow()}
      >
        {#if tidyBusy}
          <LoaderCircle class="size-3.5 animate-spin" />
        {:else}
          <Sparkles class="size-3.5" />
        {/if}
        {tidyBusy ? tr('Tidying…') : tr('Tidy')}
        {#if pendingCount > 0}
          <Badge variant="secondary" class="h-4 px-1 text-[9px]">{pendingCount}</Badge>
        {/if}
      </Button>
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
    {#if cookedDraftReady || hasCookedVariant}
      <MarkdownPreview markdown={displayedMarkdown} previews={attachmentPreviews} {onOpenAttachment} />
    {:else}
      <RichFeedbackEditor
        bind:this={richEditor}
        document={editorDocument}
        {editorEpoch}
        markdown={draftBody}
        previews={attachmentPreviews}
        disabled={editingDisabled}
        {onOpenAttachment}
        {tidyingSegmentIds}
        onChange={(snapshot) => {
          const doc = decodeFeedbackDraftDocument(snapshot.documentJson)
          pendingCount = doc ? speechCleanupCandidates(doc).length : 0
          if (!editingDisabled) onChange(snapshot)
        }}
      />
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
