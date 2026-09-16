<script lang="ts">
  import { ChevronDown, LoaderCircle, Mic, X } from '@lucide/svelte'
  import { Popover } from 'bits-ui'

  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { shortcutSettings } from '$lib/settings/shortcutSettings'
  import RecordLed from '../components/ramble/RecordLed.svelte'
  import { rambleRecordPresentation } from '../components/ramble/rambleRecordButton'
  import type { RamblePhase } from '../domain/sessionPhases'

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

  $: record = rambleRecordPresentation(ramblePhase, rambleStartedOnce)
  $: primaryLabel =
    record.label === 'starting'
      ? tr('Starting…')
      : record.label === 'stopping'
        ? tr('Pausing…')
        : record.label === 'recording'
          ? tr('Recording')
          : record.label === 'resume'
            ? tr('Resume recording')
            : tr('Start recording')
</script>

<div class="flex shrink-0 items-center gap-0.5">
  <Button
    class="h-8 gap-1.5 px-2 text-xs"
    size="sm"
    variant={record.pressed ? 'destructive' : 'ghost'}
    disabled={rambleBusy || readOnly}
    onclick={onToggle}
    aria-pressed={record.pressed}
    aria-label={primaryLabel}
    title={tr('Global shortcut {shortcut}', { shortcut: $shortcutSettings.rambleToggle })}
  >
    {#if record.icon === 'spinner'}
      <LoaderCircle class="animate-spin" data-icon="inline-start" />
    {:else}
      {#if record.icon === 'recording'}
        <RecordLed />
      {/if}
      <Mic data-icon="inline-start" />
    {/if}
    <span class="hidden @min-[640px]:inline">{primaryLabel}</span>
  </Button>
  <Popover.Root>
    <Popover.Trigger>
      {#snippet child({ props })}
        <Button {...props} variant="ghost" size="icon-sm"
          class="size-8 {modelMissing || ramblePhase === 'error' ? 'text-destructive' : 'text-muted-foreground'}"
          aria-label={tr('Ramble console')} title={tr('Ramble console')}>
          <ChevronDown />
        </Button>
      {/snippet}
    </Popover.Trigger>
    <Popover.Portal>
      <Popover.Content side="top" align="start" sideOffset={8}
        class="z-[130] w-72 max-w-[calc(100vw-2rem)] rounded-lg border bg-popover p-4 text-popover-foreground shadow-lg outline-none">
        <header class="mb-3 flex items-center gap-2">
          <Mic class="size-4 text-muted-foreground" />
          <strong class="text-xs font-medium">{tr('Ramble console')}</strong>
          <Badge variant={ramblePhase === 'error' ? 'destructive' : rambleActive ? 'default' : 'secondary'} class="ml-auto text-[10px]">
            {rambleActive ? tr('Recording') : rambleEngaged ? tr('Paused') : tr('Standby')}
          </Badge>
        </header>
        <div class="space-y-2 text-xs leading-5 text-muted-foreground">
          <div class="flex items-center gap-1.5">
            <span class={rambleActive ? 'record-led' : 'inline-block size-1.5 shrink-0 rounded-full bg-muted-foreground/40'}></span>
            <span class="min-w-0 flex-1 truncate">{voiceDevice || tr('Default microphone')}</span>
            {#if voiceChunkIndex > 0}<span class="tabular-nums">{tr('{count} segments', { count: voiceChunkIndex })}</span>{/if}
          </div>
          <p class="m-0">{message || tr('Audio is transcribed locally into the document.')}</p>
          {#if modelMissing}
            <Button variant="outline" size="sm" class="w-full" onclick={onOpenVoiceSettings}>
              {tr('Download speech model')}
            </Button>
          {/if}
          {#if voicePartial}
            <p class="m-0 max-h-24 overflow-y-auto break-words text-foreground">{tr('Listening: {text}', { text: voicePartial })}</p>
          {/if}
          <div class="h-1 overflow-hidden rounded-full bg-muted" aria-label={tr('Microphone level')}>
            <span class="block h-full bg-primary transition-[width]" style={`width: ${voiceLevel * 100}%`}></span>
          </div>
          {#if rambleEngaged}
            <Button variant="outline" size="sm" class="w-full" disabled={rambleBusy || readOnly} onclick={onExit}>
              <X />{tr('Exit Ramble console')}
            </Button>
          {/if}
        </div>
      </Popover.Content>
    </Popover.Portal>
  </Popover.Root>
</div>
