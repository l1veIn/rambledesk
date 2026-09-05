<script lang="ts">
  import { locale } from '$lib/preferences'
  import type { SessionActivity } from '../managedSessionUi'
  import { activityLabel } from '../managedSessionUi'
  import { activityMessage, activityTool } from './activity-presentation'
  import { chatText } from './chat-text'
  import SessionContent from './SessionContent.svelte'
  import ToolCallCard from './ToolCallCard.svelte'
  import ThinkingBlock from './ThinkingBlock.svelte'
  export let activity: SessionActivity
  export let runActive = false
  export let streaming = false
  $: message = activityMessage(activity)
  $: tool = activityTool(activity)
</script>

<article data-activity-id={activity.id} data-activity-kind={activity.kind} class={`group/activity min-w-0 ${activity.kind === 'user_message' ? 'ml-auto max-w-[90%] rounded-xl bg-muted/50 px-4 py-3' : ''}`}>
  {#if tool}<ToolCallCard {tool} {runActive} />
  {:else if activity.kind === 'agent_thought'}<ThinkingBlock blocks={message.blocks} truncated={message.truncated} {streaming} />
  {:else if activity.kind === 'status' || activity.kind === 'error'}
    <div class={`rounded-md px-3 py-2 text-xs ${activity.kind === 'error' ? 'border border-destructive/25 bg-destructive/5 text-destructive' : 'bg-muted/20 text-muted-foreground'}`}>
      <p class="mb-1 mt-0 text-[10px] font-medium">{chatText($locale, activityLabel(activity.kind))}</p>
      <p class="m-0 whitespace-pre-wrap break-words leading-6">{activity.text}</p>
    </div>
  {:else if activity.kind === 'tool_call'}
    <details class="rounded-lg border border-dashed px-3 py-2 text-xs"><summary class="cursor-pointer text-muted-foreground">{chatText($locale, 'Tool activity')}</summary><pre class="mb-0 whitespace-pre-wrap break-words font-mono text-[11px] leading-5">{activity.text}</pre></details>
  {:else}
    {#if activity.kind === 'user_message'}<p class="mb-2 mt-0 text-[10px] font-medium text-muted-foreground">{chatText($locale, 'You')}</p>{/if}
    <SessionContent blocks={message.blocks} truncated={message.truncated} userMessage={activity.kind === 'user_message'} />
  {/if}
</article>
