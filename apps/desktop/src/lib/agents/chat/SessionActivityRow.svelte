<script lang="ts">
  import { ChevronRight, Wrench } from '@lucide/svelte'
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
    <details class="group/legacy-tool min-w-0 text-xs">
      <summary class="flex w-fit max-w-full cursor-pointer list-none items-center gap-2 rounded-sm py-1 leading-5 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring">
        <Wrench class="size-3.5 shrink-0" />
        <span class="min-w-0 truncate font-medium" title={activity.text}>{activity.text.trim().split('\n')[0] || chatText($locale, 'Tool activity')}</span>
        <ChevronRight aria-hidden="true" class="size-3 shrink-0 transition-transform group-open/legacy-tool:rotate-90" />
      </summary>
      <pre class="mb-2 ml-5.5 mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/15 p-3 font-mono text-[11px] leading-5">{activity.text}</pre>
    </details>
  {:else}
    <!-- {#if activity.kind === 'user_message'}<p class="mb-2 mt-0 text-[10px] font-medium text-muted-foreground">{chatText($locale, 'You')}</p>{/if} -->
    <SessionContent blocks={message.blocks} truncated={message.truncated} userMessage={activity.kind === 'user_message'} />
  {/if}
</article>
