<!-- Process/answer layout adapted from Codeg CompletedTurnContent at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; durable RambleDesk turns, lazy process content. -->
<script lang="ts">
  import { ChevronRight, LoaderCircle } from '@lucide/svelte'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import { activityTool } from './activity-presentation'
  import { turnCopyText, turnDurationLabel, type AgentTurn } from './turn-presentation'
  import SessionActivityRow from './SessionActivityRow.svelte'
  import TurnFooter from './TurnFooter.svelte'
  export let turn: AgentTurn
  export let open = false
  export let onOpenChange: (open: boolean) => void
  export let quoteDisabled = false
  export let onQuote: (text: string) => void
  export let streamingId: string | null = null
  $: expanded = !turn.foldable || open
  $: label = turn.outcome === 'working' ? 'Working…' : turn.outcome === 'cancelled' ? 'Turn cancelled'
    : turn.outcome === 'interrupted' ? 'Turn interrupted' : turn.outcome === 'finished' ? 'Finished working' : 'Work stopped'
  $: elapsed = turn.durationMs !== null ? turnDurationLabel(turn.durationMs, $locale) : null
  $: heading = elapsed && turn.outcome === 'finished'
    ? `${chatText($locale, 'Worked for')} ${elapsed}` : `${chatText($locale, label)}${elapsed ? ` · ${elapsed}` : ''}`
  $: copyText = turnCopyText(turn)
  $: latestWork = turn.process.at(-1)
  $: currentWork = turn.answer.length ? chatText($locale, 'Writing reply') : latestWork
    ? latestWork.kind === 'agent_thought' ? chatText($locale, 'Thinking')
      : activityTool(latestWork)?.title || latestWork.text : ''
</script>

<section class="min-w-0 space-y-4" data-turn-id={turn.id} data-turn-active={turn.active}>
  <header data-activity-id={`turn:${turn.id}`} class="border-b border-foreground/10 pb-1.5 text-xs text-muted-foreground">
    {#if turn.foldable}
      <button type="button" class="flex w-full items-center gap-1.5 text-left" aria-expanded={expanded} aria-controls={`process-${turn.id}`} onclick={() => onOpenChange(!expanded)}>
        {#if turn.active}<LoaderCircle class="size-3.5 animate-spin" />{/if}
        <span class="tabular-nums">{heading}</span><ChevronRight class={`size-3.5 transition-transform ${expanded ? 'rotate-90' : ''}`} />
      </button>
    {:else}<p class="m-0 flex items-center gap-1.5">{#if turn.active}<LoaderCircle class="size-3.5 animate-spin" />{/if}<span class="tabular-nums">{heading}</span></p>{/if}
  </header>
  {#if turn.active && !expanded && currentWork}<p role="status" class="m-0 truncate text-[11px] text-muted-foreground">{currentWork}</p>{/if}
  {#if turn.partialStart}<p class="m-0 text-[11px] text-muted-foreground">{chatText($locale, 'Earlier activity for this turn is not loaded.')}</p>{/if}
  {#if expanded && turn.process.length}
    <div id={`process-${turn.id}`} class="space-y-4" data-turn-process>
      {#each turn.process as activity (activity.id)}<SessionActivityRow {activity} runActive={turn.active} streaming={streamingId === activity.id} {quoteDisabled} {onQuote} />{/each}
    </div>
  {/if}
  {#each turn.answer as activity (activity.id)}<SessionActivityRow {activity} runActive={turn.active} streaming={streamingId === activity.id} {quoteDisabled} {onQuote} />{/each}
  {#each turn.notices as activity (activity.id)}<SessionActivityRow {activity} {quoteDisabled} {onQuote} />{/each}
  {#if !turn.active}<TurnFooter {copyText} completedAt={turn.completedAt} />{/if}
</section>
