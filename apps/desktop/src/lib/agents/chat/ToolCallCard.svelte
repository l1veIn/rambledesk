<!-- Svelte presentation adapted from Codeg chat tool-call components at 3ebdfed. -->
<!-- SPDX-License-Identifier: Apache-2.0; RambleDesk uses authoritative ACP snapshots and explicit incomplete status. -->
<script lang="ts">
  import { Check, ChevronRight, Circle, CircleAlert, FileCode2, LoaderCircle, Search, Terminal, Wrench } from '@lucide/svelte'
  import type { SessionToolCall } from '$lib/generated/feedback'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import { formatToolJson, locationLabel, toolPresentation } from './activity-presentation'
  import SessionContent from './SessionContent.svelte'

  export let tool: SessionToolCall
  export let runActive = false
  export let open = false
  $: title = tool.title || tool.name || chatText($locale, 'Tool activity')
  $: presentation = toolPresentation(tool.status, runActive)
  $: input = open ? formatToolJson(tool.raw_input) : ''
  $: output = open ? formatToolJson(tool.raw_output) : ''
  $: kindIcon = tool.kind === 'execute' ? Terminal : tool.kind === 'search' ? Search
    : ['edit', 'read', 'delete', 'move'].includes(tool.kind) ? FileCode2 : Wrench
</script>

<details bind:open class="group/tool min-w-0" data-tool-id={tool.id} data-tool-status={tool.status}>
  <summary class="flex w-fit max-w-full cursor-pointer list-none items-center gap-2 rounded-sm py-1 text-xs leading-5 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring">
    <svelte:component this={kindIcon} class="size-3.5 shrink-0 text-muted-foreground" />
    <span class="min-w-0 truncate font-medium" {title}>{title}</span>
    <span class={`flex shrink-0 items-center gap-1 whitespace-nowrap text-[10px] ${presentation.failed ? 'text-destructive' : 'text-muted-foreground'}`}>
      {#if presentation.spinning}<LoaderCircle class="size-3 animate-spin" />
      {:else if tool.status === 'completed'}<Check class="size-3" />
      {:else if tool.status === 'failed' || presentation.incomplete}<CircleAlert class="size-3" />
      {:else}<Circle class="size-3" />{/if}
      {chatText($locale, presentation.label)}
    </span>
    <ChevronRight aria-hidden="true" class="size-3 shrink-0 transition-transform group-open/tool:rotate-90" />
  </summary>
  {#if open}<div class="mb-2 ml-5.5 mt-2 max-h-80 min-w-0 space-y-3 overflow-auto rounded-lg border bg-muted/15 p-3 text-xs">
    {#if tool.name && tool.name !== tool.title}<p class="m-0 break-all font-mono text-[11px] text-muted-foreground">{tool.name}</p>{/if}
    {#if presentation.incomplete}<p class="m-0 text-muted-foreground">{chatText($locale, 'The tool did not report a final result before the turn stopped.')}</p>{/if}
    {#if input}
      <section aria-label={chatText($locale, 'Input')}><h4 class="mb-1 mt-0 text-[10px] font-medium text-muted-foreground">{chatText($locale, 'Input')}</h4><pre class="m-0 whitespace-pre-wrap break-words font-mono text-[11px] leading-5">{input}</pre></section>
    {/if}
    {#if tool.content.length}<SessionContent blocks={tool.content} />{/if}
    {#if output}
      <details open={tool.content.length === 0}><summary class="cursor-pointer text-[10px] font-medium text-muted-foreground">{chatText($locale, tool.content.length ? 'Raw output' : 'Output')}</summary><pre class="mb-0 mt-2 whitespace-pre-wrap break-words font-mono text-[11px] leading-5">{output}</pre></details>
    {/if}
    {#if tool.locations.length}
      <section aria-label={chatText($locale, 'Locations')}><h4 class="mb-1 mt-0 text-[10px] font-medium text-muted-foreground">{chatText($locale, 'Locations')}</h4>{#each tool.locations as location}<p class="m-0 break-all font-mono text-[11px] leading-5">{locationLabel(location)}</p>{/each}</section>
    {/if}
    {#if tool.truncated}<p class="m-0 text-[11px] text-muted-foreground">{chatText($locale, 'Content truncated by the agent host')}</p>{/if}
  </div>{/if}
</details>
