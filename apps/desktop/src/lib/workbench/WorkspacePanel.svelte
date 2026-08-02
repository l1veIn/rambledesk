<script lang="ts">
  import { AlertTriangle, Inbox } from '@lucide/svelte'

  import * as Alert from '$lib/components/ui/alert'
  import { Skeleton } from '$lib/components/ui/skeleton'
  import type {
    AttachmentView,
    FeedbackResultView,
    FeedbackWorkspaceView,
  } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type {
    FeedbackEditorHandle,
    HostProfile,
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
  export let resolveHostProfile: (hostId: string) => HostProfile
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

<section class="workspace-panel relative flex min-h-0 min-w-0 flex-1 flex-col bg-background">
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
    <WorkspaceHeader {workspace} {resolveHostProfile} onReload={onReload} />

    <div class="workspace-columns min-h-0 flex-1 overflow-auto">
      <div class="document-column flex min-h-0 min-w-0 flex-col @container">
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
        <strong class="block text-sm font-medium">{tr('选择一个请求')}</strong>
        <p class="m-0 mt-1 text-xs leading-5 text-muted-foreground">
          {tr('从左侧选择宿主、会话和请求，打开反馈工作区。')}
        </p>
      </div>
    </div>
  {/if}

  {#if pageError}
    <Alert.Root variant="destructive" class="absolute inset-x-4 bottom-4 z-20 shadow-lg">
      <AlertTriangle />
      <Alert.Title>{tr('工作台暂时无法完成操作')}</Alert.Title>
      <Alert.Description>{pageError}</Alert.Description>
    </Alert.Root>
  {/if}
</section>

<style>
  .workspace-columns {
    display: grid;
    grid-template-columns: minmax(360px, 1fr) 288px;
  }

  @media (max-width: 1180px) {
    .workspace-columns {
      grid-template-columns: minmax(0, 1fr);
    }

    .document-column {
      min-height: 680px;
    }

    :global(.command-rail) {
      min-height: 620px;
      border-top: 1px solid var(--border);
      border-left: 0;
    }
  }
</style>
