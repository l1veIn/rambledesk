<script lang="ts">
  import type { Snippet } from 'svelte'
  import { readable } from 'svelte/store'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { unavailableVoiceInputState, useVoiceInput } from '../speech/voiceInputContext'
  import { emptyRequestSpeechTidy } from '../speech/requestSpeechTidy'
  import { useRequestSpeechTools } from '../speech/requestSpeechToolsContext'
  import RambelleStatusCard from './RambelleStatusCard.svelte'
  import organizingPortrait from '../../assets/rambelle-states/organizing.webp'

  export let portrait = ''
  export let feedbackDone = false
  export let cooking = false
  export let rambleEngaged = false
  export let rambleActive = false
  export let interactive = true
  export let inputActions: Snippet | undefined = undefined

  const voice = useVoiceInput()
  const voiceState = voice?.state ?? unavailableVoiceInputState
  const tidy = useRequestSpeechTools()
  const tidyState = tidy ?? readable(emptyRequestSpeechTidy)
  $: tidyMessage = !interactive || $tidyState.busy ? '' : $tidyState.error
    ? tr($tidyState.error)
    : !$voiceState.recording && ($tidyState.applied || $tidyState.skipped)
      ? tr('Tidied {count} segments', { count: $tidyState.applied })
        + ($tidyState.skipped ? ' · ' + tr('Skipped {count} changed or deleted segments', { count: $tidyState.skipped }) : '')
      : ''

  function tr(source: string, values: Record<string, string | number> = {}) { return t($locale, source, values) }
</script>

<section class="shrink-0" aria-label={tr('Input console')} data-request-input-console data-tour="input-console">
  <RambelleStatusCard portrait={$tidyState.busy ? organizingPortrait : portrait} {feedbackDone} {cooking}
    rambleEngaged={rambleEngaged || $voiceState.recording} rambleActive={voice ? $voiceState.recording : rambleActive}
    tidying={$tidyState.busy} message={tidyMessage} error={interactive && !!$tidyState.error}>
    {#snippet content()}
      {#if interactive && inputActions}
        <div class="mt-2 flex min-w-0 flex-wrap items-center gap-1 empty:hidden" data-request-input-actions>{@render inputActions()}</div>
      {/if}
    {/snippet}
  </RambelleStatusCard>
</section>
