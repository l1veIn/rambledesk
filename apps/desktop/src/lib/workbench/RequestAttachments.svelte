<script lang="ts">
  import { FileImage, FileText } from '@lucide/svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView, RequestAttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RequestAttachmentPreview from '../workspace/RequestAttachmentPreview.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  let previewOpen = false
  let previewAttachment: RequestAttachmentView | null = null
  const tr = (source: string, values: Record<string, string | number> = {}) => t($locale, source, values)
</script>

{#if workspace.request_attachments.length > 0}
  <div class="mt-2 flex flex-wrap gap-1.5" role="group" data-request-materials aria-label={tr('Request materials')}>
    {#each workspace.request_attachments as attachment (attachment.attachment_id)}
      <button type="button"
        class="flex min-w-0 max-w-[min(100%,16rem)] items-center gap-1.5 rounded-md border border-border/60 bg-background/60 px-2 py-1 text-left text-xs text-muted-foreground hover:border-primary/40 hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={tr('Preview {name}', { name: attachment.file_name })}
        title={attachment.file_name}
        onclick={() => { previewAttachment = attachment; previewOpen = true }}>
        {#if attachment.media_type.startsWith('image/')}<FileImage class="size-3.5 shrink-0" />{:else}<FileText class="size-3.5 shrink-0" />{/if}
        <span class="truncate">{attachment.file_name}</span>
      </button>
    {/each}
  </div>
{/if}

<RequestAttachmentPreview {transport} {capabilities} bind:open={previewOpen} requestId={workspace.request.request_id} attachment={previewAttachment} />
