<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte'
  import { Inbox } from '@lucide/svelte'
  import { Pane, PaneGroup, PaneResizer } from 'paneforge'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import type { JSONContent } from '@tiptap/core'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AttachmentCandidate } from '$lib/capabilities/capturePlugin'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'

  import type {
    AttachmentView,
    FeedbackResultView,
    FeedbackWorkspaceView,
  } from '$lib/feedback'
  import type { TidyConfig } from '$lib/lightCleanup'
  import type { DraftOperation } from '$lib/draftOperations'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import type { SpeechCleanupSegment } from '$lib/speech/speechBlockMetadata'
  import { t } from '$lib/i18n'
  import { autoOpenTaskBrief, locale } from '$lib/preferences'
  import { savePaneLayout, savedPaneLayout } from '$lib/uiPreferences'
  import {
    workspaceViewKey,
    type SessionViewDescriptor,
  } from '$lib/workspace/viewDescriptors'
  import type { HostProfile } from '../domain/hostProfile'
import type {
  RamblePhase,
  SavePhase,
  SubmitStage,
} from '../domain/sessionPhases'
  import { mediaQuery } from '../mediaQuery'
  import CaptureToolsCard from './CaptureToolsCard.svelte'
  import RamblePanel from './RamblePanel.svelte'
  import { nativeCaptureAvailable, voiceRambleAvailable } from '../capabilities/capabilityUi'
  import FeedbackColumn from './FeedbackColumn.svelte'
  import {
    FEEDBACK_PANE_MIN_PERCENT,
    TASK_BRIEF_PANE,
    feedbackColumnLayout,
    feedbackWidthFromLayout,
  } from './feedbackColumnLayout'
  import RequestAttachmentPreview from '../workspace/RequestAttachmentPreview.svelte'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'
  import { canAcceptImagePaste } from './imagePasteAcceptance'

  export let loadingWorkspace = false
  export let readOnly = false
  export let agentStatus: Snippet | undefined = undefined
  export let transport: ApplicationTransport
  export let capabilities: Pick<
    WorkbenchCapabilities,
    | 'serverPaths'
    | 'speech'
    | 'rambleConsole'
    | 'screenCapture'
    | 'clipboardCapture'
    | 'imagePaste'
  >
  export let view: SessionViewDescriptor | null = null
  export let workspace: FeedbackWorkspaceView | null = null
  export let feedbackResult: FeedbackResultView | null = null
  export let taskBriefOpen = true
  export let draftBody = ''
  export let editorDocument: JSONContent | null = null
  export let editorEpoch = 0
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let rambelleStatusPortrait = ''
  export let rambleEngaged = false
  export let rambleActive = false
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleBusy = false
  export let rambleStartedOnce = false
  export let voiceDevice = ''
  export let voiceChunkIndex = 0
  export let voicePartial = ''
  export let voiceLevel = 0
  export let voiceModelMissing = false
  export let rambleMessage = ''
  export let attachmentBusy = false
  export let canSubmit = false
  export let cooking = false
  export let cookingEnabled = false
  export let cookedDraftReady = false
  export let cookedPreviewModel = ''
  export let cookedPreviewMarkdown = ''
  export let tidyConfig: TidyConfig | null = null
  export let tidyAutoThreshold = 0
  export let activeActionId: string | null = null
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let publishedFeedback: { markdown: string; uncooked_markdown?: string } | null = null
  export let canCancel = false
  export let cancelling = false
  export let approving = false
  export let canOpenResumePrompt = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let formatTime: (value: string | null | undefined) => string
  export let onDraftChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onTidyError: (message: string) => void = () => {}
  export let onOpenTidySettings: () => void = () => {}
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onToggleRamble: () => void = () => {}
  export let onExitRamble: () => void = () => {}
  export let onOpenVoiceSettings: () => void = () => {}
  export let onOpenTask: (requestId: string) => void = () => {}
  export let onAutoOpenTask: (requestId: string) => void = () => {}
  export let onStartScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}
  export let onPasteCandidates: (candidates: readonly AttachmentCandidate[]) => boolean = () => false
  export let onPasteError: (cause: unknown) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let packageActionLabel = 'Open feedback package'
  export let onOpenResumePrompt: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  const TASK_BRIEF_DEFAULT_SIZE = TASK_BRIEF_PANE.defaultPercent
  const TASK_BRIEF_MIN_SIZE = TASK_BRIEF_PANE.minPercent
  const TASK_BRIEF_MAX_SIZE = TASK_BRIEF_PANE.maxPercent
  const WORKSPACE_DOCUMENT_LAYOUT_KEY = 'workspace-feedback-column'

  /**
   * Above this width the task brief and the feedback column sit side by side;
   * below it they stack, and the feedback column keeps the whole width.
   */
  const WIDE_COLUMNS_QUERY = '(min-width: 1180px)'
  const wideColumns = mediaQuery(WIDE_COLUMNS_QUERY)
  const savedFeedbackWidth = savedPaneLayout(WORKSPACE_DOCUMENT_LAYOUT_KEY)?.[0] ?? null

  let columnsWidth = 0
  $: columnsLayout = feedbackColumnLayout({
    containerPx: columnsWidth,
    savedPx: savedFeedbackWidth,
  })
  $: briefPanePercent = columnsLayout.brief
  $: feedbackPanePercent = columnsLayout.feedback

  let feedbackEditor: FeedbackColumn | undefined
  let taskBriefPane:
    | {
        collapse: () => void
        expand: () => void
        isCollapsed: () => boolean
      }
    | undefined
  let columnsPaneGroup: { setLayout: (layout: number[]) => void } | undefined
  let columnsLayoutReady = false
  let autoOpenedTaskRequestId = ''
  let workspaceRoot: HTMLElement

  $: if (taskBriefPane) {
    if (taskBriefOpen && taskBriefPane.isCollapsed()) taskBriefPane.expand()
    else if (!taskBriefOpen && !taskBriefPane.isCollapsed()) taskBriefPane.collapse()
  }
  // When enabled, waiting requests open their Task workspace once through the
  // same route as the explicit preview action.
  $: if (
    $autoOpenTaskBrief &&
    !readOnly &&
    workspace &&
    workspace.request.status === 'waiting' &&
    workspace.request.request_id !== autoOpenedTaskRequestId
  ) {
    autoOpenedTaskRequestId = workspace.request.request_id
    onAutoOpenTask(workspace.request.request_id)
  }
  $: interactionLocked = readOnly || cooking || cookedDraftReady || submitting || cancelling || approving

  function saveColumnsLayout(layout: number[]) {
    if (!columnsLayoutReady || !$wideColumns) return
    const width = feedbackWidthFromLayout(layout, columnsWidth)
    if (width !== null) savePaneLayout(WORKSPACE_DOCUMENT_LAYOUT_KEY, [width])
  }

  onMount(() => {
    const imagePaste = capabilities.imagePaste
    const unsubscribePaste = imagePaste.status.availability === 'unavailable'
      ? undefined
      : imagePaste.implementation.subscribe(
          workspaceRoot,
          (candidates) => {
            if (!canAcceptImagePaste({
              loadingWorkspace,
              requestStatus: workspace?.request.status ?? null,
              interactionLocked,
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

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

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
  bind:this={workspaceRoot}
  class="workspace-panel relative flex h-full min-h-0 min-w-0 flex-1 flex-col bg-background"
  data-workspace-view-key={view ? workspaceViewKey(view) : undefined}
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
          bind:this={taskBriefPane}
          id="task-brief-pane"
          class="min-h-0 min-w-0 @container"
          collapsible={!$wideColumns}
          collapsedSize={TASK_BRIEF_MIN_SIZE}
          defaultSize={$wideColumns ? briefPanePercent : TASK_BRIEF_DEFAULT_SIZE}
          minSize={TASK_BRIEF_MIN_SIZE}
          maxSize={$wideColumns ? 100 - FEEDBACK_PANE_MIN_PERCENT : TASK_BRIEF_MAX_SIZE}
          onCollapse={() => (taskBriefOpen = false)}
          onExpand={() => (taskBriefOpen = true)}
        >
          <WorkspaceHeader {workspace} {resolveHostProfile} {cooking} />
          <TaskBriefPanel
            {transport}
            {capabilities}
            bind:open={taskBriefOpen}
            {workspace}
            {activeActionId}
            onSelectAction={(id, index, title) => { if (!readOnly) onSelectAction(id, index, title) }}
            onOpenPreview={() => onOpenTask(workspace!.request.request_id)}
          />
        </Pane>

        <PaneResizer
          class="workbench-pane-resizer workbench-pane-resizer--vertical"
          aria-label={tr('Resize task brief')}
        />

        <Pane
          id="feedback-column-pane"
          class="min-h-0 min-w-0"
          minSize={FEEDBACK_PANE_MIN_PERCENT}
          defaultSize={$wideColumns ? feedbackPanePercent : 100 - TASK_BRIEF_DEFAULT_SIZE}
        >
          <FeedbackColumn
            bind:this={feedbackEditor}
            {workspace}
            {draftBody}
            {editorDocument}
            {editorEpoch}
            {savedRevision}
            {savePhase}
            {attachmentPreviews}
            {dragActive}
            {formatTime}
            {cooking}
            {cookedDraftReady}
            {cookedPreviewModel}
            {cookedPreviewMarkdown}
            locked={interactionLocked}
            cookedMarkdown={publishedFeedback?.markdown ?? ''}
            uncookedMarkdown={publishedFeedback?.uncooked_markdown ?? draftBody}
            {feedbackResult}
            canSubmit={canSubmit && !readOnly}
            {cookingEnabled}
            {submitting}
            {submitStage}
            canCancel={canCancel && !readOnly}
            {cancelling}
            {approving}
            canOpenResumePrompt={canOpenResumePrompt && !readOnly}
            {rambelleStatusPortrait}
            {rambleEngaged}
            {rambleActive}
            {attachmentBusy}
            onChange={onDraftChange}
            {tidyConfig}
            {tidyAutoThreshold}
            onTidyError={onTidyError}
            onOpenTidySettings={onOpenTidySettings}
            onRestoreOriginal={onRestoreOriginal}
            onOpenAttachment={openAttachmentPreviewById}
            {onRemoveAttachment}
            onPreviewAttachment={openAttachmentPreview}
            {agentStatus}
            {onOpenPackage}
            {packageActionLabel}
            {onOpenResumePrompt}
            {onCookPreview}
            {onSubmit}
            {onCancel}
            {onApprove}
          >
            {#snippet inputTools()}
              {#if voiceRambleAvailable(capabilities.speech.status)}
                <RamblePanel
                  {rambleEngaged}
                  {rambleActive}
                  {ramblePhase}
                  {rambleBusy}
                  {rambleStartedOnce}
                  readOnly={interactionLocked}
                  {voiceDevice}
                  {voiceChunkIndex}
                  {voicePartial}
                  {voiceLevel}
                  modelMissing={voiceModelMissing}
                  message={rambleMessage}
                  onToggle={onToggleRamble}
                  onExit={onExitRamble}
                  onOpenVoiceSettings={onOpenVoiceSettings}
                />
              {/if}
              <CaptureToolsCard
                {attachmentBusy}
                readOnly={interactionLocked}
                nativeCaptureAvailable={nativeCaptureAvailable({
                  screenCapture: capabilities.screenCapture.status,
                  clipboardCapture: capabilities.clipboardCapture.status,
                })}
                onScreenCapture={onStartScreenCapture}
                onImportClipboard={onImportClipboard}
                {onFileSelection}
              />
            {/snippet}
          </FeedbackColumn>
        </Pane>
      </PaneGroup>
    </div>

    <RequestAttachmentPreview
      {transport}
      {capabilities}
      bind:open={previewOpen}
      requestId={workspace.request.request_id}
      attachment={previewAttachment}
      readKind="workspace"
    />

  {:else}
    <div class="grid h-full place-items-center p-8 text-center">
      <div class="max-w-xs">
        {#if rambelleStatusPortrait}
          <img
            src={rambelleStatusPortrait}
            alt=""
            class="mx-auto mb-4 size-20 object-contain opacity-90"
          />
        {:else}
          <span class="mx-auto mb-4 grid size-12 place-items-center rounded-md bg-muted text-muted-foreground">
            <Inbox class="size-5" />
          </span>
        {/if}
        <strong class="block text-sm font-medium">{tr('Select a request')}</strong>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('Choose a host, session, and request from the left to open its workspace.')}
        </p>
        {#if agentStatus}<div class="mt-4 text-left">{@render agentStatus()}</div>{/if}
      </div>
    </div>
  {/if}

</section>

<style>
  /* The task brief and the feedback column share one split; PaneForge owns the
     sizes, so this only has to keep the group bounded. */
  .workspace-columns {
    display: flex;
    overflow: hidden;
  }
</style>
