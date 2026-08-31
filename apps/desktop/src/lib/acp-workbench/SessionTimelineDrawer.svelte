<script lang="ts">
  import {
    Activity,
    Brain,
    CheckCircle2,
    CircleAlert,
    Clock3,
    Hammer,
    LoaderCircle,
    MessageSquareText,
    X,
  } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { Badge } from '$lib/components/ui/badge'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  import AgentLogo from './AgentLogo.svelte'
  import { timelineTurnStartsOpen } from './timelinePresentation'
  import type {
    AcpSessionSummary,
    AgentSummary,
    AttentionItem,
    SessionTimeline,
    TimelineEntry,
  } from './types'

  export let open = false
  export let session: AcpSessionSummary | null = null
  export let agent: AgentSummary | null = null
  export let timeline: SessionTimeline | null = null
  export let attentionItems: AttentionItem[] = []
  export let onClose: () => void = () => {}
  export let onOpenRequest: (item: AttentionItem) => void = () => {}

  $: waitingItems = attentionItems.filter((item) => item.status === 'waiting')
  $: turns = timeline?.turns ?? []

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function entryIcon(entry: TimelineEntry) {
    if (entry.kind === 'thought') return Brain
    if (entry.kind === 'tool') return Hammer
    if (entry.kind === 'error' || entry.status === 'failed') return CircleAlert
    if (entry.status === 'running') return LoaderCircle
    if (entry.kind === 'message') return MessageSquareText
    return CheckCircle2
  }

  function statusLabel(status: AcpSessionSummary['status']) {
    if (status === 'running') return tr('Agent is working')
    if (status === 'waiting') return tr('Waiting for you')
    if (status === 'offline') return tr('Offline')
    return tr('Completed')
  }

  function requestKindLabel(item: AttentionItem) {
    if (item.kind === 'permission') return tr('Permission Request')
    if (item.kind === 'question') return tr('Ask Question')
    return tr('Feedback Request')
  }
</script>

{#if open && session}
  <div class="pointer-events-none fixed inset-y-[46px] right-0 z-50 flex justify-end" aria-live="polite">
    <aside
      class="pointer-events-auto flex h-full w-[min(460px,calc(100vw-1rem))] flex-col border-l bg-background shadow-2xl"
      aria-label={tr('Agent timeline')}
    >
      <header class="flex shrink-0 items-start gap-3 border-b px-4 py-3">
        <AgentLogo
          agentId={agent?.id ?? session.agentId}
          label={agent?.label ?? session.agentLabel}
          iconSvg={agent?.iconSvg ?? ''}
          size="md"
        />
        <div class="min-w-0 flex-1">
          <strong class="block truncate text-sm">{session.title}</strong>
          <div class="mt-1 flex items-center gap-2 text-[10px] text-muted-foreground">
            {#if session.status === 'running'}
              <LoaderCircle class="size-3 animate-spin text-primary" />
            {:else if session.status === 'waiting'}
              <Clock3 class="size-3 text-warning" />
            {:else}
              <Activity class="size-3" />
            {/if}
            <span>{statusLabel(session.status)}</span>
            <span aria-hidden="true">·</span>
            <span class="truncate">{session.workspace}</span>
          </div>
        </div>
        <Button variant="ghost" size="icon-sm" aria-label={tr('Close timeline')} onclick={onClose}>
          <X />
        </Button>
      </header>

      <div class="shrink-0 border-b bg-muted/25 px-4 py-2 text-[10px] leading-4 text-muted-foreground">
        {tr('This is a live view of the current Agent run. Full history is not stored by RambleDesk.')}
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        {#if turns.length > 0}
          <div class="grid gap-3">
            {#each turns as turn, index (turn.turnId)}
              <section class="rounded-lg border bg-card">
                <details open={timelineTurnStartsOpen(index, turns.length)}>
                  <summary class="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-[11px] font-medium marker:hidden">
                    {#if turn.status === 'running'}
                      <LoaderCircle class="size-3.5 animate-spin text-primary" />
                    {:else if turn.status === 'failed'}
                      <CircleAlert class="size-3.5 text-destructive" />
                    {:else}
                      <CheckCircle2 class="size-3.5 text-emerald-600" />
                    {/if}
                    <span class="flex-1">{tr('Turn {number}', { number: index + 1 })}</span>
                    <Badge variant="outline" class="h-5 text-[9px]">
                      {turn.status === 'running' ? tr('Working') : turn.status === 'failed' ? tr('Failed') : tr('Work details')}
                    </Badge>
                  </summary>

                  <div class="grid gap-2 border-t px-3 py-3">
                    {#each turn.entries as entry (entry.id)}
                      {@const EntryIcon = entryIcon(entry)}
                      <article class="grid grid-cols-[20px_minmax(0,1fr)] gap-2 text-[11px]">
                        <span class="mt-0.5 grid size-5 place-items-center rounded bg-muted text-muted-foreground">
                          <EntryIcon class={entry.status === 'running' ? 'size-3 animate-spin' : 'size-3'} />
                        </span>
                        <div class="min-w-0">
                          <div class="flex items-center gap-2">
                            <strong class="truncate font-medium">{entry.title}</strong>
                            {#if entry.status === 'waiting'}
                              <span class="size-1.5 shrink-0 rounded-full bg-warning"></span>
                            {/if}
                          </div>
                          {#if entry.content}
                            <p class="m-0 mt-1 whitespace-pre-wrap break-words text-[10px] leading-4 text-muted-foreground">{entry.content}</p>
                          {/if}
                        </div>
                      </article>
                    {:else}
                      <p class="m-0 text-[10px] text-muted-foreground">{tr('Waiting for Agent activity…')}</p>
                    {/each}
                  </div>
                </details>
              </section>
            {/each}
          </div>
        {:else}
          <div class="grid min-h-40 place-items-center rounded-lg border border-dashed text-center text-[11px] text-muted-foreground">
            <div>
              <Activity class="mx-auto mb-2 size-5" />
              <p class="m-0">{tr('No live Timeline is attached to this Session.')}</p>
              <p class="m-0 mt-1 text-[10px]">{tr('Resume or run the Agent to watch its work here.')}</p>
            </div>
          </div>
        {/if}

        {#if waitingItems.length > 0}
          <section class="mt-4">
            <h3 class="mb-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">{tr('Structured requests')}</h3>
            <div class="grid gap-2">
              {#each waitingItems as item (item.id)}
                <button
                  type="button"
                  class="rounded-lg border border-primary/25 bg-primary/5 px-3 py-2 text-left transition-colors hover:bg-primary/10"
                  onclick={() => onOpenRequest(item)}
                >
                  <span class="text-[9px] font-semibold uppercase tracking-wide text-primary">{requestKindLabel(item)}</span>
                  <strong class="mt-0.5 block text-[11px]">{item.title}</strong>
                  <span class="mt-1 block text-[10px] text-muted-foreground">{tr('Open in the request workspace')}</span>
                </button>
              {/each}
            </div>
          </section>
        {/if}
      </div>
    </aside>
  </div>
{/if}
