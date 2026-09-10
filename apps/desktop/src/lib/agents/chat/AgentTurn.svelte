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
  export let streamingId: string | null = null
  let processLimit = 60
  let processTurnId = turn.id
  $: if (processTurnId !== turn.id) {
    processTurnId = turn.id
    processLimit = 60
  }
  $: expanded = !turn.foldable || open
  // Long running turns can have thousands of tool/thinking rows. Mount only the
  // latest page until the reader explicitly asks to reveal earlier work.
  $: hiddenProcessCount = Math.max(0, turn.process.length - processLimit)
  $: visibleProcess = turn.process.slice(hiddenProcessCount)
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
      {#if hiddenProcessCount}
        <button type="button" class="text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" onclick={() => processLimit += 60}>
          {chatText($locale, 'View earlier work')} · {hiddenProcessCount}
        </button>
      {/if}
      {#each visibleProcess as activity (activity.id)}<SessionActivityRow {activity} runActive={turn.active} streaming={streamingId === activity.id} />{/each}
    </div>
  {/if}
  {#each turn.answer as activity (activity.id)}<SessionActivityRow {activity} runActive={turn.active} streaming={streamingId === activity.id} />{/each}
  {#each turn.notices as activity (activity.id)}<SessionActivityRow {activity} />{/each}
  {#if !turn.active}<TurnFooter {copyText} completedAt={turn.completedAt} />{/if}
</section>
