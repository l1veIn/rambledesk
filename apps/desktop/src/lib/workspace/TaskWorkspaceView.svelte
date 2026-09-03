<script lang="ts">
  import type { JSONContent } from '@tiptap/core'

  import type { AttachmentView, FeedbackWorkspaceView } from '$lib/feedback'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RequestAttachmentPreview from '$lib/workbench/RequestAttachmentPreview.svelte'
  import TaskBriefView from '$lib/workbench/TaskBriefView.svelte'
  import type { HostProfile } from '$lib/workbench/types'
  import type { RamblePhase } from '$lib/workbench/types'

  export let workspace: FeedbackWorkspaceView | null = null
  export let transport: ApplicationTransport
  export let capabilities: Pick<
    WorkbenchCapabilities,
    'externalLinks' | 'serverPaths' | 'speech' | 'rambleConsole'
  >
  export let loading = false
  export let editorDocument: JSONContent | null = null
  export let activeActionId: string | null = null
  export let actionsDisabled = false
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
  export let previews: Record<string, string> = {}
  export let formatTime: (value: string | null | undefined) => string
  export let resolveHostProfile: (hostId: string) => HostProfile
  export let onToggleRamble: () => void = () => {}
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleStartedOnce = false
  export let rambleBusy = false

  let attachmentPreviewOpen = false
  let previewAttachment: AttachmentView | null = null

  function openAttachmentPreview(attachmentId: string) {
    previewAttachment = workspace?.attachments.find(
      (attachment) => attachment.attachment_id === attachmentId,
    ) ?? null
    attachmentPreviewOpen = previewAttachment !== null
  }
</script>

{#if loading}
  <div
    class="grid h-full min-h-0 place-items-center text-sm text-muted-foreground"
    aria-busy="true"
    aria-live="polite"
  >
    {t($locale, 'Loading workspace…')}
  </div>
{:else}
  <TaskBriefView
    {transport}
    {capabilities}
    {workspace}
    {editorDocument}
    {activeActionId}
    {actionsDisabled}
    {onSelectAction}
    {previews}
    {formatTime}
    {resolveHostProfile}
    onOpenAttachment={openAttachmentPreview}
    {onToggleRamble}
    {ramblePhase}
    {rambleStartedOnce}
    {rambleBusy}
  />
{/if}

{#if workspace}
  <RequestAttachmentPreview
    {transport}
    {capabilities}
    bind:open={attachmentPreviewOpen}
    requestId={workspace.request.request_id}
    attachment={previewAttachment}
    readKind="workspace"
  />
{/if}
