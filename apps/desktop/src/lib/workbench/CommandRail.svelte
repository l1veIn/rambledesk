<script lang="ts">
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { AttachmentView, FeedbackResultView, FeedbackWorkspaceView } from '../feedback'
  import type { SubmitStage } from '../domain/sessionPhases'
  import AttachmentsCard from './AttachmentsCard.svelte'
  import DeliveryCard from './DeliveryCard.svelte'
  import RambelleStatusCard from './RambelleStatusCard.svelte'

  export let workspace: FeedbackWorkspaceView
  export let workDisabled = false
  export let feedbackResult: FeedbackResultView | null = null
  export let rambelleStatusPortrait = ''
  export let rambleEngaged = false
  export let rambleActive = false
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
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onPreviewAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let packageActionLabel = 'Open feedback package'
  export let onOpenResumePrompt: () => void = () => {}
  export let onCookPreview: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onApprove: () => void = () => {}

  $: readOnly =
    workDisabled || workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
  $: interactionLocked = cooking || submitting || cancelling || approving
</script>

<aside
  class="command-rail flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-l bg-muted/15"
  aria-label={t($locale, 'Ramble console')}
>
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
      canSubmit={canSubmit && !workDisabled}
      {cooking}
      {cookingEnabled}
      {cookedDraftReady}
      {submitting}
      {submitStage}
      canCancel={canCancel && !workDisabled}
      {cancelling}
      allowFinish={workspace.request.allow_finish && !workDisabled}
      finalSummary={workspace.request.final_summary ?? ''}
      {approving}
      canOpenResumePrompt={canOpenResumePrompt && !workDisabled}
      onOpenPackage={onOpenPackage}
      {packageActionLabel}
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
