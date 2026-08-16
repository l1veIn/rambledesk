<script lang="ts">
  import { ChevronDown, Eye, FileImage, FileText, ListChecks, Maximize2, Paperclip } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import type { FeedbackWorkspaceView, RequestAttachmentView } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RequestAttachmentPreview from './RequestAttachmentPreview.svelte'

  export let workspace: FeedbackWorkspaceView
  export let open = true
  export let pulseNonce = 0
  export let onOpenPreview: (transformOrigin: string | null) => void = () => {}

  let previewOpen = false
  let previewAttachment: RequestAttachmentView | null = null
  let previewButton: HTMLElement | null = null

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function openPreview(attachment: RequestAttachmentView) {
    previewAttachment = attachment
    previewOpen = true
  }

  function openFullscreenPreview() {
    const rect = previewButton?.getBoundingClientRect()
    if (!rect) {
      onOpenPreview(null)
      return
    }
    const bx = rect.left + rect.width / 2
    const by = rect.top + rect.height / 2
    // The dialog is centered in the viewport; express the collapse pivot as an
    // offset from the dialog center so it lands back on this button.
    const dx = Math.round(bx - window.innerWidth / 2)
    const dy = Math.round(by - window.innerHeight / 2)
    onOpenPreview(`calc(50% + ${dx}px) calc(50% + ${dy}px)`)
  }
</script>

<Collapsible.Root
  bind:open
  class={`task-brief flex h-full min-h-0 flex-col overflow-hidden ${open ? '' : 'border-b'}`}
>
  <div class="flex min-h-12 shrink-0 items-center gap-3 px-5 py-2">
    <ListChecks class="size-5 shrink-0 text-muted-foreground" />
    <div class="min-w-0 flex-1">
      {#if open}
        <strong class="block text-xs font-medium">
          {tr('What happened')} · {tr('Actions to experience')}
        </strong>
      {:else}
        <strong class="block text-xs font-medium">{tr('Task brief')}</strong>
        <span class="block truncate text-[10px] text-muted-foreground">
          {workspace.request.what_happened}
        </span>
      {/if}
    </div>
    <Badge variant="secondary" class="h-5 px-1.5 text-[9px]">
      {tr('{count} steps', { count: workspace.actions.length })}
    </Badge>
    {#if workspace.request_attachments.length > 0}
      <Badge variant="outline" class="h-5 gap-1 px-1.5 text-[9px]">
        <Paperclip class="size-2.5" />
        {workspace.request_attachments.length}
      </Badge>
    {/if}
    <Button
      bind:ref={previewButton}
      variant="ghost"
      size="icon-sm"
      aria-label={tr('Fullscreen preview')}
      title={tr('Fullscreen preview')}
      onclick={openFullscreenPreview}
    >
      {#key pulseNonce}
        <Maximize2 class={pulseNonce > 0 ? 'brief-pulse-icon' : ''} />
      {/key}
    </Button>
    <Collapsible.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="ghost"
          size="icon-sm"
          aria-label={open ? tr('Collapse') : tr('Expand')}
          title={open ? tr('Collapse') : tr('Expand')}
        >
          <ChevronDown class={['transition-transform', open ? 'rotate-180' : '']} />
        </Button>
      {/snippet}
    </Collapsible.Trigger>
  </div>

  <Collapsible.Content class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
    <div class="grid gap-5 bg-muted/25 px-5 py-4 text-xs @min-[700px]:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
      <section>
        <h2 class="m-0 text-[10px] font-semibold uppercase text-muted-foreground">
          {tr('What happened')}
        </h2>
        <p class="m-0 mt-2 leading-5">{workspace.request.what_happened}</p>
      </section>

      <section>
        <h2 class="m-0 text-[10px] font-semibold uppercase text-muted-foreground">
          {tr('Actions to experience')}
        </h2>
        <ol class="m-0 mt-2 grid list-none gap-2 p-0">
          {#each workspace.actions as action, index (action.id)}
            <li class="grid grid-cols-[22px_minmax(0,1fr)] gap-2 leading-5">
              <span class="grid size-5 place-items-center rounded-md bg-background text-[9px] font-medium ring-1 ring-border">
                {index + 1}
              </span>
              <span>{action.instruction}</span>
            </li>
          {/each}
        </ol>
      </section>

      {#if workspace.request_attachments.length > 0}
        <section class="@min-[700px]:col-span-2">
          <h2 class="m-0 flex items-center gap-1.5 text-[10px] font-semibold uppercase text-muted-foreground">
            <Paperclip class="size-3" />
            {tr('Review attachments from the agent')}
          </h2>
          <div class="mt-2 grid gap-2 @min-[700px]:grid-cols-2">
            {#each workspace.request_attachments as attachment (attachment.attachment_id)}
              <button
                type="button"
                class="group flex min-w-0 items-center gap-2 rounded-lg border bg-background px-3 py-2 text-left transition-colors hover:border-primary/40 hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label={tr('Preview {name}', { name: attachment.file_name })}
                onclick={() => openPreview(attachment)}
              >
                {#if attachment.media_type.startsWith('image/')}
                  <FileImage class="size-4 shrink-0 text-muted-foreground group-hover:text-primary" />
                {:else}
                  <FileText class="size-4 shrink-0 text-muted-foreground group-hover:text-primary" />
                {/if}
                <span class="min-w-0 flex-1">
                  <strong class="block truncate text-[10px] font-medium">{attachment.file_name}</strong>
                  <span class="block text-[9px] text-muted-foreground">
                    {attachment.media_type === 'text/markdown' ? 'Markdown' : tr('Image')}
                    · {(attachment.byte_size / 1024).toFixed(1)} KiB
                  </span>
                </span>
                <Eye class="size-3.5 shrink-0 text-muted-foreground group-hover:text-primary" />
              </button>
            {/each}
          </div>
        </section>
      {/if}
    </div>
  </Collapsible.Content>
</Collapsible.Root>

<RequestAttachmentPreview
  bind:open={previewOpen}
  requestId={workspace.request.request_id}
  attachment={previewAttachment}
/>

<style>
  :global(.brief-pulse-icon) {
    animation: brief-pulse 640ms ease-out;
  }

  @keyframes brief-pulse {
    0% {
      transform: scale(1);
    }
    30% {
      transform: scale(1.45);
    }
    70% {
      transform: scale(1.1);
    }
    100% {
      transform: scale(1);
    }
  }
</style>
