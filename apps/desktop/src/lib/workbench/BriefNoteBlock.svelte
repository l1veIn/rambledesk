<script lang="ts">
  import { LoaderCircle, StickyNote } from '@lucide/svelte'

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

  let seenNotes = notes.length
  let latestNoteIndex = -1

  $: if (notes.length > seenNotes) {
    latestNoteIndex = notes.length - 1
    seenNotes = notes.length
  } else if (notes.length < seenNotes) {
    seenNotes = notes.length
    latestNoteIndex = -1
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: recordLabel = processing
    ? tr('Transcribing note…')
    : recording
      ? tr('Recording this note')
      : tr('Record a note')
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

  {#if !disabled || recording || processing || notes.length > 0}
    <div class="flex shrink-0 items-start gap-0.5">
      {#each notes as note, index (index)}
        <RambleClipIcon
          kind="note"
          align="right"
          placement="bottom"
          compact
          index={index + 1}
          text={note}
          autoOpen={index === latestNoteIndex}
          {readOnly}
          onSave={(text) => onSaveNote(index, text)}
        />
      {/each}
      <Button
        variant="ghost"
        size="icon-xs"
        class={[
          'text-muted-foreground transition-opacity',
          recording || processing || notes.length > 0
            ? 'opacity-100'
            : 'opacity-0 group-hover/note:opacity-100 focus-visible:opacity-100',
        ]}
        disabled={disabled && !recording && !processing}
        aria-label={recordLabel}
        title={recordLabel}
        onclick={onToggleRecord}
      >
        {#if processing}
          <LoaderCircle class="size-3.5 animate-spin" />
        {:else if recording}
          <span class="relative grid size-3.5 place-items-center">
            <span class="record-blink size-2.5 rounded-full bg-destructive"></span>
          </span>
        {:else}
          <StickyNote class="size-3.5" />
        {/if}
      </Button>
    </div>
  {/if}
</div>
