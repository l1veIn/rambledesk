<script lang="ts">
  import { Check, Folder, ShieldCheck, TerminalSquare, X } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { PermissionAttentionItem, PermissionOption } from './types'

  export let item: PermissionAttentionItem
  export let busy = false
  export let answerable = true
  export let onAnswer: (option: PermissionOption) => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  function formattedToolCall(value: unknown) {
    try {
      return JSON.stringify(value, null, 2)
    } catch {
      return String(value)
    }
  }
</script>

<section class="flex h-full min-h-0 flex-col bg-background">
  <header class="flex min-h-16 shrink-0 items-center gap-3 border-b px-5 py-3">
    <span class="grid size-9 shrink-0 place-items-center rounded-md bg-warning/15 text-warning-foreground dark:text-warning"><ShieldCheck class="size-4" /></span>
    <div class="min-w-0 flex-1">
      <p class="m-0 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{tr('Permission Request')}</p>
      <h1 class="m-0 truncate text-sm font-semibold">{item.title}</h1>
    </div>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto p-6">
    <div class="mx-auto max-w-2xl">
      <h2 class="m-0 text-base font-semibold">{tr('The Agent is waiting for permission')}</h2>
      <p class="mt-2 text-xs leading-5 text-muted-foreground">{item.description}</p>

      <div class="mt-6 overflow-hidden rounded-lg border bg-muted/15">
        <div class="flex items-center gap-2 border-b px-4 py-3 text-xs font-medium"><TerminalSquare class="size-4" />{item.toolTitle}</div>
        {#if item.command}
          <pre class="m-0 overflow-x-auto whitespace-pre-wrap break-words bg-zinc-950 px-4 py-4 font-mono text-[11px] leading-5 text-zinc-100">{item.command}</pre>
        {/if}
        {#if item.path}
          <div class="flex items-center gap-2 border-t px-4 py-3 text-[10px] text-muted-foreground"><Folder class="size-3.5" /><span class="truncate">{item.path}</span></div>
        {/if}
      </div>

      {#if item.toolCall !== null && item.toolCall !== undefined}
        <details class="mt-3 rounded-md border bg-muted/10">
          <summary class="cursor-pointer px-4 py-3 text-[11px] font-medium">{tr('Full tool call')}</summary>
          <pre class="m-0 max-h-72 overflow-auto border-t px-4 py-3 font-mono text-[10px] leading-4">{formattedToolCall(item.toolCall)}</pre>
        </details>
      {/if}

      {#if item.status === 'waiting'}
        {#if !answerable}
          <p class="mt-6 rounded-md border bg-muted/25 px-4 py-3 text-xs text-muted-foreground">
            {tr('Answer the earlier request first.')}
          </p>
        {/if}
        <div class="mt-6 flex flex-wrap justify-end gap-2">
          {#each item.options as option}
            <Button
              variant={option.tone === 'allow' ? 'default' : option.tone === 'deny' ? 'destructive' : 'outline'}
              disabled={busy || !answerable}
              onclick={() => { if (answerable) onAnswer(option) }}
            >
              {#if option.tone === 'deny'}<X data-icon="inline-start" />{:else}<Check data-icon="inline-start" />{/if}
              {option.label}
            </Button>
          {/each}
        </div>
      {:else}
        <div class="mt-6 rounded-md border bg-muted/25 px-4 py-3 text-xs text-muted-foreground">{tr('This Permission Request has been answered.')}</div>
      {/if}
    </div>
  </div>
</section>
