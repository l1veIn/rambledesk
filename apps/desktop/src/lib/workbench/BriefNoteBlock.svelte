<script lang="ts">
  import { LoaderCircle, Pencil, StickyNote } from '@lucide/svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import { nextSavedTranscript } from './briefNotes'

  export let notes: string[] = []
  export let recording = false
  export let processing = false
  export let disabled = false
  export let readOnly = false
  export let partial = ''
  export let onToggleRecord: () => void = () => {}
  export let onSaveNote: (index: number, text: string) => void = () => {}

  let heldPartial = ''
  let editing = false
  let draft = ''

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: note = notes[0] ?? ''
  $: if (recording && partial.trim()) heldPartial = partial.trim()
  // Hold the last spoken words on screen from the moment recording stops until
  // the cleaned note lands, so nothing blinks out of existence mid-transcription.
  $: if (!recording && !processing) heldPartial = ''
  $: pending = recording ? partial.trim() : heldPartial
  $: display = [note, pending].filter((part) => part.length > 0).join('\n')
  $: canEdit = !readOnly && !recording && !processing && note.length > 0
  $: dirty = nextSavedTranscript(draft, note) !== null
  $: sourceRecordLabel = recording
    ? tr('Recording this note')
    : processing
      ? tr('Transcribing note…')
      : tr('Record a note')

  function openEditor() {
    if (!canEdit) return
    draft = `${note}\n`
    editing = true
  }

  function save() {
    const next = (draft.trim() || note).trim()
    if (!next) return
    onSaveNote(0, next)
    editing = false
  }

  function onDraftKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
      event.preventDefault()
      save()
    }
  }
</script>

<div class="group/note min-w-0 rounded-md">
  <div class="flex items-start gap-1.5">
    <div class="min-w-0 flex-1">
      <slot />
    </div>
    {#if !disabled || recording || processing}
      <Button
        variant={recording ? 'outline' : 'ghost'}
        size="icon-xs"
        class={[
          recording
            ? 'rounded-full border-destructive/40'
            : 'text-muted-foreground opacity-0 group-hover/note:opacity-100 focus-visible:opacity-100',
        ]}
        disabled={processing || (disabled && !recording)}
        aria-label={sourceRecordLabel}
        title={sourceRecordLabel}
        aria-pressed={recording}
        onclick={onToggleRecord}
      >
        {#if processing}
          <LoaderCircle class="size-3.5 animate-spin" />
        {:else if recording}
          <span class="record-blink size-2.5 rounded-full bg-destructive"></span>
        {:else}
          <StickyNote class="size-3.5" />
        {/if}
      </Button>
    {/if}
  </div>

  {#if display || editing}
    <div class="group/preview relative mt-2 pr-8">
      {#if canEdit}
        <Button
          variant="ghost"
          size="icon-xs"
          class="absolute right-0 top-0 text-muted-foreground opacity-0 group-hover/preview:opacity-100 focus-visible:opacity-100"
          aria-label={tr('Edit note')}
          title={tr('Edit note')}
          onclick={openEditor}
        >
          <Pencil class="size-3.5" />
        </Button>
      {/if}
      {#if editing}
        <textarea
          class="min-h-16 w-full resize-y rounded-md border bg-background px-2 py-1.5 text-xs leading-5 text-destructive outline-none focus-visible:ring-2 focus-visible:ring-ring"
          value={draft}
          aria-label={tr('Edit note')}
          oninput={(event) => (draft = (event.currentTarget as HTMLTextAreaElement).value)}
          onkeydown={onDraftKeydown}
        ></textarea>
        <div class="mt-1.5 flex items-center justify-end gap-2">
          <Button size="xs" disabled={!dirty} onclick={save}>{tr('Save')}</Button>
        </div>
      {:else}
        <p class="m-0 whitespace-pre-wrap text-xs leading-5 text-destructive">{display}</p>
      {/if}
    </div>
  {/if}
</div>
