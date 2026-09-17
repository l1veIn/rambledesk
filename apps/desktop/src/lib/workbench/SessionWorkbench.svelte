<!--
  Ramble session view: a registered workbench (here: Ramble) inside the common
  workbench container. This file only wires the App's props to those two pieces;
  the layout, the feedback column and the submission flow live in the container.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
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
  import CaptureToolsCard from './CaptureToolsCard.svelte'
  import RamblePanel from './RamblePanel.svelte'
  import { nativeCaptureAvailable, voiceRambleAvailable } from '../capabilities/capabilityUi'
  import RambleWorkbench from './RambleWorkbench.svelte'
  import WorkbenchContainer from './WorkbenchContainer.svelte'

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

  $: interactionLocked = readOnly || cooking || cookedDraftReady || submitting || cancelling || approving

  let container: WorkbenchContainer

  // The full-screen task view is hidden for now (see TaskBriefPanel): nothing
  // opens it automatically. `workspaceNavigation.autoOpenTaskView`, the
  // preference and its Settings toggle stay for when it returns.

  export function applyDraftOperation(operation: DraftOperation): boolean {
    return container?.applyDraftOperation(operation) ?? false
  }

  export function pendingSpeechSegments(): SpeechCleanupSegment[] {
    return container?.pendingSpeechSegments() ?? []
  }

  export function replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean {
    return container?.replaceSpeechSegments(replacements) ?? false
  }

  export function removeAttachmentReference(attachmentId: string) {
    container?.removeAttachmentReference(attachmentId)
  }
</script>

<div class="flex h-full min-h-0 min-w-0 flex-1 flex-col" data-workspace-view-key={view ? workspaceViewKey(view) : undefined}>
  <WorkbenchContainer
    bind:this={container}
    {workspace}
    {transport}
    {capabilities}
    {loadingWorkspace}
    {readOnly}
    locked={interactionLocked}
    {attachmentBusy}
    {draftBody}
    {editorDocument}
    {editorEpoch}
    {savedRevision}
    {savePhase}
    {attachmentPreviews}
    {dragActive}
    {formatTime}
    {feedbackResult}
    {cooking}
    {cookingEnabled}
    {cookedDraftReady}
    {cookedPreviewModel}
    {cookedPreviewMarkdown}
    {publishedFeedback}
    canSubmit={canSubmit && !readOnly}
    {submitting}
    {submitStage}
    canCancel={canCancel && !readOnly}
    {cancelling}
    {approving}
    canOpenResumePrompt={canOpenResumePrompt && !readOnly}
    {tidyConfig}
    {tidyAutoThreshold}
    {rambelleStatusPortrait}
    {rambleEngaged}
    {rambleActive}
    {agentStatus}
    onDraftChange={onDraftChange}
    {onTidyError}
    {onOpenTidySettings}
    {onRestoreOriginal}
    {onRemoveAttachment}
    {onPasteCandidates}
    {onPasteError}
    {onOpenPackage}
    {packageActionLabel}
    {onOpenResumePrompt}
    {onCookPreview}
    {onSubmit}
    {onCancel}
    {onApprove}
  >
    {#snippet workbench()}
      {#if workspace}
        <RambleWorkbench
          {workspace}
          {transport}
          {capabilities}
          {resolveHostProfile}
          {readOnly}
          {cooking}
          {activeActionId}
          bind:open={taskBriefOpen}
          onOpenFullView={() => onOpenTask(workspace!.request.request_id)}
          {onSelectAction}
        />
      {/if}
    {/snippet}

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
  </WorkbenchContainer>
</div>
