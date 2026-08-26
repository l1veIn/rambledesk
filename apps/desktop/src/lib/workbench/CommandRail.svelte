<script lang="ts">
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { AttachmentView, FeedbackResultView, FeedbackWorkspaceView } from '../feedback'
  import type { RamblePhase, SubmitStage } from './types'
  import AttachmentsCard from './AttachmentsCard.svelte'
  import CaptureToolsCard from './CaptureToolsCard.svelte'
  import DeliveryCard from './DeliveryCard.svelte'
  import RambelleStatusCard from './RambelleStatusCard.svelte'
  import RamblePanel from './RamblePanel.svelte'

  export let workspace: FeedbackWorkspaceView
  export let feedbackResult: FeedbackResultView | null = null
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
  export let submitting = false
  export let submitStage: SubmitStage = 'idle'
  export let canCancel = false
  export let cancelling = false
  export let approving = false
  export let canOpenResumePrompt = false
  export let onToggleRamble: () => void = () => {}
  export let onExitRamble: () => void = () => {}
  export let foreignRambleTitle = ''
  export let onReturnToRamble: () => void = () => {}
  export let onHandoffRamble: () => void = () => {}
  export let onOpenVoiceSettings: () => void = () => {}
  export let onStartScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onPreviewAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let onOpenResumePrompt: () => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: interactionLocked = cooking || submitting || cancelling || approving
</script>

<aside
  class="command-rail flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-l bg-muted/15"
  aria-label={t($locale, 'Ramble console')}
>
  {#if !readOnly && !interactionLocked}
    <div class="shrink-0">
      <RamblePanel
        {rambleEngaged}
        {rambleActive}
        {ramblePhase}
        {rambleBusy}
        {rambleStartedOnce}
        {readOnly}
        {voiceDevice}
        {voiceChunkIndex}
        {voicePartial}
        {voiceLevel}
        modelMissing={voiceModelMissing}
        message={rambleMessage}
        {foreignRambleTitle}
        onToggle={onToggleRamble}
        onExit={onExitRamble}
        onReturn={onReturnToRamble}
        onHandoff={onHandoffRamble}
        onOpenVoiceSettings={onOpenVoiceSettings}
      />

      <CaptureToolsCard
        attachmentCount={workspace.attachments.length}
        {attachmentBusy}
        {readOnly}
        onScreenCapture={onStartScreenCapture}
        onImportClipboard={onImportClipboard}
        {onFileSelection}
      />
    </div>
  {/if}

  <AttachmentsCard
    attachments={workspace.attachments}
    {attachmentBusy}
    readOnly={readOnly || interactionLocked}
    onRemove={onRemoveAttachment}
    onPreview={onPreviewAttachment}
  />

  <div class="shrink-0">
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
      finalSummary={workspace.request.final_summary ?? ''}
      {approving}
      {canOpenResumePrompt}
      onOpenPackage={onOpenPackage}
      onOpenResumePrompt={onOpenResumePrompt}
      onCookPreview={onCookPreview}
      onSubmit={onSubmit}
      onCancel={onCancel}
      onApprove={onApprove}
    />
    <RambelleStatusCard
      portrait={rambelleStatusPortrait}
      feedbackDone={feedbackResult !== null}
      {cooking}
      {rambleEngaged}
      {rambleActive}
    />
  </div>
</aside>
