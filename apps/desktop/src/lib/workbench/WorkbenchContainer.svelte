<!--
  Common workbench container.

  Owns everything a registered workbench shares: the workspace panel shell and
  its loading state, the column split with the feedback width policy, the generic
  feedback column (editor, tools, save state, delivery and Rambelle line), the
  attachment preview dialog and the paste subscription. The workbench itself is
  passed in as a snippet and only renders its own task content; it never owns the
  document, submission or host protocol, and this container never knows what kind
  of workbench it is presenting.
-->
<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte'
  import { Pane, PaneGroup, PaneResizer } from 'paneforge'

  import { Skeleton } from '$lib/components/ui/skeleton'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AttachmentCandidate } from '$lib/capabilities/capturePlugin'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { AttachmentView, FeedbackResultView, FeedbackWorkspaceView } from '$lib/feedback'
  import type { DraftOperation } from '$lib/draftOperations'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import type { TidyConfig } from '$lib/lightCleanup'
  import type { SpeechCleanupSegment } from '$lib/speech/speechBlockMetadata'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { savePaneLayout, savedPaneLayout } from '$lib/uiPreferences'
  import { mediaQuery } from '../mediaQuery'
  import type { SavePhase, SubmitStage } from '../domain/sessionPhases'
  import { canAcceptImagePaste } from './imagePasteAcceptance'
  import FeedbackColumn from './FeedbackColumn.svelte'
  import RequestAttachmentPreview from '../workspace/RequestAttachmentPreview.svelte'
  import {
    FEEDBACK_PANE_MIN_PERCENT,
    TASK_BRIEF_PANE,
    feedbackColumnLayout,
    feedbackWidthFromLayout,
  } from './feedbackColumnLayout'

  export let workspace: FeedbackWorkspaceView | null = null
  export let transport: ApplicationTransport
  export let capabilities: Pick<
    WorkbenchCapabilities,
    'serverPaths' | 'imagePaste'
  >
  export let loadingWorkspace = false
  export let readOnly = false
  /** Editing is locked by a running operation (cooking, submitting, cancelling, approving). */
  export let locked = false
  export let attachmentBusy = false

  export let draftBody = ''
  export let editorDocument: import('@tiptap/core').JSONContent | null = null
  export let editorEpoch = 0
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let formatTime: (value: string | null | undefined) => string

  export let feedbackResult: FeedbackResultView | null = null
  export let cooking = false
  export let cookingEnabled = false
  export let cookedDraftReady = false
  export let cookedPreviewModel = ''
  export let cookedPreviewMarkdown = ''
  export let publishedFeedback: { markdown: string; uncooked_markdown?: string } | null = null
  export let canSubmit = false
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let canCancel = false
  export let cancelling = false
  export let approving = false
  export let canOpenResumePrompt = false

  export let tidyConfig: TidyConfig | null = null
  export let tidyAutoThreshold = 0

  export let rambelleStatusPortrait = ''
  export let rambleEngaged = false
  export let rambleActive = false

  export let onDraftChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onTidyError: (message: string) => void = () => {}
  export let onOpenTidySettings: () => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onPasteCandidates: (candidates: readonly AttachmentCandidate[]) => boolean = () => false
  export let onPasteError: (cause: unknown) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let packageActionLabel = 'Open feedback package'
  export let onOpenResumePrompt: () => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  export let workbench: Snippet
  export let inputTools: Snippet | undefined = undefined
  export let agentStatus: Snippet | undefined = undefined

  const TASK_BRIEF_DEFAULT_SIZE = TASK_BRIEF_PANE.defaultPercent
  const TASK_BRIEF_MIN_SIZE = TASK_BRIEF_PANE.minPercent
  const TASK_BRIEF_MAX_SIZE = TASK_BRIEF_PANE.maxPercent
  const FEEDBACK_COLUMN_LAYOUT_KEY = 'workspace-feedback-column'

  /**
   * Above this width the workbench and the feedback column sit side by side;
   * below it they stack and the feedback column keeps the whole width.
   */
  const WIDE_COLUMNS_QUERY = '(min-width: 1180px)'
  const wideColumns = mediaQuery(WIDE_COLUMNS_QUERY)
  const savedFeedbackWidth = savedPaneLayout(FEEDBACK_COLUMN_LAYOUT_KEY)?.[0] ?? null

  let columnsWidth = 0
  $: columnsLayout = feedbackColumnLayout({
    containerPx: columnsWidth,
    savedPx: savedFeedbackWidth,
  })
  $: workbenchPanePercent = columnsLayout.brief
  $: feedbackPanePercent = columnsLayout.feedback

  let feedbackEditor: FeedbackColumn | undefined
  let workbenchPane: { isCollapsed: () => boolean } | undefined
  let columnsPaneGroup: { setLayout: (layout: number[]) => void } | undefined
  let columnsLayoutReady = false
  let containerRoot: HTMLElement

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function saveColumnsLayout(layout: number[]) {
    if (!columnsLayoutReady || !$wideColumns) return
    const width = feedbackWidthFromLayout(layout, columnsWidth)
    if (width !== null) savePaneLayout(FEEDBACK_COLUMN_LAYOUT_KEY, [width])
  }

  onMount(() => {
    const imagePaste = capabilities.imagePaste
    const unsubscribePaste = imagePaste.status.availability === 'unavailable'
      ? undefined
      : imagePaste.implementation.subscribe(
          containerRoot,
          (candidates) => {
            if (!canAcceptImagePaste({
              loadingWorkspace,
              requestStatus: workspace?.request.status ?? null,
              interactionLocked: readOnly || locked,
              attachmentBusy,
            })) return false
            return onPasteCandidates(candidates)
          },
          onPasteError,
        )
    void tick().then(() => {
      columnsLayoutReady = true
    })
    return () => unsubscribePaste?.()
  })

  export function applyDraftOperation(operation: DraftOperation): boolean {
    return feedbackEditor?.applyDraftOperation(operation) ?? false
  }

  export function pendingSpeechSegments(): SpeechCleanupSegment[] {
    return feedbackEditor?.pendingSpeechSegments() ?? []
  }

  export function replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean {
    return feedbackEditor?.replaceSpeechSegments(replacements) ?? false
  }

  export function removeAttachmentReference(attachmentId: string) {
    feedbackEditor?.removeAttachmentReference(attachmentId)
  }

  let previewOpen = false
  let previewAttachment: AttachmentView | null = null

  function openAttachmentPreview(attachment: AttachmentView) {
    previewAttachment = attachment
    previewOpen = true
  }

  function openAttachmentPreviewById(attachmentId: string) {
    const attachment = workspace?.attachments.find(
      (item) => item.attachment_id === attachmentId,
    )
    if (attachment) openAttachmentPreview(attachment)
  }
</script>

<section
  bind:this={containerRoot}
  class="workspace-panel relative flex h-full min-h-0 min-w-0 flex-1 flex-col bg-background"
  style:--workspace-feedback-width={$wideColumns ? `${columnsLayout.feedback}%` : null}
>
  {#if loadingWorkspace}
    <div class="grid h-full min-h-0 grid-rows-[64px_1fr]">
      <div class="flex items-center gap-3 border-b px-5">
        <Skeleton class="h-4 w-52" />
        <Skeleton class="ml-auto size-7" />
      </div>
      <div class="grid gap-4 p-5">
        <Skeleton class="h-12 w-full" />
        <Skeleton class="h-full min-h-80 w-full" />
      </div>
    </div>
  {:else if workspace}
    <div class="workspace-columns min-h-0 flex-1" bind:clientWidth={columnsWidth}>
      <PaneGroup
        bind:this={columnsPaneGroup}
        direction={$wideColumns ? 'horizontal' : 'vertical'}
        class="h-full"
        id="workspace-columns-split"
        onLayoutChange={saveColumnsLayout}
      >
        <Pane
          bind:this={workbenchPane}
          id="workbench-pane"
          class="min-h-0 min-w-0 @container"
          collapsible={false}
          collapsedSize={TASK_BRIEF_MIN_SIZE}
          defaultSize={$wideColumns ? workbenchPanePercent : 100 - TASK_BRIEF_DEFAULT_SIZE}
          minSize={TASK_BRIEF_MIN_SIZE}
          maxSize={$wideColumns ? 100 - FEEDBACK_PANE_MIN_PERCENT : TASK_BRIEF_MAX_SIZE}
        >
          {@render workbench()}
        </Pane>

        <PaneResizer
          class="workbench-pane-resizer workbench-pane-resizer--vertical"
          aria-label={tr('Resize feedback column')}
        />

        <Pane
          id="feedback-column-pane"
          class="min-h-0 min-w-0"
          minSize={FEEDBACK_PANE_MIN_PERCENT}
          defaultSize={$wideColumns ? feedbackPanePercent : TASK_BRIEF_DEFAULT_SIZE}
        >
          <FeedbackColumn
            bind:this={feedbackEditor}
            {workspace}
            {attachmentBusy}
            {onRemoveAttachment}
            onPreviewAttachment={openAttachmentPreview}
            {draftBody}
            {editorDocument}
            {editorEpoch}
            {savedRevision}
            {savePhase}
            {attachmentPreviews}
            {dragActive}
            locked={readOnly || locked}
            {cooking}
            {cookedDraftReady}
            {cookedPreviewModel}
            {cookedPreviewMarkdown}
            cookedMarkdown={publishedFeedback?.markdown ?? ''}
            uncookedMarkdown={publishedFeedback?.uncooked_markdown ?? draftBody}
            {feedbackResult}
            {canSubmit}
            {cookingEnabled}
            {submitting}
            {submitStage}
            {canCancel}
            {cancelling}
            {approving}
            {canOpenResumePrompt}
            {rambelleStatusPortrait}
            {rambleEngaged}
            {rambleActive}
            {agentStatus}
            {formatTime}
            {tidyConfig}
            {tidyAutoThreshold}
            {inputTools}
            onChange={onDraftChange}
            onTidyError={onTidyError}
            onOpenTidySettings={onOpenTidySettings}
            onRestoreOriginal={onRestoreOriginal}
            onOpenAttachment={openAttachmentPreviewById}
            {onOpenPackage}
            {packageActionLabel}
            {onOpenResumePrompt}
            {onCookPreview}
            {onSubmit}
            {onCancel}
            {onApprove}
          />
        </Pane>
      </PaneGroup>
    </div>
  {/if}
</section>

{#if workspace}
  <RequestAttachmentPreview
    {transport}
    {capabilities}
    bind:open={previewOpen}
    requestId={workspace.request.request_id}
    attachment={previewAttachment}
    readKind="workspace"
  />
{/if}
