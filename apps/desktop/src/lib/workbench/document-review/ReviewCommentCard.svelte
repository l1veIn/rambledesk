<script lang="ts">
  import { MessageSquare, PencilLine, MapPin, Check, RotateCcw, Trash2, X } from '@lucide/svelte'
  import type { ReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { reviewText } from './reviewI18n'

  export let annotation: ReviewAnnotation
  export let paragraphLabel: string
  export let disabled = false
  export let contextual = false
  export let onUpdate: (annotation: ReviewAnnotation) => void
  export let onDelete: () => void
  export let onLocate: () => void
  export let onClose: () => void = () => {}
  const tr = (text: string) => reviewText($locale, text)
  const update = (patch: Partial<ReviewAnnotation>) => { if (!disabled) onUpdate({ ...annotation, ...patch }) }
  function updateText(event: Event, field: 'body' | 'replacement', limit: number) {
    const input = event.currentTarget as HTMLTextAreaElement
    const value = [...input.value].slice(0, limit).join('')
    if (input.value !== value) input.value = value
    update({ [field]: value })
  }
  function focusNew(node: HTMLTextAreaElement) { if (!disabled && !annotation.body) node.focus({ preventScroll: true }) }
</script>

<article class="review-comment rounded-xl border bg-background p-4 shadow-sm" class:opacity-70={annotation.status === 'resolved'} data-comment-id={annotation.id}>
  <div class="mb-3 flex items-center gap-2 text-xs">
    {#if annotation.kind === 'suggestion'}<PencilLine class="size-3.5 text-primary" />{:else}<MessageSquare class="size-3.5 text-primary" />{/if}
    <strong class="min-w-0 flex-1 truncate font-medium">{paragraphLabel}</strong>
    <span class="text-muted-foreground">{tr(annotation.status === 'resolved' ? 'Resolved' : 'Open')}</span>
    {#if contextual}<button type="button" aria-label={tr('Close comment')} title={tr('Close comment')} onclick={onClose} class="rounded p-1 hover:bg-muted"><X class="size-4" /></button>{/if}
  </div>
  {#if annotation.quote}<blockquote class="m-0 mb-3 max-h-24 overflow-auto border-l-2 border-primary/40 pl-3 text-xs leading-5 text-muted-foreground">{annotation.quote}</blockquote>
  {:else}<p class="mb-3 text-xs text-muted-foreground">{tr('Whole paragraph')}</p>{/if}
  <label class="mb-3 grid gap-1.5 text-xs">
    <span>{tr('Your comment')}</span>
    <textarea use:focusNew value={annotation.body} {disabled} rows="3" aria-label={tr('Your comment')}
      placeholder={tr('Explain what to change and why…')} oninput={(event) => updateText(event, 'body', 4000)}
      class="w-full resize-y rounded-lg border bg-background px-3 py-2 text-sm leading-6 focus-visible:outline-ring disabled:opacity-70"></textarea>
  </label>
  {#if annotation.kind === 'suggestion'}
    <label class="mb-3 grid gap-1.5 text-xs">
      <span>{tr('Suggested wording')}</span>
      <textarea value={annotation.replacement ?? ''} {disabled} rows="3" aria-label={tr('Suggested wording')}
        oninput={(event) => updateText(event, 'replacement', 8000)}
        class="w-full resize-y rounded-lg border border-emerald-600/25 bg-emerald-500/5 px-3 py-2 text-sm leading-6 focus-visible:outline-ring disabled:opacity-70"></textarea>
      <span class="text-muted-foreground">{tr('An empty suggestion means delete this text.')}</span>
    </label>
  {/if}
  {#if !disabled && !annotation.body.trim()}<p class="mb-3 text-xs text-amber-700 dark:text-amber-400">{tr('Finish or remove empty comments.')}</p>{/if}
  <div class="flex flex-wrap items-center gap-2 text-xs">
    <button type="button" onclick={onLocate} class="inline-flex items-center gap-1 rounded-md border px-2 py-1.5 hover:bg-muted"><MapPin class="size-3" />{tr('Locate in original')}</button>
    <button type="button" {disabled} onclick={() => update({ status: annotation.status === 'open' ? 'resolved' : 'open' })} class="inline-flex items-center gap-1 rounded-md px-2 py-1.5 hover:bg-muted disabled:opacity-40">
      {#if annotation.status === 'open'}<Check class="size-3" />{:else}<RotateCcw class="size-3" />{/if}{tr(annotation.status === 'open' ? 'Resolve' : 'Reopen')}
    </button>
    <button type="button" {disabled} aria-label={tr('Delete comment')} title={tr('Delete comment')} onclick={onDelete} class="ml-auto rounded-md p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"><Trash2 class="size-3.5" /></button>
  </div>
</article>
