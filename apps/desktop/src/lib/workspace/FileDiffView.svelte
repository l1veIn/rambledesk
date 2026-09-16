<!-- Full-height unified diff tab. The chat card already owns per-file stats and
     the change list; this view only renders one file's recorded diff. -->
<script lang="ts">
  import { FileDiff } from '@lucide/svelte'
  import { locale } from '$lib/preferences'
  import { chatText } from '$lib/agents/chat/chat-text'
  import { diffLines } from '$lib/agents/chat/diff-lines'
  import { countUnifiedDiffLineChanges } from '$lib/agents/chat/line-change-stats'

  export let path: string
  export let diff: string

  let visibleLimit = 1000

  $: lines = diffLines(diff)
  $: stats = countUnifiedDiffLineChanges(diff)
</script>

<section class="flex h-full min-h-0 flex-col bg-background" aria-label={chatText($locale, 'Changed file')}>
  <header class="flex min-w-0 shrink-0 items-center gap-2 border-b bg-muted/30 px-4 py-2">
    <FileDiff class="size-4 shrink-0 text-muted-foreground" />
    <span class="min-w-0 flex-1 break-all font-mono text-xs">{path}</span>
    <span class="shrink-0 font-mono text-xs text-emerald-600 dark:text-emerald-400">+{stats.additions}</span>
    <span class="shrink-0 font-mono text-xs text-red-600 dark:text-red-400">−{stats.deletions}</span>
  </header>
  {#if lines.length}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to scroll long diffs.) -->
    <div class="min-h-0 flex-1 overflow-auto font-mono text-[11px] leading-5" tabindex="0" role="region" aria-label={path}>
      <div class="min-w-max">
        {#each lines.slice(0, visibleLimit) as line}
          <div class={`flex ${line.kind === 'addition' ? 'bg-emerald-500/10 text-emerald-800 dark:text-emerald-200' : line.kind === 'deletion' ? 'bg-red-500/10 text-red-800 dark:text-red-200' : line.kind === 'hunk' || line.kind === 'header' ? 'bg-muted/50 text-muted-foreground' : ''}`}>
            <span aria-hidden="true" class="w-12 shrink-0 select-none border-r px-1 text-right text-muted-foreground/65">{line.oldLine ?? ''}</span>
            <span aria-hidden="true" class="w-12 shrink-0 select-none border-r px-1 text-right text-muted-foreground/65">{line.newLine ?? ''}</span>
            <span class="whitespace-pre px-2">{line.text}</span>
          </div>
        {/each}
      </div>
    </div>
    {#if lines.length > visibleLimit}
      <div class="flex shrink-0 items-center justify-between gap-2 border-t px-4 py-2 text-xs text-muted-foreground">
        <span>{chatText($locale, 'Showing')} {visibleLimit} / {lines.length} {chatText($locale, 'lines')}</span>
        <button class="text-foreground underline underline-offset-4" onclick={() => { visibleLimit += 1000 }}>{chatText($locale, 'Show more lines')}</button>
      </div>
    {/if}
  {:else}
    <div class="grid min-h-0 flex-1 place-items-center p-6 text-center">
      <div class="max-w-sm">
        <FileDiff class="mx-auto size-5 text-muted-foreground/60" />
        <p class="mb-0 mt-2 text-xs leading-5 text-muted-foreground">{chatText($locale, 'No diff was reported for this file.')}</p>
        <p class="mb-0 mt-1 break-all font-mono text-[11px] text-muted-foreground">{path}</p>
      </div>
    </div>
  {/if}
</section>
