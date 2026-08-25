<script lang="ts">
  import { ChevronDown, ChevronRight, LoaderCircle, StickyNote } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'

  export let notes: string[] = []
  export let recording = false
  export let processing = false
  export let disabled = false
  export let readOnly = false
  export let partial = ''
  export let onToggleRecord: () => void = () => {}
  export let onSaveNote: (index: number, text: string) => void = () => {}

  let drafts: string[] = []

  let expanded = true
  let seenNotes = 0

  $: if (notes.length > seenNotes) {
    expanded = true
    seenNotes = notes.length
  }
  $: if (drafts.length !== notes.length || notes.some((note, index) => drafts[index] === undefined)) {
    drafts = [...notes]
  }

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: recordLabel = processing
    ? tr('Transcribing note…')
    : recording
      ? tr('Recording this note')
      : tr('Record a note')

  function saveNote(index: number) {
    const next = (drafts[index] ?? '').trim()
    if (!next || next === notes[index]) return
    onSaveNote(index, next)
  }
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
        <div class="mt-2 grid gap-3">
          {#each notes as note, index (`${index}:${note}`)}
            <div class="grid gap-1.5">
              <textarea
                class="min-h-16 w-full resize-y rounded-md border bg-background px-2 py-1.5 text-[13px] leading-6 outline-none focus-visible:ring-2 focus-visible:ring-ring"
                value={drafts[index] ?? note}
                readonly={readOnly}
                aria-label={tr('Note {index}', { index: index + 1 })}
                oninput={(event) => {
                  const next = [...drafts]
                  next[index] = (event.currentTarget as HTMLTextAreaElement).value
                  drafts = next
                }}
                onkeydown={(event) => {
                  if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
                    event.preventDefault()
                    saveNote(index)
                  }
                }}
              ></textarea>
              {#if !readOnly}
                <div class="flex justify-end">
                  <Button
                    size="xs"
                    disabled={(drafts[index] ?? note).trim() === note.trim() || !(drafts[index] ?? '').trim()}
                    onclick={() => saveNote(index)}
                  >
                    {tr('Save')}
                  </Button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
