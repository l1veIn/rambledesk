<script lang="ts">
  import { FileText, LoaderCircle, Plus } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import RambleClipIcon from './RambleClipIcon.svelte'

  export let notes: string[] = []
  export let recording = false
  export let processing = false
  export let disabled = false
  export let readOnly = false
  export let partial = ''
  export let onToggleRecord: () => void = () => {}
  export let onSaveNote: (index: number, text: string) => void = () => {}

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: note = notes[0] ?? ''
  $: recordLabel = processing
    ? tr('Transcribing note…')
    : recording
      ? tr('Recording this note')
      : tr('Add a note')
  $: showAdd = !note && (!disabled || recording || processing)
</script>

<div class="group/note flex min-w-0 items-start gap-1.5 rounded-md">
  <div class="min-w-0 flex-1">
    <slot />
    {#if recording && partial}
      <p class="m-0 mt-2 truncate text-xs text-destructive">
        {tr('Listening: {text}', { text: partial })}
      </p>
    {/if}
  </div>

  {#if showAdd || note}
    <div class="flex shrink-0 items-start gap-0.5">
      {#if note}
        <RambleClipIcon
          kind="note"
          align="right"
          placement="bottom"
          compact
          index={1}
          text={note}
          {readOnly}
          {recording}
          {processing}
          onSave={(text) => onSaveNote(0, text)}
          onToggleRecord={readOnly ? null : onToggleRecord}
        />
      {:else}
        <Button
          variant="ghost"
          size="icon-xs"
          class={[
            'text-muted-foreground transition-opacity',
            recording || processing
              ? 'border-primary/50 bg-muted text-foreground opacity-100 shadow-inner'
              : 'opacity-0 group-hover/note:opacity-100 focus-visible:opacity-100',
          ]}
          disabled={disabled && !recording && !processing}
          aria-label={recordLabel}
          title={recordLabel}
          aria-pressed={recording}
          onclick={onToggleRecord}
        >
          {#if processing}
            <LoaderCircle class="size-3.5 animate-spin" />
          {:else if recording}
            <FileText class="size-3.5" />
          {:else}
            <Plus class="size-3.5" />
          {/if}
        </Button>
      {/if}
    </div>
  {/if}
</div>
