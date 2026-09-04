<script lang="ts">
  import { FileDiff } from '@lucide/svelte'
  import { locale } from '$lib/preferences'
  import { chatText } from './chat-text'
  import { generateUnifiedDiff } from './unified-diff-generator'
  import { countUnifiedDiffLineChanges } from './line-change-stats'
  import { diffLines } from './diff-lines'

  export let path: string
  export let oldText: string | null
  export let newText: string
  let visibleLimit = 500
  $: diff = generateUnifiedDiff(oldText ?? '', newText, path)
  $: stats = countUnifiedDiffLineChanges(diff ?? '')
  $: lines = diffLines(diff)
</script>

<section class="overflow-hidden rounded-md border bg-background text-xs" aria-label={chatText($locale, 'Changed file')}>
  <div class="flex min-w-0 items-center gap-2 border-b bg-muted/30 px-3 py-2">
    <FileDiff class="size-3.5 shrink-0 text-muted-foreground" />
    <span class="min-w-0 flex-1 break-all font-mono">{path}</span>
    <span class="shrink-0 font-mono text-emerald-600 dark:text-emerald-400">+{stats.additions}</span>
    <span class="shrink-0 font-mono text-red-600 dark:text-red-400">−{stats.deletions}</span>
  </div>
  {#if lines.length}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to scroll long diffs.) -->
    <div class="max-h-96 overflow-auto font-mono text-[11px] leading-5" tabindex="0" role="region" aria-label={path}>
      <div class="min-w-max">
        {#each lines.slice(0, visibleLimit) as line}
          <div class={`flex ${line.kind === 'addition' ? 'bg-emerald-500/10 text-emerald-800 dark:text-emerald-200' : line.kind === 'deletion' ? 'bg-red-500/10 text-red-800 dark:text-red-200' : line.kind === 'hunk' || line.kind === 'header' ? 'bg-muted/50 text-muted-foreground' : ''}`}>
            <span aria-hidden="true" class="w-10 shrink-0 select-none border-r px-1 text-right text-muted-foreground/65">{line.oldLine ?? ''}</span>
            <span aria-hidden="true" class="w-10 shrink-0 select-none border-r px-1 text-right text-muted-foreground/65">{line.newLine ?? ''}</span>
            <span class="whitespace-pre px-2">{line.text}</span>
          </div>
        {/each}
      </div>
    </div>
    {#if lines.length > visibleLimit}
      <div class="flex items-center justify-between gap-2 border-t px-3 py-2 text-muted-foreground">
        <span>{chatText($locale, 'Showing')} {visibleLimit} / {lines.length} {chatText($locale, 'lines')}</span>
        <button class="text-foreground underline underline-offset-4" onclick={() => { visibleLimit += 500 }}>{chatText($locale, 'Show more lines')}</button>
      </div>
    {/if}
  {:else}<p class="m-0 px-3 py-2 text-muted-foreground">{chatText($locale, 'No line changes')}</p>{/if}
</section>
