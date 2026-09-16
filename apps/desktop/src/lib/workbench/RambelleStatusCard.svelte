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
  class="flex shrink-0 items-center gap-2 border-t bg-muted/25 px-3 py-1.5"
  aria-label={tr('Rambelle status')}
>
  {#if portrait}
    <img src={portrait} alt="Rambelle" class="size-6 shrink-0 object-contain" />
  {/if}
  <strong class="shrink-0 text-[10px] font-semibold">Rambelle</strong>
  <p class="m-0 min-w-0 flex-1 truncate text-[11px] text-muted-foreground" title={line}>{line}</p>
</section>

