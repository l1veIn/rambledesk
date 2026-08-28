<script lang="ts">
  import { Mic, Pause, Play, X } from '@lucide/svelte'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { RamblePhase } from './types'

  export let rambleEngaged = false
  export let rambleActive = false
  export let ramblePhase: RamblePhase = 'idle'
  export let rambleBusy = false
  export let rambleStartedOnce = false
  export let readOnly = false
  export let voiceDevice = ''
  export let voiceChunkIndex = 0
  export let voicePartial = ''
  export let voiceLevel = 0
  export let message = ''
  export let modelMissing = false
  export let onToggle: () => void = () => {}
  export let onExit: () => void = () => {}
  export let onOpenVoiceSettings: () => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: primaryLabel =
    ramblePhase === 'starting'
      ? tr('Starting…')
      : ramblePhase === 'stopping'
        ? tr('Pausing…')
        : rambleActive
          ? tr('Pause recording')
          : rambleStartedOnce
            ? tr('Resume recording')
            : tr('Start recording')
</script>

<section class="border-b p-4">
  <header class="mb-3 flex items-center gap-2">
    <Mic class="size-5 text-muted-foreground" />
    <strong class="text-xs font-medium">Ramble</strong>
    <Badge
      variant={ramblePhase === 'error' ? 'destructive' : rambleActive ? 'default' : 'secondary'}
      class="ml-auto h-5 px-1.5 text-[9px]"
    >
      {rambleActive ? tr('Recording') : rambleEngaged ? tr('Paused') : tr('Standby')}
    </Badge>
  </header>

  <div class="flex gap-2">
    <Button
      class="flex-1"
      variant={rambleActive ? 'secondary' : 'default'}
      disabled={rambleBusy || readOnly}
      onclick={onToggle}
      title={tr('Global shortcut Ctrl + Shift + R')}
    >
      {#if rambleActive}
        <Pause data-icon="inline-start" />
      {:else}
        <Play data-icon="inline-start" />
      {/if}
      {primaryLabel}
    </Button>
    {#if rambleEngaged}
      <Button
        variant="outline"
        size="icon"
        disabled={rambleBusy}
        onclick={onExit}
        aria-label={tr('Exit Ramble console')}
        title={tr('Exit Ramble console')}
      >
        <X />
      </Button>
    {/if}
  </div>

  <div class="mt-3 text-[10px] leading-4 text-muted-foreground">
    <div class="flex items-center gap-1.5">
      <span class={['size-1.5 rounded-full', rambleActive ? 'bg-destructive' : 'bg-muted-foreground/40']}></span>
      <span class="min-w-0 flex-1 truncate">{voiceDevice || tr('Default microphone')}</span>
      {#if voiceChunkIndex > 0}
        <span class="tabular-nums">{tr('{count} segments', { count: voiceChunkIndex })}</span>
      {/if}
    </div>
    <p class="m-0 mt-1">{message || tr('Audio is transcribed locally into the document.')}</p>
    {#if modelMissing}
      <Button variant="outline" size="sm" class="mt-2 w-full" onclick={onOpenVoiceSettings}>
        {tr('Download speech model')}
      </Button>
    {/if}
    {#if voicePartial}
      <p class="m-0 mt-1 truncate text-foreground">
        {tr('Listening: {text}', { text: voicePartial })}
      </p>
    {/if}
    <div class="mt-2 h-1 overflow-hidden rounded-full bg-muted" aria-label={tr('Microphone level')}>
      <span
        class="block h-full bg-primary transition-[width]"
        style={`width: ${voiceLevel * 100}%`}
      ></span>
    </div>
  </div>
</section>
