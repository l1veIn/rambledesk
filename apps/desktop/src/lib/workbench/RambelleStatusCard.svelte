<script lang="ts">
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let portrait = ''
  export let feedbackDone = false
  export let cooking = false
  export let rambleEngaged = false
  export let rambleActive = false

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: line = feedbackDone
    ? tr('Package sealed, commander. I will not touch it again.')
    : cooking
      ? tr('Commander, I am cooking this feedback now.')
      : rambleEngaged && rambleActive
        ? tr('Commander, I am recording this now.')
        : rambleEngaged
          ? tr('Commander, I paused. Say the word and I will follow again.')
          : tr('Commander, standing by. Start a Ramble when you want me.')
</script>

<section
  class="flex min-h-[136px] shrink-0 items-start gap-2 border-t bg-muted/25 px-3 pb-2 pt-2"
  aria-label={tr('Rambelle status')}
>
  {#if portrait}
    <img src={portrait} alt="Rambelle" class="size-[120px] shrink-0 self-start object-contain object-top" />
  {/if}
  <div class="rambelle-bubble relative mt-3 min-w-0 flex-1 self-start rounded-2xl border bg-background px-3 py-2">
    <strong class="block text-[10px] font-semibold">Rambelle</strong>
    <p class="m-0 mt-1 text-[11px] leading-4">{line}</p>
  </div>
</section>

<style>
  .rambelle-bubble {
    filter: drop-shadow(0 1px 2px rgb(15 35 55 / 12%));
  }

  .rambelle-bubble::before {
    content: '';
    position: absolute;
    left: -7px;
    bottom: 15px;
    width: 12px;
    height: 12px;
    border-bottom: 1px solid var(--border);
    border-left: 1px solid var(--border);
    background: var(--background);
    transform: rotate(45deg);
  }
</style>

