<script lang="ts">
  import type { AttachmentView, FeedbackResultView, FeedbackWorkspaceView } from '../feedback'
  import type { RamblePhase } from './types'
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
  export let submitting = false
  export let canCancel = false
  export let cancelling = false
  export let onToggleRamble: () => void = () => {}
  export let onExitRamble: () => void = () => {}
  export let onOpenVoiceSettings: () => void = () => {}
  export let onStartScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}
  export let onInsertAttachment: (attachment: AttachmentView) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}

  $: readOnly =
    workspace.request.status === 'completed' || workspace.request.status === 'cancelled'
</script>

<aside
  class="command-rail min-h-0 min-w-0 overflow-y-auto border-l bg-muted/15"
  aria-label="Ramble 操作台"
>
  {#if !readOnly}
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
      onToggle={onToggleRamble}
      onExit={onExitRamble}
      onOpenVoiceSettings={onOpenVoiceSettings}
    />

    <CaptureToolsCard
      attachmentCount={workspace.attachments.length}
      {attachmentBusy}
      {rambleEngaged}
      {readOnly}
      onScreenCapture={onStartScreenCapture}
      onImportClipboard={onImportClipboard}
      {onFileSelection}
    />
  {/if}

  <AttachmentsCard
    attachments={workspace.attachments}
    {attachmentBusy}
    {readOnly}
    onInsert={onInsertAttachment}
    onRemove={onRemoveAttachment}
  />

  <RambelleStatusCard
    portrait={rambelleStatusPortrait}
    feedbackDone={feedbackResult !== null}
    {rambleEngaged}
  />

  <DeliveryCard
    {feedbackResult}
    cancelled={workspace.request.status === 'cancelled'}
    {canSubmit}
    {submitting}
    {canCancel}
    {cancelling}
    onOpenPackage={onOpenPackage}
    onSubmit={onSubmit}
    onCancel={onCancel}
  />
</aside>
