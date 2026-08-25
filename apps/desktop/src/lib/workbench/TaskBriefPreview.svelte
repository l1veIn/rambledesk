<script lang="ts">
  import { FileImage, FileText, LoaderCircle, Mic, Paperclip } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import {
    requestStatusLabel,
    type FeedbackStatus,
    type FeedbackWorkspaceView,
    type RequestAttachmentView,
  } from '$lib/feedback'
  import { playClipRackSound } from '$lib/notifications'
  import { t } from '$lib/i18n'
  import { locale, notificationVolume } from '$lib/preferences'
  import LinkifiedText from '$lib/LinkifiedText.svelte'
  import { isSafeHttpUrl } from '$lib/linkify'
  import { openExternalUrl } from '$lib/openExternalUrl'
  import {
    briefBlocks,
    isCaptureTooltipEvent,
    type ClipFlyFrom,
    type RambleClip,
  } from './briefNotes'
  import BriefNoteBlock from './BriefNoteBlock.svelte'
  import RambleClipIcon from './RambleClipIcon.svelte'
  import RequestAttachmentPreview from './RequestAttachmentPreview.svelte'
  import type { BriefNotePhase, HostProfile, RamblePhase } from './types'

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
  export let rambleClips: RambleClip[] = []
  export let briefNotes: Record<string, string[]> = {}
  export let briefNotePhase: BriefNotePhase = 'idle'
  export let briefNoteBlockId: string | null = null
  export let briefNoteProcessingIds: string[] = []
  /** Everything spoken into the note being recorded, stable segments included. */
  export let noteTranscript = ''
  export let onToggleBriefNote: (blockId: string) => void = () => {}
  export let onSaveRambleClip: (clipId: string, text: string) => void = () => {}
  export let onSaveBriefNote: (blockId: string, index: number, text: string) => void = () => {}
  /** CSS transform-origin the dialog should shrink toward when closing. */
  export let origin: string | null = null

  let attachmentPreviewOpen = false
  let attachmentPreview: RequestAttachmentView | null = null
  let recordButtonEl: HTMLElement | null = null
  let flyingClipId: string | null = null
  let flyFrom: ClipFlyFrom | null = null
  let seenClipCount = 0
  let hydratedRequestId: string | null = null

  export let interactionLocked = false

  $: rambleActive = ramblePhase === 'active'
  // Cooking, submitting, cancelling and approving all refuse draft writes, so a
  // tooltip edit made during one is silently swallowed when the operation's own
  // result lands. Present the captures as read-only instead of losing the edit.
  $: readOnly =
    workspace === null ||
    interactionLocked ||
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

  $: rambleLabel =
    ramblePhase === 'starting'
      ? tr('Starting…')
      : ramblePhase === 'stopping'
        ? tr('Writing speech into the document…')
        : rambleActive
          ? tr('Recording')
          : rambleStartedOnce
            ? tr('Resume Ramble')
            : tr('Start Ramble')
  $: noteRecording = briefNotePhase === 'recording'
  $: noteStarting = briefNotePhase === 'starting'
  $: blocks = workspace
    ? briefBlocks({
        whatHappened: workspace.request.what_happened,
        actions: workspace.actions,
        contextRefs: workspace.context_refs,
      })
    : []
  $: whatHappenedBlocks = blocks.filter((block) => block.kind === 'what_happened')
  $: rambleButtonDisabled = rambleBusy || noteRecording || noteStarting
  $: {
    const requestId = workspace?.request.request_id ?? null
    if (requestId !== hydratedRequestId) {
      hydratedRequestId = requestId
      seenClipCount = rambleClips.length
      flyingClipId = null
      flyFrom = null
    } else if (rambleClips.length > seenClipCount) {
      const newest = rambleClips[rambleClips.length - 1]
      const rect = recordButtonEl?.getBoundingClientRect()
      flyFrom = rect
        ? { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
        : null
      flyingClipId = newest.id
      seenClipCount = rambleClips.length
      void playClipRackSound($notificationVolume)
    } else if (rambleClips.length < seenClipCount) {
      seenClipCount = rambleClips.length
      flyingClipId = null
      flyFrom = null
    }
  }

  function openAttachment(attachment: RequestAttachmentView) {
    attachmentPreview = attachment
    attachmentPreviewOpen = true
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    onInteractOutside={(event) => {
      if (isCaptureTooltipEvent(event.target)) event.preventDefault()
    }}
    onFocusOutside={(event) => {
      if (isCaptureTooltipEvent(event.target)) event.preventDefault()
    }}
    class="task-brief-preview-content grid h-[calc(100vh-2rem)] w-[min(1200px,calc(100vw-2rem))] max-w-[min(1200px,calc(100vw-2rem))] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0 duration-200 sm:max-w-[min(1200px,calc(100vw-2rem))]"
    style={origin ? `transform-origin: ${origin}` : undefined}
  >
    <Dialog.Header class="border-b px-6 py-4 pr-14">
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
            <div class="mt-4 grid gap-3">
              {#each whatHappenedBlocks as block (block.id)}
                <BriefNoteBlock
                  notes={briefNotes[block.id] ?? []}
                  recording={noteRecording && briefNoteBlockId === block.id}
                  processing={briefNoteProcessingIds.includes(block.id) ||
                    (noteStarting && briefNoteBlockId === block.id)}
                  disabled={readOnly || rambleActive || rambleBusy}
                  {readOnly}
                  partial={briefNoteBlockId === block.id ? noteTranscript : ''}
                  onToggleRecord={() => onToggleBriefNote(block.id)}
                  onSaveNote={(index, text) => onSaveBriefNote(block.id, index, text)}
                >
                  <p class="m-0 whitespace-pre-wrap text-[15px] leading-7">
                    <LinkifiedText text={block.quote} />
                  </p>
                </BriefNoteBlock>
              {/each}
            </div>
          </section>

          <section class="mt-8">
            <h2 class="m-0 border-b border-border pb-2 text-base font-semibold">
              {tr('Actions to experience')}
            </h2>
            <ol class="m-0 mt-4 grid list-none gap-3 p-0">
              {#each workspace.actions as action, index (action.id)}
                <li class="grid grid-cols-[28px_minmax(0,1fr)] gap-3">
                  <span
                    class="grid size-7 place-items-center rounded-md bg-background text-xs font-semibold text-muted-foreground ring-1 ring-border"
                  >
                    {index + 1}
                  </span>
                  <BriefNoteBlock
                    notes={briefNotes[`action:${action.id}`] ?? []}
                    recording={noteRecording && briefNoteBlockId === `action:${action.id}`}
                    processing={briefNoteProcessingIds.includes(`action:${action.id}`) ||
                      (noteStarting && briefNoteBlockId === `action:${action.id}`)}
                    disabled={readOnly || rambleActive || rambleBusy}
                    {readOnly}
                    partial={briefNoteBlockId === `action:${action.id}` ? noteTranscript : ''}
                    onToggleRecord={() => onToggleBriefNote(`action:${action.id}`)}
                    onSaveNote={(index, text) => onSaveBriefNote(`action:${action.id}`, index, text)}
                  >
                    <span class="min-w-0 self-center text-[15px] leading-7">
                      <LinkifiedText text={action.instruction} />
                    </span>
                  </BriefNoteBlock>
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
                    <BriefNoteBlock
                      notes={briefNotes[`context:${index}`] ?? []}
                      recording={noteRecording && briefNoteBlockId === `context:${index}`}
                      processing={briefNoteProcessingIds.includes(`context:${index}`) ||
                        (noteStarting && briefNoteBlockId === `context:${index}`)}
                      disabled={readOnly || rambleActive || rambleBusy}
                      {readOnly}
                      partial={briefNoteBlockId === `context:${index}` ? noteTranscript : ''}
                      onToggleRecord={() => onToggleBriefNote(`context:${index}`)}
                      onSaveNote={(index, text) => onSaveBriefNote(`context:${index}`, index, text)}
                    >
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
                    </BriefNoteBlock>
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
      <div class="flex shrink-0 items-center gap-3 border-t bg-background px-6 py-3">
        <div class="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto">
          {#each rambleClips as clip, index (clip.id)}
            <RambleClipIcon
              index={index + 1}
              text={clip.text}
              flyFrom={clip.id === flyingClipId ? flyFrom : null}
              processing={Boolean(clip.processing)}
              {readOnly}
              onSave={(text) => onSaveRambleClip(clip.id, text)}
            />
          {/each}
          {#if ramblePhase === 'stopping' && !rambleClips.some((clip) => clip.processing)}
            <span class="grid size-8 place-items-center text-muted-foreground" aria-label={rambleLabel}>
              <LoaderCircle class="size-4 animate-spin" />
            </span>
          {:else if rambleActive}
            <span
              class="record-blink size-2.5 shrink-0 rounded-full bg-destructive"
              aria-hidden="true"
            ></span>
          {/if}
        </div>
        <Button
          bind:ref={recordButtonEl}
          variant={rambleActive ? 'destructive' : 'default'}
          disabled={rambleButtonDisabled}
          onclick={onToggleRamble}
          aria-pressed={rambleActive}
        >
          {#if ramblePhase === 'starting' || ramblePhase === 'stopping'}
            <LoaderCircle class="animate-spin" data-icon="inline-start" />
          {:else if rambleActive}
            <span
              class="record-blink size-2.5 rounded-full bg-destructive"
              data-icon="inline-start"
            ></span>
          {:else}
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
