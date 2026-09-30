<!--
  Ramble session view: a registered workbench (here: Ramble) inside the common
  workbench container. This file only wires the App's props to those two pieces;
  the layout, the feedback column and the submission flow live in the container.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import { writable } from 'svelte/store'
  import type { JSONContent } from '@tiptap/core'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { AttachmentCandidate } from '$lib/capabilities/capturePlugin'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'

  import type {
    AttachmentView,
    FeedbackResultView,
    FeedbackWorkspaceView,
  } from '$lib/feedback'
  import type { DraftOperation } from '$lib/draftOperations'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import type { SpeechCleanupSegment } from '$lib/speech/speechBlockMetadata'
  import { reconcileFieldSpeechSegments } from '$lib/speech/fieldSpeechSegments'
  import {
    workspaceViewKey,
    type SessionViewDescriptor,
  } from '$lib/workspace/viewDescriptors'
  import type { HostProfile } from '../domain/hostProfile'
  import type {
    SavePhase,
    SubmitStage,
  } from '../domain/sessionPhases'
  import { provideInputTools, type InputToolsState } from '../input/inputToolsContext'
  import { removeWorkbenchAttachmentReferences } from '../input/fieldAttachmentText'
  import type { InputTarget } from '../domain/inputTarget'
  import RequestAttachmentPreview from '../workspace/RequestAttachmentPreview.svelte'
  import RegisteredWorkbench from './RegisteredWorkbench.svelte'
  import type { WorkbenchState } from '../generated/feedback'
  import { readWorkbenchState, withWorkbenchState } from '../workbenchState'
  import { applyFeedbackDraftSnapshot, snapshotFeedbackDraftDocument } from '../feedbackDraftDocument'
  import { workbenchIsReadOnly } from '../workbenchPolicy'
  import WorkbenchContainer from './WorkbenchContainer.svelte'

  export let loadingWorkspace = false
  export let readOnly = false
  export let agentStatus: Snippet | undefined = undefined
  export let inputActions: Snippet | undefined = undefined
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
  export let draftBody = ''
  export let draftDocumentJson: string | undefined = undefined
  export let editorDocument: JSONContent | null = null
  export let editorEpoch = 0
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let rambelleStatusPortrait = ''
  export let rambleEngaged = false
  export let rambleActive = false
  export let attachmentBusy = false
  export let canSubmit = false
  export let cooking = false
  export let cookingEnabled = false
  export let cookedDraftReady = false
  export let cookedPreviewModel = ''
  export let cookedPreviewMarkdown = ''
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
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onStartScreenCapture: (target?: InputTarget) => void | Promise<void> = () => {}
  export let onImportClipboard: (target?: InputTarget) => void | Promise<void> = () => {}
  export let onFiles: (files: readonly File[], target?: InputTarget) => void | Promise<void> = () => {}
  export let onPasteCandidates: (candidates: readonly AttachmentCandidate[]) => boolean = () => false
  export let onPasteError: (cause: unknown) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let packageActionLabel = 'Open feedback package'
  export let onOpenResumePrompt: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  $: unsupported = workbenchIsReadOnly(workspace?.workbench)
  $: feedbackReadOnly = readOnly || unsupported
  $: interactionLocked = feedbackReadOnly || cooking || cookedDraftReady || submitting || cancelling || approving

  let inputAttachmentOpen = false
  let inputAttachment: AttachmentView | null = null
  const inputToolsState = writable<InputToolsState>({ requestId: '', disabled: true, busy: false, canCapture: false, canPaste: false, attachments: [] })
  provideInputTools({
    state: inputToolsState,
    capture: (target) => onStartScreenCapture(target),
    paste: (target) => onImportClipboard(target),
    files: (target, files) => onFiles(files, target),
    preview: (id) => {
      inputAttachment = workspace?.attachments.find((item) => item.attachment_id === id) ?? null
      inputAttachmentOpen = !!inputAttachment
    },
    reportError: (cause) => onPasteError(cause),
  })
  $: inputToolsState.set({
    requestId: workspace?.request.request_id ?? '', disabled: interactionLocked || workspace?.request.status === 'completed' || workspace?.request.status === 'cancelled',
    busy: attachmentBusy, canCapture: capabilities.screenCapture.status.availability !== 'unavailable',
    canPaste: capabilities.clipboardCapture.status.availability !== 'unavailable', attachments: workspace?.attachments ?? [],
  })

  let container: WorkbenchContainer
  let interactionState: WorkbenchState | null = null
  let currentSnapshot: FeedbackDraftSnapshot = { documentJson: '{"schemaVersion":2,"doc":{"type":"doc","content":[]}}', bodyMarkdown: '' }
  $: loadDraft(workspace?.request.request_id, draftDocumentJson ?? workspace?.draft.document_json, editorEpoch)
  function loadDraft(_requestId: string | undefined, documentJson: string | null | undefined, _epoch: number) {
    interactionState = readWorkbenchState(documentJson)
    currentSnapshot = documentJson ? { documentJson, bodyMarkdown: draftBody }
      : snapshotFeedbackDraftDocument(editorDocument ?? { type: 'doc', content: [{ type: 'paragraph' }] })
  }
  function draftChanged(snapshot: FeedbackDraftSnapshot) {
    if (interactionLocked) return
    currentSnapshot = applyFeedbackDraftSnapshot(currentSnapshot, snapshot)
    onDraftChange(currentSnapshot)
  }
  function interactionChanged(state: WorkbenchState) {
    if (interactionLocked || workspace?.request.status === 'completed' || workspace?.request.status === 'cancelled') return
    interactionState = state
    currentSnapshot = reconcileFieldSpeechSegments(currentSnapshot, withWorkbenchState(currentSnapshot, state))
    onDraftChange(currentSnapshot)
  }

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
    const next = removeWorkbenchAttachmentReferences(currentSnapshot, attachmentId)
    if (next !== currentSnapshot) {
      const reconciled = reconcileFieldSpeechSegments(currentSnapshot, next)
      interactionState = readWorkbenchState(reconciled.documentJson)
      draftChanged(reconciled)
    }
  }
</script>

<div class="flex h-full min-h-0 min-w-0 flex-1 flex-col" data-workspace-view-key={view ? workspaceViewKey(view) : undefined}>
  <WorkbenchContainer
    bind:this={container}
    expandable={!unsupported && (workspace?.workbench?.type === 'web_review' || workspace?.workbench?.type === 'document_review')}
    initiallyExpanded={workspace?.workbench?.type === 'web_review'}
    {workspace}
    {transport}
    {capabilities}
    {loadingWorkspace}
    readOnly={feedbackReadOnly}
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
    canSubmit={canSubmit && !feedbackReadOnly}
    {submitting}
    {submitStage}
    canCancel={canCancel && !readOnly}
    {cancelling}
    {approving}
    canOpenResumePrompt={canOpenResumePrompt && !readOnly}
    {rambelleStatusPortrait}
    {rambleEngaged}
    {rambleActive}
    {agentStatus}
    {inputActions}
    onDraftChange={draftChanged}
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
        <RegisteredWorkbench
          {workspace}
          {transport}
          {capabilities}
          {resolveHostProfile}
          readOnly={feedbackReadOnly}
          locked={interactionLocked}
          state={interactionState}
          onStateChange={interactionChanged}
          {cooking}
          {activeActionId}
          {onSelectAction}
        />
      {/if}
    {/snippet}

  </WorkbenchContainer>
</div>

{#if workspace}
  <RequestAttachmentPreview {transport} {capabilities} bind:open={inputAttachmentOpen}
    requestId={workspace.request.request_id} attachment={inputAttachment} readKind="workspace" />
{/if}
