<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { JSONContent } from '@tiptap/core'
  import { ChefHat, Copy, FileImage, FileText, LoaderCircle, Mic, Paperclip, Send } from '@lucide/svelte'

  import { collectActionGroupContent } from '$lib/actionGroupContent'
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { toast } from '$lib/components/ui/sonner'
  import {
    requestStatusLabel,
    type FeedbackStatus,
    type FeedbackWorkspaceView,
    type RequestAttachmentView,
  } from '$lib/feedback'
  import { t } from '$lib/i18n'
  import type { ApplicationTransport } from '$lib/application/applicationTransport'
  import type { WorkbenchCapabilities } from '$lib/capabilities/workbenchCapabilities'
  import { locale } from '$lib/preferences'
  import LinkifiedText from '$lib/LinkifiedText.svelte'
  import { isSafeHttpUrl } from '$lib/linkify'
  import RequestAttachmentPreview from './RequestAttachmentPreview.svelte'
  import ActionFeedbackCard from './ActionFeedbackCard.svelte'
  import { buildTaskBriefText } from './taskBriefCopy'
  import RecordLed from './RecordLed.svelte'
  import { rambleRecordPresentation } from './rambleRecordButton'
  import { voiceRambleAvailable as canStartVoiceRamble } from './workbenchCapabilityUi'
  import type { HostProfile, RamblePhase } from './types'

  export let workspace: FeedbackWorkspaceView | null = null
  export let transport: ApplicationTransport
  export let capabilities: Pick<
    WorkbenchCapabilities,
    'externalLinks' | 'serverPaths' | 'speech' | 'rambleConsole'
  >
  export let editorDocument: JSONContent | null = null
  export let activeActionId: string | null = null
  export let actionsDisabled = false
  export let agentStatus: Snippet | undefined = undefined
  export let onSelectAction: (actionId: string, actionIndex: number, title: string) => void = () => {}
  export let previews: Record<string, string> = {}
  export let onOpenAttachment: (attachmentId: string) => void = () => {}
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
  export let canSubmit = false
  export let cookingEnabled = false
  export let cookedDraftReady = false
  export let cooking = false
  export let submitting = false
  export let onSubmitFeedback: () => void = () => {}

  let attachmentPreviewOpen = false
  let attachmentPreview: RequestAttachmentView | null = null

  $: readOnly =
    workspace === null ||
    workspace.request.status === 'completed' ||
    workspace.request.status === 'cancelled'
  $: actionGroupContent = collectActionGroupContent(editorDocument)
  $: voiceRambleAvailable = canStartVoiceRamble(capabilities.speech.status)

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
  $: deliveryLabel = cooking
    ? tr('Cooking…')
    : submitting
      ? tr('Publishing…')
      : cookingEnabled && !cookedDraftReady
        ? tr('Cook and submit')
        : tr('Submit feedback')
  $: deliveryBusy = cooking || submitting

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

<div class="task-brief grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden bg-background">
    <header class="task-header border-b" class:has-agent-status={!!agentStatus}>
      <div class="relative min-w-0 px-6 py-4 pr-14">
      {#if workspace}
        <Button
          variant="ghost"
          size="icon-sm"
          class="absolute right-3 top-3"
          aria-label={tr('Copy task brief')}
          title={tr('Copy task brief')}
          onclick={() => void copyTaskBrief()}
        >
          <Copy />
        </Button>
      {/if}
      <div class="flex items-center gap-2">
        {#if workspace}
          <Badge variant={statusBadgeVariant(workspace.request.status)} class="shrink-0">
            {requestStatusLabel(workspace.request.status, $locale)}
          </Badge>
        {/if}
        <h1 class="m-0 min-w-0 text-lg font-semibold leading-snug">
          {workspace?.request.title ?? tr('Task brief')}
        </h1>
      </div>
      <div class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1.5 text-sm text-muted-foreground">
        {#if workspace}
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
      </div>
      </div>
      {#if agentStatus}
        <div class="agent-status-column min-w-0 border-l bg-muted/15 px-4 py-2">
          {@render agentStatus()}
        </div>
      {/if}
    </header>

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
                {@const feedback =
                  actionGroupContent.get(action.id) ??
                  actionGroupContent.get(`legacy-action-${index + 1}`)}
                <li>
                  <button
                    type="button"
                    disabled={readOnly || actionsDisabled}
                    aria-pressed={activeActionId === action.id}
                    class={[
                      'grid w-full grid-cols-[28px_minmax(0,1fr)] gap-3 rounded-md px-2 py-2 text-left transition-colors hover:bg-accent/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-transparent',
                      activeActionId === action.id ? 'bg-primary/10 ring-1 ring-primary/30' : '',
                    ]}
                    onclick={(event) => {
                      if (!event.defaultPrevented) onSelectAction(action.id, index, action.instruction)
                    }}
                  >
                    <span
                      class="grid size-7 place-items-center rounded-md bg-background text-xs font-semibold text-muted-foreground ring-1 ring-border"
                    >
                      {index + 1}
                    </span>
                    <span class="min-w-0 self-center text-[15px] leading-7">
                      <LinkifiedText text={action.instruction} />
                    </span>
                  </button>
                  {#if feedback}
                    <div class="ml-12 min-w-0 pr-2">
                      <ActionFeedbackCard
                        document={feedback.document}
                        groupCount={feedback.groupCount}
                        {previews}
                        {onOpenAttachment}
                      />
                    </div>
                  {/if}
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
                            void capabilities.externalLinks.implementation.open(ref.uri).catch((cause) => {
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
      <div class="flex shrink-0 items-center justify-between gap-2 border-t bg-background px-6 py-2">
        <Button
          variant={cookingEnabled && !cookedDraftReady ? 'secondary' : 'default'}
          disabled={!canSubmit}
          onclick={onSubmitFeedback}
          title={tr('Submit the feedback for this task')}
        >
          {#if deliveryBusy}
            <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {:else}
            {#if cookingEnabled && !cookedDraftReady}
              <ChefHat data-icon="inline-start" />
            {:else}
              <Send data-icon="inline-start" />
            {/if}
          {/if}
          {deliveryLabel}
        </Button>
        {#if voiceRambleAvailable}
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
        {:else}
          <div
            class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground"
            role="status"
          >
            <Mic class="size-4 shrink-0" aria-hidden="true" />
            <span>{tr('Voice Ramble is available only in the desktop app.')}</span>
            <Badge variant="outline" class="shrink-0">{tr('Desktop only')}</Badge>
          </div>
        {/if}
      </div>
    {/if}
</div>

{#if workspace}
  <RequestAttachmentPreview
    {transport}
    {capabilities}
    bind:open={attachmentPreviewOpen}
    requestId={workspace.request.request_id}
    attachment={attachmentPreview}
  />
{/if}

<style>
  .task-header.has-agent-status {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--workspace-rail-width, 288px);
  }
  @media (max-width: 1180px) {
    .task-header.has-agent-status { grid-template-columns: minmax(0, 1fr); }
    .agent-status-column { border-left: 0; border-top: 1px solid var(--border); }
  }
</style>
