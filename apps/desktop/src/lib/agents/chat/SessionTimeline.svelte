<script lang="ts">
  import type { SessionActivity } from '../managedSessionUi'
  import { activityInRunningTurn, latestStreamingActivity } from './activity-presentation'
  import { groupTimeline, sessionTurnFolds } from './turn-presentation'
  import SessionActivityRow from './SessionActivityRow.svelte'
  import AgentTurn from './AgentTurn.svelte'
  export let sessionId: string
  export let activities: readonly SessionActivity[]
  export let runActive = false
  export let onResize: () => void = () => {}
  let foldRevision = 0
  let openTurns = new Map<string, boolean>()
  $: streamingId = latestStreamingActivity(activities, runActive)
  $: items = groupTimeline(activities, runActive)
  $: folds = sessionTurnFolds(sessionId)
  $: {
    foldRevision
    folds.observe(activities, items)
    openTurns = new Map(items.flatMap((item) => item.type === 'turn' ? [[item.id, folds.open(item.turn)] as const] : []))
  }
  function toggle(id: string, open: boolean) { folds.toggle(id, open); foldRevision += 1 }
  function observeSize(element: HTMLElement) {
    const observer = new ResizeObserver(() => onResize())
    observer.observe(element)
    return { destroy() { observer.disconnect() } }
  }
</script>

{#key sessionId}
  <div class="mx-auto w-full max-w-4xl space-y-5" data-agent-timeline use:observeSize>
    {#each items as item (item.id)}
      {#if item.type === 'turn'}
        <AgentTurn turn={item.turn} open={openTurns.get(item.id) ?? false} onOpenChange={(open) => toggle(item.turn.id, open)} {streamingId} />
      {:else}<SessionActivityRow activity={item.activity} runActive={activityInRunningTurn(item.activity, activities, runActive)} streaming={streamingId === item.activity.id} />{/if}
    {/each}
  </div>
{/key}
