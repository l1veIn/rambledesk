<!-- The right-hand feedback column: the one place a person writes, captures and
     submits feedback. It combines what used to be split between the lower half
     of the document column and the old command rail. -->
<script lang="ts">
  import type { JSONContent } from '@tiptap/core'
  import type { Snippet } from 'svelte'
  import type { AttachmentView, FeedbackResultView, FeedbackWorkspaceView } from '$lib/feedback'
  import type { TidyConfig } from '$lib/lightCleanup'
  import type { DraftOperation } from '$lib/draftOperations'
  import type { FeedbackDraftSnapshot } from '$lib/feedbackDraftDocument'
  import type { SpeechCleanupSegment } from '$lib/speech/speechBlockMetadata'
  import type { SavePhase, SubmitStage } from '../domain/sessionPhases'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import DeliveryCard from './DeliveryCard.svelte'
  import FeedbackEditorPanel from './FeedbackEditorPanel.svelte'
  import RambelleStatusCard from './RambelleStatusCard.svelte'

  export let workspace: FeedbackWorkspaceView
  export let draftBody = ''
  export let editorDocument: JSONContent | null = null
  export let editorEpoch = 0
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let locked = false
  export let cooking = false
  export let cookedDraftReady = false
  export let cookedPreviewModel = ''
  export let cookedPreviewMarkdown = ''
  export let cookedMarkdown = ''
  export let uncookedMarkdown = ''
  export let tidyConfig: TidyConfig | null = null
  export let tidyAutoThreshold = 0
  export let feedbackResult: FeedbackResultView | null = null
  export let canSubmit = false
  export let cookingEnabled = false
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let canCancel = false
  export let cancelling = false
  export let approving = false
  export let canOpenResumePrompt = false
  export let rambelleStatusPortrait = ''
  export let rambleEngaged = false
  export let rambleActive = false
  export let inputTools: Snippet | undefined = undefined
  export let agentStatus: Snippet | undefined = undefined
  export let attachmentBusy = false
  export let formatTime: (value: string | null | undefined) => string
  export let onChange: (snapshot: FeedbackDraftSnapshot) => void = () => {}
  export let onRestoreOriginal: () => void = () => {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onPreviewAttachment: (attachment: AttachmentView) => void = () => {}
  export let onTidyError: (message: string) => void = () => {}
  export let onOpenTidySettings: () => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let packageActionLabel = 'Open feedback package'
  export let onOpenResumePrompt: () => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  let editor: FeedbackEditorPanel

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  export function applyDraftOperation(operation: DraftOperation): boolean {
    return editor?.applyDraftOperation(operation) ?? false
  }

  export function pendingSpeechSegments(): SpeechCleanupSegment[] {
    return editor?.pendingSpeechSegments() ?? []
  }

  export function replaceSpeechSegments(
    replacements: Array<{ segmentId: string; originalText: string; nextText: string }>,
  ): boolean {
    return editor?.replaceSpeechSegments(replacements) ?? false
  }

  export function removeAttachmentReference(attachmentId: string) {
    editor?.removeAttachmentReference(attachmentId)
  }
</script>

<!-- The column divider is the pane resizer itself; a border here would draw a
     second line next to it. -->
<section
  class="feedback-column flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-background"
  aria-label={workspace.request.title}
>
  <!-- The Agent/ACP status belongs to the feedback column, not the workbench header. -->
  {#if agentStatus}
    <div class="shrink-0 border-b bg-muted/15 px-4 py-2" data-feedback-agent-status>
      {@render agentStatus()}
    </div>
  {/if}

  <FeedbackEditorPanel
    bind:this={editor}
    {workspace}
    attachmentCount={workspace.attachments.length}
    {attachmentBusy}
    {onRemoveAttachment}
    {onPreviewAttachment}
    {draftBody}
    {editorDocument}
    {editorEpoch}
    {savedRevision}
    {savePhase}
    {attachmentPreviews}
    {dragActive}
    {locked}
    {cooking}
    {cookedDraftReady}
    {cookedPreviewModel}
    {cookedPreviewMarkdown}
    {cookedMarkdown}
    {uncookedMarkdown}
    {tidyConfig}
    {tidyAutoThreshold}
    {formatTime}
    {inputTools}
    {headerActions}
    {onChange}
    {onRestoreOriginal}
    {onOpenAttachment}
    {onTidyError}
    {onOpenTidySettings}
  />

  {#snippet headerActions()}
    <DeliveryCard
      {feedbackResult}
      cancelled={workspace.request.status === 'cancelled'}
      approved={workspace.request.resolution === 'approved'}
      {canSubmit}
      {cooking}
      {cookingEnabled}
      {cookedDraftReady}
      {submitting}
      {submitStage}
      {canCancel}
      {cancelling}
      allowFinish={workspace.request.allow_finish}
      {approving}
      {canOpenResumePrompt}
      {onOpenPackage}
      {packageActionLabel}
      {onOpenResumePrompt}
      {onCookPreview}
      {onSubmit}
      {onCancel}
      {onApprove}
    />
  {/snippet}

  {#if workspace.request.allow_finish && workspace.request.final_summary}
    <section class="shrink-0 border-t border-primary/25 bg-primary/5 px-4 py-2">
      <strong class="block text-[10px] font-medium">{tr('Agent final summary')}</strong>
      <p class="m-0 mt-1 max-h-24 overflow-y-auto whitespace-pre-wrap text-[10px] leading-4 text-muted-foreground">{workspace.request.final_summary}</p>
    </section>
  {/if}

  <RambelleStatusCard
    portrait={rambelleStatusPortrait}
    feedbackDone={feedbackResult !== null}
    {cooking}
    {rambleEngaged}
    {rambleActive}
  />
</section>
