<script lang="ts">
  import { Mic } from '@lucide/svelte'
  import { t } from '../i18n'
  import { locale } from '../preferences'
  import type { SpeechTarget } from './speechDraftQueue'
  import { sameVoiceInputTarget, speechTargetLabel } from './speechTargetLabel'
  import { unavailableVoiceInputState, useVoiceInput } from './voiceInputContext'

  export let target: SpeechTarget | null = null
  export let disabled = false
  export let label = 'Speak here'
  export let iconOnly = false
  export let toggle = false
  const voice = useVoiceInput()
  const voiceState = voice?.state ?? unavailableVoiceInputState
  $: active = sameVoiceInputTarget(target, $voiceState.nextTarget)
  $: locked = disabled || $voiceState.disabled || !target || target.destination.kind === 'unknown' || target.requestId !== $voiceState.requestId
  $: actionLabel = toggle && active && $voiceState.recording ? 'Pause recording' : label
  $: title = `${t($locale, actionLabel)}${target ? ` · ${speechTargetLabel(target, (source) => t($locale, source))}` : ''}`
  function activate() {
    if (!voice || locked || !target) return
    if (toggle && active && $voiceState.recording) void voice.stop()
    else void voice.start(target)
  }
</script>

{#if voice}
  <button type="button" data-voice-input class="inline-flex shrink-0 items-center gap-1 rounded-md border px-2 py-1 text-xs text-muted-foreground hover:bg-muted aria-pressed:border-primary aria-pressed:bg-primary/10 aria-pressed:text-primary disabled:opacity-40"
    class:recording={active && $voiceState.recording}
    disabled={locked} aria-label={t($locale, actionLabel)} aria-pressed={active} {title}
    onpointerdown={(event) => event.preventDefault()} onclick={activate}>
    <Mic class="size-3.5" />
    {#if !iconOnly}<span>{t($locale, label)}</span>{/if}
  </button>
{/if}

<style>
  .recording { color: var(--destructive); border-color: var(--destructive); background: color-mix(in srgb, var(--destructive) 10%, transparent); }
</style>
