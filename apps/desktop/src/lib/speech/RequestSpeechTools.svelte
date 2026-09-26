<script lang="ts">
  import { LoaderCircle, Sparkles } from '@lucide/svelte'
  import { readable } from 'svelte/store'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import { emptyRequestSpeechTidy } from './requestSpeechTidy'
  import { useRequestSpeechTools } from './requestSpeechToolsContext'
  const tidy = useRequestSpeechTools()
  const tidyState = tidy ?? readable(emptyRequestSpeechTidy)
  function tr(source: string, values: Record<string, string | number> = {}) { return t($locale, source, values) }
</script>

{#if tidy && ($tidyState.pendingCount || $tidyState.busy)}
  <button type="button" class="tidy" disabled={$tidyState.disabled || $tidyState.busy || !$tidyState.pendingCount} onclick={() => void tidy.run()}
    title={tr('Tidy pending speech in notes, answers, and comments. Typed text is kept.')}>
    {#if $tidyState.busy}<LoaderCircle class="size-3.5 shrink-0 animate-spin" />{:else}<Sparkles class="size-3.5 shrink-0" />{/if}
    {#if $tidyState.error && !$tidyState.busy}{tr('Retry')} · {/if}
    {tr($tidyState.busy ? 'Tidying…' : 'Tidy {count} speech segments', { count: $tidyState.pendingCount })}
  </button>
{/if}

<style>
  .tidy { display: inline-flex; align-items: center; gap: 6px; border: 1px solid color-mix(in oklab, var(--primary) 30%, var(--border)); border-radius: 8px; padding: 6px 9px; font-size: 12px; text-align: left; background: color-mix(in oklab, var(--primary) 8%, transparent); color: var(--primary); }
  .tidy:hover:not(:disabled) { background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .tidy:disabled { opacity: .45; }
  .tidy:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
</style>
