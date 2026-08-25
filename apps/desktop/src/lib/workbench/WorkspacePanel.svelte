<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { Inbox } from '@lucide/svelte'
  import { Pane, PaneGroup, PaneResizer } from 'paneforge'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import type {
    AttachmentView,
    FeedbackResultView,
    FeedbackWorkspaceView,
  } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { savePaneLayout, savedPaneLayout } from '$lib/uiPreferences'
  import type {
    BriefNotePhase,
    FeedbackEditorHandle,
    HostProfile,
    RamblePhase,
    SavePhase,
    SubmitStage,
  } from './types'
  import type { RambleClip } from './briefNotes'
  import CommandRail from './CommandRail.svelte'
  import FeedbackEditorPanel from './FeedbackEditorPanel.svelte'
  import RequestAttachmentPreview from './RequestAttachmentPreview.svelte'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import TaskBriefPreview from './TaskBriefPreview.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'

  export let loadingWorkspace = false
  export let workspace: FeedbackWorkspaceView | null = null
  export let feedbackResult: FeedbackResultView | null = null
  export let taskBriefOpen = true
  export let draftBody = ''
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
  export let rambleClips: RambleClip[] = []
  export let briefNoteProcessingIds: string[] = []
  export let noteTranscript = ''
  export let briefNotes: Record<string, string[]> = {}
  export let briefNotePhase: BriefNotePhase = 'idle'
  export let briefNoteBlockId: string | null = null
  export let onToggleBriefNote: (blockId: string) => void = () => {}
  export let onSaveRambleClip: (clipId: string, text: string) => void = () => {}
  export let onSaveBriefNote: (blockId: string, index: number, text: string) => void = () => {}
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
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let publishedFeedback: { markdown: string; uncooked_markdown?: string } | null = null
  export let canCancel = false
  export let cancelling = false
  export let approving = false
  export let noteBusy = false
  export let canOpenResumePrompt = false
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let formatTime: (value: string | null | undefined) => string
  export let onReload: () => void = () => {}
  export let onDraftChange: (markdown: string) => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onToggleRamble: () => void = () => {}
  export let onExitRamble: () => void = () => {}
  export let onOpenVoiceSettings: () => void = () => {}
  export let onStartScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let onOpenResumePrompt: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  const TASK_BRIEF_DEFAULT_SIZE = 30
  const TASK_BRIEF_MIN_SIZE = 8
  const TASK_BRIEF_MAX_SIZE = 40
  const WORKSPACE_DOCUMENT_LAYOUT_KEY = 'workspace-document-layout'
  const savedDocumentLayout = savedPaneLayout(WORKSPACE_DOCUMENT_LAYOUT_KEY)

  let feedbackEditor: FeedbackEditorHandle | undefined
  let taskBriefPane:
    | {
        collapse: () => void
        expand: () => void
        isCollapsed: () => boolean
      }
    | undefined
  let documentPaneGroup: { setLayout: (layout: number[]) => void } | undefined
  let documentLayoutReady = false
  let taskBriefPreviewOpen = false
  let taskBriefPreviewOrigin: string | null = null
  let autoPreviewedRequestId = ''
  let briefPulseNonce = 0
  let previewWasOpen = false

  $: if (taskBriefPane) {
    if (taskBriefOpen && taskBriefPane.isCollapsed()) taskBriefPane.expand()
    else if (!taskBriefOpen && !taskBriefPane.isCollapsed()) taskBriefPane.collapse()
  }
  // Auto-open the full-screen brief when switching to a request that is still
  // waiting for the human to begin.
  $: if (
    workspace &&
    workspace.request.status === 'waiting' &&
    workspace.request.request_id !== autoPreviewedRequestId
  ) {
    autoPreviewedRequestId = workspace.request.request_id
    taskBriefPreviewOrigin = null
    taskBriefPreviewOpen = true
  }
  $: interactionLocked = cooking || submitting || cancelling || approving
  // Nudge the preview button when the full-screen brief collapses back to it.
  $: {
    const nowOpen = taskBriefPreviewOpen
    if (previewWasOpen && !nowOpen) briefPulseNonce += 1
    previewWasOpen = nowOpen
  }

  function saveDocumentLayout(layout: number[]) {
    if (documentLayoutReady) savePaneLayout(WORKSPACE_DOCUMENT_LAYOUT_KEY, layout)
  }

  onMount(() => {
    void tick().then(() => {
      if (!documentPaneGroup) return
      documentLayoutReady = true
      if (savedDocumentLayout) documentPaneGroup.setLayout(savedDocumentLayout)
    })
  })

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    return feedbackEditor?.insertAttachments(attachments) ?? false
  }

  export function applyExternalMarkdown(markdown: string): boolean {
    return feedbackEditor?.applyExternalMarkdown(markdown) ?? false
  }

  export function appendTranscript(text: string) {
    feedbackEditor?.appendTranscript(text)
  }

  export function appendClipboardCapture(text: string, label: string) {
    return feedbackEditor?.appendClipboardCapture(text, label) ?? false
  }

  export function appendCapturedAttachment(attachment: AttachmentView, label: string) {
    return feedbackEditor?.appendCapturedAttachment(attachment, label) ?? false
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

<section class="workspace-panel relative flex h-full min-h-0 min-w-0 flex-1 flex-col bg-background">
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
    <WorkspaceHeader {workspace} {resolveHostProfile} {cooking} disabled={interactionLocked} onReload={onReload} />

    <div class="workspace-columns min-h-0 flex-1">
      <div class="document-column min-h-0 min-w-0 overflow-hidden @container">
        <PaneGroup
          bind:this={documentPaneGroup}
          direction="vertical"
          class="h-full"
          id="workspace-document-split"
          onLayoutChange={saveDocumentLayout}
        >
          <Pane
            bind:this={taskBriefPane}
            id="task-brief-pane"
            collapsible={true}
            collapsedSize={TASK_BRIEF_MIN_SIZE}
            defaultSize={TASK_BRIEF_DEFAULT_SIZE}
            minSize={TASK_BRIEF_MIN_SIZE}
            maxSize={TASK_BRIEF_MAX_SIZE}
            onCollapse={() => (taskBriefOpen = false)}
            onExpand={() => (taskBriefOpen = true)}
          >
            <TaskBriefPanel
              bind:open={taskBriefOpen}
              {workspace}
              pulseNonce={briefPulseNonce}
              onOpenPreview={(origin) => {
                taskBriefPreviewOrigin = origin
                taskBriefPreviewOpen = true
              }}
            />
          </Pane>

          <PaneResizer
            class="workbench-pane-resizer workbench-pane-resizer--horizontal"
            aria-label={tr('Resize task brief')}
          />

          <Pane id="feedback-editor-pane" minSize={100 - TASK_BRIEF_MAX_SIZE}>
            <FeedbackEditorPanel
              bind:this={feedbackEditor}
              {workspace}
              {draftBody}
              {savedRevision}
              {savePhase}
              {attachmentPreviews}
              {dragActive}
              {formatTime}
              {cooking}
              cookingEnabled={cookingEnabled && !publishedFeedback}
              {cookedDraftReady}
              {cookedPreviewModel}
              locked={interactionLocked}
              cookedMarkdown={publishedFeedback?.markdown ?? ''}
              uncookedMarkdown={publishedFeedback?.uncooked_markdown ?? draftBody}
              onChange={onDraftChange}
              onCookPreview={onCookPreview}
              onRestoreOriginal={onRestoreOriginal}
              onOpenAttachment={openAttachmentPreviewById}
            />
          </Pane>
        </PaneGroup>
      </div>

      <CommandRail
        {workspace}
        {feedbackResult}
        {rambelleStatusPortrait}
        {rambleEngaged}
        {rambleActive}
        {ramblePhase}
        {rambleBusy}
        {rambleStartedOnce}
        {voiceDevice}
        {voiceChunkIndex}
        {voicePartial}
        {voiceLevel}
        {voiceModelMissing}
        {rambleMessage}
        {attachmentBusy}
        {canSubmit}
        {cooking}
        {cookingEnabled}
        {cookedDraftReady}
        {submitting}
        {submitStage}
        {canCancel}
        {cancelling}
        {approving}
        {noteBusy}
        {canOpenResumePrompt}
        {onToggleRamble}
        {onExitRamble}
        {onOpenVoiceSettings}
        {onStartScreenCapture}
        {onImportClipboard}
        {onFileSelection}
        {onRemoveAttachment}
        onPreviewAttachment={openAttachmentPreview}
        {onOpenPackage}
        {onOpenResumePrompt}
        {onCookPreview}
        {onSubmit}
        {onCancel}
        {onApprove}
      />
    </div>

    <RequestAttachmentPreview
      bind:open={previewOpen}
      requestId={workspace.request.request_id}
      attachment={previewAttachment}
      readKind="workspace"
    />

    <TaskBriefPreview
      bind:open={taskBriefPreviewOpen}
      {workspace}
      {formatTime}
      {resolveHostProfile}
      {onToggleRamble}
      {ramblePhase}
      {rambleStartedOnce}
      {rambleBusy}
      {rambleClips}
      {briefNotes}
      {briefNotePhase}
      {briefNoteBlockId}
      {briefNoteProcessingIds}
      {noteTranscript}
      {interactionLocked}
      {onToggleBriefNote}
      {onSaveRambleClip}
      {onSaveBriefNote}
      origin={taskBriefPreviewOrigin}
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
      </div>
    </div>
  {/if}

</section>

<style>
  .workspace-columns {
    display: grid;
    grid-template-columns: minmax(360px, 1fr) 288px;
    overflow: hidden;
  }

  .document-column {
    height: 100%;
  }

  @media (max-width: 1180px) {
    .workspace-columns {
      grid-template-columns: minmax(0, 1fr);
      overflow: auto;
    }

    .document-column {
      height: 680px;
      min-height: 680px;
    }

    :global(.command-rail) {
      min-height: 620px;
      border-top: 1px solid var(--border);
      border-left: 0;
    }
  }
</style>
