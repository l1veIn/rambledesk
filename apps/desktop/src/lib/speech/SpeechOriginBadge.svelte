<script lang="ts">
  import { Mic } from '@lucide/svelte'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import { fieldSpeechSegmentCounts } from './fieldSpeechSegments'
  import type { SpeechTarget } from './speechTargets'
  import { unavailableVoiceInputState, useVoiceInput } from './voiceInputContext'

  export let target: SpeechTarget | null
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  $: snapshot = $voiceState.draftSnapshot
  $: counts = snapshot && target?.requestId === $voiceState.requestId
    ? fieldSpeechSegmentCounts(snapshot, target) : { pending: 0, cleaned: 0 }
  $: total = counts.pending + counts.cleaned
  function tr(source: string, count: number) { return t($locale, source, { count }) }
</script>

{#if total > 0}
  <span class="inline-flex shrink-0 items-center gap-1 rounded bg-muted/60 px-1.5 py-0.5 text-[10px] text-muted-foreground" data-speech-origin-badge>
    <Mic class="size-3" aria-hidden="true" />
    {#if counts.pending > 0}<span class="text-amber-700 dark:text-amber-400">{tr('{count} speech segments to tidy', counts.pending)}</span>{/if}
    {#if counts.pending > 0 && counts.cleaned > 0}<span aria-hidden="true">·</span>{/if}
    {#if counts.cleaned > 0}<span>{tr('{count} speech segments tidied', counts.cleaned)}</span>{/if}
  </span>
{/if}
