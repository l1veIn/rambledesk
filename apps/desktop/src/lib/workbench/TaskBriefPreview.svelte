<script lang="ts">
  import { ChevronDown, Copy, FileImage, FileText, LoaderCircle, Mic, Paperclip } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import * as Dialog from '$lib/components/ui/dialog'
  import { toast } from '$lib/components/ui/sonner'
  import {
    requestStatusLabel,
    type FeedbackStatus,
    type FeedbackWorkspaceView,
    type RequestAttachmentView,
  } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import LinkifiedText from '$lib/LinkifiedText.svelte'
  import { isSafeHttpUrl } from '$lib/linkify'
  import { openExternalUrl } from '$lib/openExternalUrl'
  import MarkdownPreview from './MarkdownPreview.svelte'
  import RequestAttachmentPreview from './RequestAttachmentPreview.svelte'
  import RecordLed from './RecordLed.svelte'
  import { buildTaskBriefText } from './taskBriefCopy'
  import { rambleRecordPresentation } from './rambleRecordButton'
  import type { HostProfile, RamblePhase } from './types'

  export let open = false
  export let workspace: FeedbackWorkspaceView | null = null
  export let formatTime: (value: string | null | undefined) => string = () => ''
  export let resolveHostProfile: (hostId: string) => HostProfile = (hostId) => ({
    id: hostId,
    label: hostId,
    icon_svg: '',
    default_adapter: 'generic_mcp',
    continuation_mode: 'manual',
  })
  export let onToggleRamble: () => void = () => {}
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleStartedOnce = false
  export let rambleBusy = false
  /** CSS transform-origin the dialog should shrink toward when closing. */
  export let origin: string | null = null
  export let insertDisabled = false
  export let currentActionIndex: number | null = null
  export let onToggleActionChannel: (index: number) => void = () => {}
  /** Markdown notes the human recorded under each Action (from the draft). */
  export let actionNotes: Record<number, string> = {}
  export let attachmentPreviews: Record<string, string> = {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}

  let attachmentPreviewOpen = false
  let attachmentPreview: RequestAttachmentView | null = null

  $: readOnly =
    workspace === null ||
    workspace.request.status === 'completed' ||
    workspace.request.status === 'cancelled'

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function statusBadgeVariant(
    status: FeedbackStatus,
  ): 'default' | 'secondary' | 'destructive' | 'outline' | 'ghost' | 'link' {
    switch (status) {
      case 'in_progress':
        return 'default'
      case 'completed':
        return 'outline'
      case 'cancelled':
        return 'destructive'
      default:
        return 'secondary'
    }
  }

  $: record = rambleRecordPresentation(ramblePhase, rambleStartedOnce)
  $: rambleLabel =
    record.label === 'starting'
      ? tr('Starting…')
      : record.label === 'stopping'
        ? tr('Pausing…')
        : record.label === 'recording'
          ? tr('Recording')
          : record.label === 'resume'
            ? tr('Resume Ramble')
            : tr('Start Ramble')

  function openAttachment(attachment: RequestAttachmentView) {
    attachmentPreview = attachment
    attachmentPreviewOpen = true
  }

  async function copyTaskBrief() {
    if (!workspace) return
    try {
      await navigator.clipboard.writeText(buildTaskBriefText(workspace))
      toast.success(tr('Task brief copied to clipboard.'))
    } catch (cause) {
      toast.error(tr('Could not copy the task brief. Select the text and copy it manually.'), {
        description:
          cause instanceof Error ? cause.message : String(cause),
      })
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    class="task-brief-preview-content grid h-[calc(100vh-2rem)] w-[min(1200px,calc(100vw-2rem))] max-w-[min(1200px,calc(100vw-2rem))] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0 duration-200 sm:max-w-[min(1200px,calc(100vw-2rem))]"
    style={origin ? `transform-origin: ${origin}` : undefined}
  >
    <Dialog.Header class="relative border-b px-6 py-4 pr-14">
      {#if workspace}
        <Button
          variant="ghost"
          size="icon-sm"
          class="absolute right-12 top-2.5"
          aria-label={tr('Copy task brief')}
          title={tr('Copy task brief')}
          onclick={() => void copyTaskBrief()}
        >
          <Copy />
        </Button>
      {/if}
      <Dialog.Title class="text-lg font-semibold leading-snug">
        {workspace?.request.title ?? tr('Task brief')}
      </Dialog.Title>
      <Dialog.Description class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1.5">
        {#if workspace}
          <Badge variant={statusBadgeVariant(workspace.request.status)}>
            {requestStatusLabel(workspace.request.status, $locale)}
          </Badge>
          <span>{resolveHostProfile(workspace.request.host_id).label}</span>
          {#if workspace.request.source_hint}
            <span class="text-muted-foreground">·</span>
            <span class="max-w-[42ch] truncate">{workspace.request.source_hint}</span>
          {/if}
          <span class="text-muted-foreground">·</span>
          <span>{tr('{count} steps', { count: workspace.actions.length })}</span>
          <span class="text-muted-foreground">·</span>
          <span>{formatTime(workspace.request.created_at)}</span>
        {/if}
      </Dialog.Description>
    </Dialog.Header>

    <div class="min-h-0 overflow-y-auto overscroll-contain bg-muted/20">
      {#if workspace}
        <article class="mx-auto max-w-3xl px-8 py-8">
          <section>
            <h2 class="m-0 border-b border-border pb-2 text-base font-semibold">
              {tr('What happened')}
            </h2>
            <p class="m-0 mt-4 whitespace-pre-wrap text-[15px] leading-7">
              <LinkifiedText text={workspace.request.what_happened} />
            </p>
          </section>

          <section class="mt-8">
            <h2 class="m-0 border-b border-border pb-2 text-base font-semibold">
              {tr('Actions to experience')}
            </h2>
            <ol class="m-0 mt-4 grid list-none gap-3 p-0">
              {#each workspace.actions as action, index (action.id)}
                <li class="grid grid-cols-[28px_minmax(0,1fr)] gap-3">
                  <button
                    type="button"
                    class={[
                      'grid size-7 place-items-center rounded-md text-xs font-semibold ring-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
                      currentActionIndex === index + 1
                        ? 'action-channel-live bg-primary/15 text-primary ring-primary'
                        : 'bg-background text-muted-foreground ring-border hover:bg-accent hover:text-accent-foreground',
                    ]}
                    disabled={insertDisabled || readOnly}
                    aria-pressed={currentActionIndex === index + 1}
                    aria-label={
                      currentActionIndex === index + 1
                        ? tr('Return to default channel from Action {index}', { index: index + 1 })
                        : tr('Tune to Action {index}', { index: index + 1 })
                    }
                    title={
                      currentActionIndex === index + 1
                        ? tr('Return to default channel from Action {index}', { index: index + 1 })
                        : tr('Tune to Action {index}', { index: index + 1 })
                    }
                    onclick={() => onToggleActionChannel(index + 1)}
                  >
                    {index + 1}
                  </button>
                  <div class="min-w-0">
                    <span class="block self-center text-[15px] leading-7">
                      <LinkifiedText text={action.instruction} />
                    </span>
                    {#if actionNotes[index + 1]}
                      <Collapsible.Root open class="mt-2 rounded-lg border bg-background/70">
                        <Collapsible.Trigger>
                          {#snippet child({ props })}
                            <button
                              type="button"
                              {...props}
                              class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs font-medium text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                            >
                              <ChevronDown class="size-3.5 transition-transform data-[open]:rotate-180" />
                              {tr('My notes for Action {index}', { index: index + 1 })}
                            </button>
                          {/snippet}
                        </Collapsible.Trigger>
                        <Collapsible.Content class="max-h-72 overflow-y-auto overscroll-contain border-t px-3 pb-3 pt-2">
                          <MarkdownPreview
                            markdown={actionNotes[index + 1] ?? ''}
                            previews={attachmentPreviews}
                            {onOpenAttachment}
                          />
                        </Collapsible.Content>
                      </Collapsible.Root>
                    {/if}
                  </div>
                </li>
              {/each}
            </ol>
          </section>

          {#if workspace.context_refs.length > 0}
            <section class="mt-8">
              <h2 class="m-0 border-b border-border pb-2 text-base font-semibold">
                {tr('Context references')}
              </h2>
              <ul class="m-0 mt-4 grid list-none gap-3 p-0">
                {#each workspace.context_refs as ref, index (`${ref.label}:${ref.uri}:${index}`)}
                  <li class="flex items-start gap-3">
                    <span
                      class="grid size-7 shrink-0 place-items-center rounded-md bg-background text-xs font-semibold text-muted-foreground ring-1 ring-border"
                    >
                      {index + 1}
                    </span>
                    <div class="min-w-0">
                      <strong class="block text-[15px] font-medium leading-6">{ref.label}</strong>
                      {#if isSafeHttpUrl(ref.uri)}
                        <a
                          href={ref.uri}
                          class="block break-all text-sm leading-6 text-primary underline underline-offset-2"
                          rel="noreferrer"
                          onclick={(event) => {
                            event.preventDefault()
                            void openExternalUrl(ref.uri).catch((cause) => {
                              console.warn('Could not open external URL', cause)
                            })
                          }}
                        >
                          {ref.uri}
                        </a>
                      {:else}
                        <span class="block break-all text-sm leading-6 text-muted-foreground">
                          {ref.uri}
                        </span>
                      {/if}
                    </div>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}

          {#if workspace.request_attachments.length > 0}
            <section class="mt-8">
              <h2
                class="m-0 flex items-center gap-1.5 border-b border-border pb-2 text-base font-semibold"
              >
                <Paperclip class="size-4 text-muted-foreground" />
                {tr('Review attachments from the agent')}
              </h2>
              <ul class="m-0 mt-4 grid list-none gap-2 p-0">
                {#each workspace.request_attachments as attachment (attachment.attachment_id)}
                  <li>
                    <button
                      type="button"
                      class="flex w-full items-center gap-3 rounded-lg border bg-background px-3 py-2 text-left transition-colors hover:border-primary/40 hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      aria-label={tr('Preview {name}', { name: attachment.file_name })}
                      onclick={() => openAttachment(attachment)}
                    >
                      {#if attachment.media_type.startsWith('image/')}
                        <FileImage class="size-4 shrink-0 text-muted-foreground" />
                      {:else}
                        <FileText class="size-4 shrink-0 text-muted-foreground" />
                      {/if}
                      <span class="min-w-0 flex-1">
                        <strong class="block truncate text-sm font-medium">
                          {attachment.file_name}
                        </strong>
                        <span class="block text-xs text-muted-foreground">
                          {attachment.media_type === 'text/markdown' ? 'Markdown' : tr('Image')}
                          · {(attachment.byte_size / 1024).toFixed(1)} KiB
                        </span>
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        </article>
      {:else}
        <div class="grid h-full place-items-center text-sm text-muted-foreground">
          {tr('There is no task brief to preview.')}
        </div>
      {/if}
    </div>

    {#if workspace && !readOnly}
      <div class="flex shrink-0 items-center justify-end gap-2 border-t bg-background px-6 py-3">
        <Button
          variant={record.variant}
          disabled={rambleBusy}
          onclick={onToggleRamble}
          aria-pressed={record.pressed}
        >
          {#if record.icon === 'spinner'}
            <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {:else}
            {#if record.icon === 'recording'}
              <RecordLed />
            {/if}
            <Mic data-icon="inline-start" />
          {/if}
          {rambleLabel}
        </Button>
      </div>
    {/if}
  </Dialog.Content>
</Dialog.Root>

{#if workspace}
  <RequestAttachmentPreview
    bind:open={attachmentPreviewOpen}
    requestId={workspace.request.request_id}
    attachment={attachmentPreview}
  />
{/if}

<style>
  /* Collapse toward the preview button instead of the default subtle zoom-out. */
  :global(.task-brief-preview-content[data-state='closed']) {
    --tw-exit-scale: 0.08 !important;
  }
</style>
