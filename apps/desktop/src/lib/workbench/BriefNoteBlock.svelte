<script lang="ts">
  import { LoaderCircle, Pencil, StickyNote } from '@lucide/svelte'
  import { onMount, tick } from 'svelte'

  import { Button } from '$lib/components/ui/button'
  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import {
    isCaptureTooltipEvent,
    nextSavedTranscript,
    tooltipFixedStyle,
  } from './briefNotes'

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
  let root: HTMLDivElement
  let popover: HTMLDivElement | undefined
  let popoverStyle = ''

  function tr(source: string, values: Record<string, string | number> = {}) {
    return t($locale, source, values)
  }

  $: note = notes[0] ?? ''
  $: if (recording && partial.trim()) heldPartial = partial.trim()
  $: if (note) heldPartial = ''
  $: display = note
    ? recording && partial.trim()
      ? `${note}\n${partial.trim()}`
      : note
    : recording
      ? partial.trim()
      : heldPartial
  $: dirty = nextSavedTranscript(draft, note) !== null
  $: recordLabel = recording
    ? tr('Recording this note')
    : processing
      ? tr('Transcribing note…')
      : note
        ? tr('Edit note')
        : tr('Record a note')

  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return {
      destroy() {
        node.remove()
      },
    }
  }

  function updatePopoverPosition() {
    if (!root) return
    const placed = tooltipFixedStyle(root.getBoundingClientRect(), 'bottom', 'right')
    popoverStyle = `top:${placed.top}px;left:${placed.left}px;transform:${placed.transform}`
  }

  function openEditor() {
    if (readOnly || !note) return
    draft = note
    updatePopoverPosition()
    editing = true
    void tick().then(updatePopoverPosition)
  }

  function save() {
    const next = nextSavedTranscript(draft, note)
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

  function onWindowPointerDown(event: PointerEvent) {
    if (!editing) return
    const target = event.target as HTMLElement | null
    if (root.contains(target) || isCaptureTooltipEvent(target)) return
    editing = false
  }

  function onWindowKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') editing = false
  }

  onMount(() => {
    window.addEventListener('pointerdown', onWindowPointerDown, true)
    window.addEventListener('keydown', onWindowKeyDown)
    window.addEventListener('resize', updatePopoverPosition)
    return () => {
      window.removeEventListener('pointerdown', onWindowPointerDown, true)
      window.removeEventListener('keydown', onWindowKeyDown)
      window.removeEventListener('resize', updatePopoverPosition)
    }
  })
</script>

<div bind:this={root} class="group/note flex min-w-0 items-start gap-1.5 rounded-md">
  <div class="min-w-0 flex-1">
    <slot />
    {#if display}
      <p class="m-0 mt-2 whitespace-pre-wrap text-xs leading-5 text-destructive">
        {display}
      </p>
    {/if}
  </div>

  {#if !disabled || recording || processing || note}
    <Button
      variant="ghost"
      size="icon-xs"
      class={[
        'text-muted-foreground transition-opacity',
        recording || processing || note
          ? 'opacity-100'
          : 'opacity-0 group-hover/note:opacity-100 focus-visible:opacity-100',
      ]}
      disabled={disabled && !recording && !processing}
      aria-label={recordLabel}
      title={recordLabel}
      aria-pressed={recording}
      onclick={() => {
        if (recording || processing || !note) onToggleRecord()
        else openEditor()
      }}
    >
      {#if processing}
        <LoaderCircle class="size-3.5 animate-spin" />
      {:else if recording}
        <span class="record-blink size-2.5 rounded-full bg-destructive"></span>
      {:else if note}
        <Pencil class="size-3.5" />
      {:else}
        <StickyNote class="size-3.5" />
      {/if}
    </Button>
  {/if}
</div>

{#if editing}
  <div
    bind:this={popover}
    use:portal
    data-capture-tooltip
    class="fixed z-[200] w-[min(22rem,calc(100vw-4rem))] rounded-md border bg-popover p-3 text-xs leading-5 text-popover-foreground shadow-lg"
    style={popoverStyle}
    role="dialog"
    tabindex="-1"
    aria-label={tr('Edit note')}
    onpointerdown={(event) => event.stopPropagation()}
  >
    <div class="mb-1 flex items-center gap-1.5">
      <strong class="min-w-0 flex-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        {tr('Edit note')}
      </strong>
      {#if !readOnly}
        <button
          type="button"
          class="grid size-6 place-items-center rounded-full text-muted-foreground hover:bg-muted"
          aria-label={recording ? tr('Recording this note') : tr('Record more')}
          title={recording ? tr('Recording this note') : tr('Record more')}
          onclick={onToggleRecord}
        >
          <span
            class="size-2.5 rounded-full {recording
              ? 'record-blink bg-destructive'
              : 'bg-muted-foreground/50'}"
          ></span>
        </button>
      {/if}
    </div>
    <textarea
      class="mt-1 max-h-48 min-h-24 w-full resize-y rounded-md border bg-background px-2 py-1.5 text-xs leading-5 text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
      value={draft}
      readonly={readOnly}
      aria-label={tr('Edit note')}
      oninput={(event) => (draft = (event.currentTarget as HTMLTextAreaElement).value)}
      onkeydown={onDraftKeydown}
    ></textarea>
    {#if !readOnly}
      <div class="mt-2 flex justify-end">
        <Button size="xs" disabled={!dirty} onclick={save}>{tr('Save')}</Button>
      </div>
    {/if}
  </div>
{/if}
