<script lang="ts">
  import type {
    AttachmentView,
    FeedbackResultView,
    FeedbackWorkspaceView,
  } from '../feedback'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import type {
    AdapterPresentation,
    FeedbackEditorHandle,
    RamblePhase,
    SavePhase,
  } from './types'
  import CommandRail from './CommandRail.svelte'
  import FeedbackEditorPanel from './FeedbackEditorPanel.svelte'
  import TaskBriefPanel from './TaskBriefPanel.svelte'
  import WorkspaceHeader from './WorkspaceHeader.svelte'

  export let loadingWorkspace = false
  export let workspace: FeedbackWorkspaceView | null = null
  export let feedbackResult: FeedbackResultView | null = null
  export let pageError = ''
  export let taskBriefOpen = true
  export let draftBody = ''
  export let savedRevision = 0
  export let savePhase: SavePhase = 'idle'
  export let attachmentPreviews: Record<string, string> = {}
  export let dragActive = false
  export let attachmentMessage = ''
  export let saveMessage = ''
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
  export let rambleMessage = ''
  export let attachmentBusy = false
  export let canSubmit = false
  export let submitting = false
  export let canCancel = false
  export let cancelling = false
  export let adapterPresentation: (hostId: string) => AdapterPresentation
  export let formatTime: (value: string | null | undefined) => string
  export let onReload: () => void = () => {}
  export let onDraftChange: (markdown: string) => void = () => {}
  export let onToggleRamble: () => void = () => {}
  export let onExitRamble: () => void = () => {}
  export let onStartScreenCapture: () => void = () => {}
  export let onImportClipboard: () => void = () => {}
  export let onFileSelection: (event: Event) => void = () => {}
  export let onInsertAttachment: (attachment: AttachmentView) => void = () => {}
  export let onRemoveAttachment: (attachment: AttachmentView) => void = () => {}
  export let onOpenPackage: () => void = () => {}
  export let onSubmit: () => void = () => {}
  export let onCancel: () => void = () => {}

  let feedbackEditor: FeedbackEditorHandle | undefined

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  export function insertAttachments(attachments: AttachmentView[]) {
    return feedbackEditor?.insertAttachments(attachments) ?? false
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
</script>

<section class="workspace-panel">
  {#if loadingWorkspace}
    <div class="workspace-placeholder">{tr('正在打开反馈工作区…')}</div>
  {:else if workspace}
    <div class="workspace-stage">
      <WorkspaceHeader {workspace} {adapterPresentation} onReload={onReload} />

      <div class="workspace-columns">
        <div class="document-column">
          <TaskBriefPanel bind:open={taskBriefOpen} {workspace} />

          <FeedbackEditorPanel
            bind:this={feedbackEditor}
            {workspace}
            {draftBody}
            {savedRevision}
            {savePhase}
            {attachmentPreviews}
            {dragActive}
            {attachmentMessage}
            {saveMessage}
            {formatTime}
            onChange={onDraftChange}
          />
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
          {rambleMessage}
          {attachmentBusy}
          {canSubmit}
          {submitting}
          {canCancel}
          {cancelling}
          {onToggleRamble}
          {onExitRamble}
          {onStartScreenCapture}
          {onImportClipboard}
          {onFileSelection}
          {onInsertAttachment}
          {onRemoveAttachment}
          {onOpenPackage}
          {onSubmit}
          {onCancel}
        />
      </div>
    </div>
  {:else}
    <div class="workspace-placeholder">
      <span class="placeholder-mark">↙</span>
      <strong>{tr('选择一个请求开始体验')}</strong>
      <p>{tr('任务清单和你的 Markdown 草稿都会持久保存在本机。')}</p>
    </div>
  {/if}

  {#if pageError}
    <div class="error-banner" role="alert">
      <strong>{tr('工作台暂时无法完成操作')}</strong>
      <span>{pageError}</span>
    </div>
  {/if}
</section>
