<script lang="ts">
  import { Eye, FileImage, FileText, Paperclip } from '@lucide/svelte'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import type { FeedbackWorkspaceView, RequestAttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import MarkdownPreview from '$lib/editor/MarkdownPreview.svelte'
  import RequestAttachmentPreview from '../workspace/RequestAttachmentPreview.svelte'

  export let workspace: FeedbackWorkspaceView
  export let transport: ApplicationTransport
  export let capabilities: Pick<WorkbenchCapabilities, 'serverPaths'>
  export let showContext = true
  let previewOpen = false
  let previewAttachment: RequestAttachmentView | null = null
  const tr = (source: string, values: Record<string, string | number> = {}) => t($locale, source, values)
</script>

{#if showContext || workspace.request_attachments.length > 0}
  <aside class="max-h-48 shrink-0 overflow-y-auto border-t bg-muted/20 px-5 py-3 text-xs" data-request-materials aria-label={tr('Request materials')}>
    {#if showContext}
      <h2 class="m-0 text-[10px] font-semibold uppercase text-muted-foreground">{tr('What happened')}</h2>
      <div class="mt-1 leading-5"><MarkdownPreview markdown={workspace.request.what_happened} bare /></div>
    {/if}
    {#if workspace.request_attachments.length > 0}
      <h2 class="mb-2 flex items-center gap-1.5 text-[10px] font-semibold uppercase text-muted-foreground">
        <Paperclip class="size-3" />{tr('Review attachments from the agent')}
      </h2>
      <div class="grid gap-2 @min-[700px]:grid-cols-2">
        {#each workspace.request_attachments as attachment (attachment.attachment_id)}
          <button type="button" class="group flex min-w-0 items-center gap-2 rounded-lg border bg-background px-3 py-2 text-left hover:border-primary/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            aria-label={tr('Preview {name}', { name: attachment.file_name })}
            onclick={() => { previewAttachment = attachment; previewOpen = true }}>
            {#if attachment.media_type.startsWith('image/')}<FileImage class="size-4 shrink-0 text-muted-foreground" />{:else}<FileText class="size-4 shrink-0 text-muted-foreground" />{/if}
            <span class="min-w-0 flex-1"><strong class="block truncate text-[10px] font-medium">{attachment.file_name}</strong><span class="text-[9px] text-muted-foreground">{(attachment.byte_size / 1024).toFixed(1)} KiB</span></span>
            <Eye class="size-3.5 shrink-0 text-muted-foreground" />
          </button>
        {/each}
      </div>
    {/if}
  </aside>
{/if}

<RequestAttachmentPreview {transport} {capabilities} bind:open={previewOpen} requestId={workspace.request.request_id} attachment={previewAttachment} />
