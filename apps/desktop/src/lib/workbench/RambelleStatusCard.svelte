<script lang="ts">
  import type { Snippet } from 'svelte'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let portrait = ''
  export let feedbackDone = false
  export let cooking = false
  export let rambleEngaged = false
  export let rambleActive = false
  export let tidying = false
  export let message = ''
  export let error = false
  export let content: Snippet | undefined = undefined

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: line = feedbackDone
    ? tr('Package sealed, commander. I will not touch it again.')
    : tidying
      ? tr('Tidying…')
      : message
        ? message
        : cooking
          ? tr('Commander, I am cooking this feedback now.')
          : rambleEngaged && rambleActive
            ? tr('Commander, I am recording this now.')
            : rambleEngaged
              ? tr('Commander, I paused. Say the word and I will follow again.')
              : tr('Commander, standing by. Start a Ramble when you want me.')
</script>

<section
  class="flex shrink-0 items-center gap-3 border-t bg-muted/15 px-3 py-3"
  aria-label={tr('Rambelle status')}
  data-rambelle-status
>
  {#if portrait}
    <img src={portrait} alt="Rambelle" class="size-20 shrink-0 object-contain" />
  {/if}
  <div class="rambelle-bubble relative min-w-0 flex-1 rounded-2xl border bg-card px-3 py-2.5" data-rambelle-bubble>
    <p class="m-0 break-words text-xs leading-5 text-muted-foreground" class:text-destructive={error} role={error ? 'alert' : 'status'}>{line}</p>
    {@render content?.()}
  </div>
</section>

<style>
  .rambelle-bubble::before {
    content: '';
    position: absolute;
    left: -5px;
    top: 20px;
    width: 8px;
    height: 8px;
    transform: rotate(45deg);
    border-left: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: var(--card);
  }
</style>

