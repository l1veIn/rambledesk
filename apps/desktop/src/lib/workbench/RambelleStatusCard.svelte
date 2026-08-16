<script lang="ts">
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let portrait = ''
  export let feedbackDone = false
  export let rambleEngaged = false

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }
</script>

<section class="flex min-h-20 items-center gap-3 border-b bg-muted/25 px-4 py-3" aria-label={tr('Rambelle status')}>
  {#if portrait}
    <img src={portrait} alt="Rambelle" class="size-12 shrink-0 object-contain" />
  {/if}
  <div class="min-w-0">
    <strong class="block text-[10px] font-medium">Rambelle</strong>
    <p class="m-0 mt-1 text-[10px] leading-4 text-muted-foreground">
      {feedbackDone
        ? tr('The feedback package has been published.')
        : rambleEngaged
          ? tr('Organizing this recording.')
          : tr('Start recording when you are ready.')}
    </p>
  </div>
</section>
