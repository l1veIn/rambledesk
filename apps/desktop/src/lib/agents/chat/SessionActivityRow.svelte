<script lang="ts">
  import { Quote } from '@lucide/svelte'
  import { locale } from '$lib/preferences'
  import type { SessionActivity } from '../managedSessionUi'
  import { activityLabel } from '../managedSessionUi'
  import { activityHasQuote, activityMessage, activityQuoteText, activityTool } from './activity-presentation'
  import { chatText } from './chat-text'
  import SessionContent from './SessionContent.svelte'
  import ToolCallCard from './ToolCallCard.svelte'
  import ThinkingBlock from './ThinkingBlock.svelte'
  export let activity: SessionActivity
  export let runActive = false
  export let streaming = false
  export let quoteDisabled = false
  export let onQuote: (text: string) => void
  $: message = activityMessage(activity)
  $: tool = activityTool(activity)
  function quote(event: MouseEvent) {
    const root = (event.currentTarget as HTMLElement).closest('[data-activity-id]')
    const selection = window.getSelection()
    const text = selection && root?.contains(selection.anchorNode) && root.contains(selection.focusNode)
      ? selection.toString().trim() : ''
    onQuote(text || activityQuoteText(activity))
  }
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
  {#if activity.kind !== 'status' && activity.kind !== 'error' && activityHasQuote(activity)}
    <div class="mt-1 flex justify-end">
      <button type="button" class="rounded p-1 text-muted-foreground opacity-50 transition-opacity hover:bg-muted hover:text-foreground hover:opacity-100 focus-visible:opacity-100 group-hover/activity:opacity-100 disabled:opacity-25" disabled={quoteDisabled} aria-label={chatText($locale, 'Quote in message')} title={chatText($locale, 'Quote in message')} onmousedown={(event) => event.preventDefault()} onclick={quote}><Quote class="size-3.5" /></button>
    </div>
  {/if}
</article>
