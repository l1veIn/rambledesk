<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { Inbox } from '@lucide/svelte'
  import { Pane, PaneGroup, PaneResizer } from 'paneforge'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import type { JSONContent } from '@tiptap/core'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'

  import type {
    AttachmentView,
    FeedbackResultView,
    FeedbackWorkspaceView,
  } from '$lib/feedback'
  import type { TidyConfig } from '$lib/lightCleanup'
  import type { DraftOperation } from '$lib/draftOperations'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import type { SpeechCleanupSegment } from '$lib/speechBlockMetadata'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { savePaneLayout, savedPaneLayout } from '$lib/uiPreferences'
  import {
    workspaceViewKey,
    type SessionViewDescriptor,
  } from '$lib/workspace/viewDescriptors'
  import type {
    FeedbackEditorHandle,
    HostProfile,
    RamblePhase,
    SavePhase,
    SubmitStage,
  } from './types'
  import CommandRail from './CommandRail.svelte'
  import FeedbackEditorPanel from './FeedbackEditorPanel.svelte'
  import RequestAttachmentPreview from './RequestAttachmentPreview.svelte'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'

  export let loadingWorkspace = false
  export let transport: ApplicationTransport
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
  export let onReload: () => void = () => {}
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
  let autoOpenedTaskRequestId = ''

  $: if (taskBriefPane) {
    if (taskBriefOpen && taskBriefPane.isCollapsed()) taskBriefPane.expand()
    else if (!taskBriefOpen && !taskBriefPane.isCollapsed()) taskBriefPane.collapse()
  }
  // Waiting requests open their Task workspace once through the same route as
  // the explicit preview action.
  $: if (
    workspace &&
    workspace.request.status === 'waiting' &&
    workspace.request.request_id !== autoOpenedTaskRequestId
  ) {
    autoOpenedTaskRequestId = workspace.request.request_id
    onAutoOpenTask(workspace.request.request_id)
  }
  $: interactionLocked = cooking || cookedDraftReady || submitting || cancelling || approving

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
  class="workspace-panel relative flex h-full min-h-0 min-w-0 flex-1 flex-col bg-background"
  data-workspace-view-key={view ? workspaceViewKey(view) : undefined}
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
              {transport}
              bind:open={taskBriefOpen}
              {workspace}
              {activeActionId}
              onSelectAction={onSelectAction}
              onOpenPreview={() => onOpenTask(workspace!.request.request_id)}
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
              onChange={onDraftChange}
              {tidyConfig}
              {tidyAutoThreshold}
              onTidyError={onTidyError}
              onOpenTidySettings={onOpenTidySettings}
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
      {transport}
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
