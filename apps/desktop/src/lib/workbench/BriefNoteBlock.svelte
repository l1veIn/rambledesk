<script lang="ts">
  import { ChevronDown, ChevronRight, LoaderCircle, StickyNote } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let notes: string[] = []
  export let recording = false
  export let processing = false
  export let disabled = false
  export let partial = ''
  export let onToggleRecord: () => void = () => {}

  let expanded = true
  let seenNotes = 0

  $: if (notes.length > seenNotes) {
    expanded = true
    seenNotes = notes.length
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

<div class="group/note relative min-w-0 rounded-md">
  <div class="pr-9">
    <slot />
  </div>

  {#if !disabled || recording || processing || notes.length > 0}
    <Button
      variant="ghost"
      size="icon-xs"
      class={[
        'absolute right-0 top-0.5 text-muted-foreground transition-opacity',
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
  {/if}

  {#if recording && partial}
    <p class="m-0 mt-2 truncate text-xs text-destructive">
      {tr('Listening: {text}', { text: partial })}
    </p>
  {/if}

  {#if notes.length > 0}
    <div
      class="mt-2 rounded-md border border-warning/40 bg-warning/15 px-3 py-2 text-[13px] leading-6 text-foreground shadow-sm"
    >
      <button
        type="button"
        class="flex w-full items-center gap-1.5 text-left text-[11px] font-medium text-warning-foreground"
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
      >
        {#if expanded}
          <ChevronDown class="size-3.5" />
        {:else}
          <ChevronRight class="size-3.5" />
        {/if}
        {expanded ? tr('Collapse note') : tr('Expand note')}
        <span class="ml-auto tabular-nums text-muted-foreground">
          {tr('{count} notes', { count: notes.length })}
        </span>
      </button>
      {#if expanded}
        <div class="mt-2 grid gap-2">
          {#each notes as note, index (`${index}:${note}`)}
            <p class="m-0 whitespace-pre-wrap">{note}</p>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
