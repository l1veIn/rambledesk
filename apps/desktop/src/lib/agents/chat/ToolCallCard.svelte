<!-- Svelte presentation adapted from Codeg chat tool-call components at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; RambleDesk uses authoritative ACP snapshots and explicit incomplete status. -->
<script lang="ts">
  import { Check, Circle, CircleAlert, FileCode2, LoaderCircle, Search, Terminal, Wrench } from '@lucide/svelte'
  import type { SessionToolCall } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import { formatToolJson, locationLabel, toolPresentation } from './activity-presentation'
  import SessionContent from './SessionContent.svelte'

  export let tool: SessionToolCall
  export let runActive = false
  export let open = false
  $: presentation = toolPresentation(tool.status, runActive)
  $: input = open ? formatToolJson(tool.raw_input) : ''
  $: output = open ? formatToolJson(tool.raw_output) : ''
  $: kindIcon = tool.kind === 'execute' ? Terminal : tool.kind === 'search' ? Search
    : ['edit', 'read', 'delete', 'move'].includes(tool.kind) ? FileCode2 : Wrench
</script>

<details bind:open class="group/tool overflow-hidden rounded-lg border bg-muted/15" data-tool-id={tool.id} data-tool-status={tool.status}>
  <summary class="flex cursor-pointer list-none items-center gap-2 px-3 py-2.5 text-xs">
    <svelte:component this={kindIcon} class="size-3.5 shrink-0 text-muted-foreground" />
    <span class="min-w-0 flex-1 break-words font-medium">{tool.title || tool.name || chatText($locale, 'Tool activity')}</span>
    <span class={`flex shrink-0 items-center gap-1.5 text-[10px] ${presentation.failed ? 'text-destructive' : 'text-muted-foreground'}`}>
      {#if presentation.spinning}<LoaderCircle class="size-3 animate-spin" />
      {:else if tool.status === 'completed'}<Check class="size-3" />
      {:else if tool.status === 'failed' || presentation.incomplete}<CircleAlert class="size-3" />
      {:else}<Circle class="size-3" />{/if}
      {chatText($locale, presentation.label)}
    </span>
    <span aria-hidden="true" class="text-muted-foreground transition-transform group-open/tool:rotate-90">›</span>
  </summary>
  {#if open}<div class="space-y-3 border-t bg-background/60 p-3 text-xs">
    {#if tool.name && tool.name !== tool.title}<p class="m-0 break-all font-mono text-[11px] text-muted-foreground">{tool.name}</p>{/if}
    {#if presentation.incomplete}<p class="m-0 text-muted-foreground">{chatText($locale, 'The tool did not report a final result before the turn stopped.')}</p>{/if}
    {#if input}
      <section aria-label={chatText($locale, 'Input')}><h4 class="mb-1 mt-0 text-[10px] font-medium text-muted-foreground">{chatText($locale, 'Input')}</h4><pre class="m-0 max-h-64 overflow-auto whitespace-pre-wrap break-words rounded bg-muted/35 p-2 font-mono text-[11px] leading-5">{input}</pre></section>
    {/if}
    {#if tool.content.length}<SessionContent blocks={tool.content} />{/if}
    {#if output}
      <details open={tool.content.length === 0} class="rounded border px-2 py-1.5"><summary class="cursor-pointer text-[10px] font-medium text-muted-foreground">{chatText($locale, tool.content.length ? 'Raw output' : 'Output')}</summary><pre class="mb-0 mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-5">{output}</pre></details>
    {/if}
    {#if tool.locations.length}
      <section aria-label={chatText($locale, 'Locations')}><h4 class="mb-1 mt-0 text-[10px] font-medium text-muted-foreground">{chatText($locale, 'Locations')}</h4>{#each tool.locations as location}<p class="m-0 break-all font-mono text-[11px] leading-5">{locationLabel(location)}</p>{/each}</section>
    {/if}
    {#if tool.truncated}<p class="m-0 text-[11px] text-muted-foreground">{chatText($locale, 'Content truncated by the agent host')}</p>{/if}
  </div>{/if}
</details>
