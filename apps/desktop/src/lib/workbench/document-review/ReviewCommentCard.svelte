<script lang="ts">
  import { tick } from 'svelte'
  import { Check, ChevronUp, MessageSquare, PencilLine, Trash2 } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { ReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { reviewText } from './reviewI18n'
  import { suggestionDiff } from './reviewSuggestion'
  import { isImeCompositionKey } from '../../imeComposition'
  import { text } from '../definitions/validation'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { reviewAnnotationVoiceTarget, unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'

  export let annotation: ReviewAnnotation
  export let sourceLabel: string
  export let sourceVersion: string
  export let sourceText: string
  export let disabled = false
  export let onUpdate: (annotation: ReviewAnnotation) => void
  export let onDelete: () => void
  export let onCollapse: () => void
  let deleteConfirmOpen = false
  let deleteButton: HTMLButtonElement
  let cancelButton: HTMLElement | null = null
  let bodyField: WorkbenchTextField | undefined
  let replacementField: WorkbenchTextField | undefined
  let attemptedFinish = false
  const tr = (text: string) => reviewText($locale, text)
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  $: bodyTarget = reviewAnnotationVoiceTarget($voiceState, annotation.id, 'body', sourceVersion, sourceLabel)
  $: replacementTarget = reviewAnnotationVoiceTarget($voiceState, annotation.id, 'replacement', sourceVersion, sourceLabel)
  $: if (disabled) deleteConfirmOpen = false
  $: valid = text(annotation.body, 4000, annotation.kind === 'comment')
    && (annotation.kind !== 'suggestion' || text(annotation.replacement, 8000, false))
  $: unchangedSuggestion = annotation.kind === 'suggestion' && annotation.replacement === sourceText && !text(annotation.body, 4000, true)
  $: diff = suggestionDiff(sourceText, annotation.replacement ?? '')
  const update = (patch: Partial<ReviewAnnotation>) => { if (!disabled) onUpdate({ ...annotation, ...patch }) }

  export function focusComment() {
    if (!disabled) (annotation.kind === 'suggestion' ? replacementField : bodyField)?.focusAtEnd()
  }

  async function finish() {
    // Field onChange is synchronous; let the parent echo it before validating.
    await tick()
    if (disabled || bodyField?.isComposing() || replacementField?.isComposing()) return
    attemptedFinish = true
    if (valid && !unchangedSuggestion) onCollapse()
    else if (unchangedSuggestion) replacementField?.focusAtEnd()
    else bodyField?.focusAtEnd()
  }

  function keydown(event: KeyboardEvent) {
    if (event.defaultPrevented || deleteConfirmOpen || isImeCompositionKey(event)
      || bodyField?.isComposing() || replacementField?.isComposing()) return
    if (event.key === 'Escape' && !event.altKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault(); event.stopPropagation(); onCollapse()
    } else if (event.key === 'Enter' && (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey) {
      event.preventDefault(); event.stopPropagation(); void finish()
    }
  }

  function requestDelete() {
    if (disabled) return
    // An empty replacement is still a deliberate suggestion to remove text.
    if (annotation.body.trim() || annotation.replacement?.trim() || annotation.kind === 'suggestion') deleteConfirmOpen = true
    else onDelete()
  }

  function confirmDelete() {
    if (disabled || !deleteConfirmOpen) return
    deleteConfirmOpen = false
    onDelete()
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Capture local editor commands before TipTap handles them.) -->
<article class="review-comment rounded-xl border bg-background p-4 shadow-sm" data-comment-id={annotation.id} onkeydowncapture={keydown}>
  <div class="mb-3 flex items-center gap-2 text-xs">
    {#if annotation.kind === 'suggestion'}<PencilLine class="size-3.5 text-primary" />{:else}<MessageSquare class="size-3.5 text-primary" />{/if}
    <strong class="min-w-0 flex-1 truncate font-medium">{sourceLabel}</strong>
    <button bind:this={deleteButton} type="button" {disabled} aria-label={tr('Delete comment')} title={tr('Delete comment')} onclick={requestDelete} class="rounded-md p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"><Trash2 class="size-3.5" /></button>
    <button type="button" aria-label={tr('Collapse comment')} title={tr('Collapse comment')} aria-expanded="true" onclick={onCollapse} class="rounded-md p-1.5 text-muted-foreground hover:bg-muted"><ChevronUp class="size-3.5" /></button>
  </div>
  {#if annotation.quote}<blockquote class="m-0 mb-3 max-h-24 overflow-auto border-l-2 border-primary/40 pl-3 text-xs leading-5 text-muted-foreground">{annotation.quote}</blockquote>
  {:else}<p class="mb-3 text-xs text-muted-foreground">{tr('Whole document')}</p>{/if}
  {#if annotation.kind === 'suggestion'}
    <div class="grid gap-1.5 text-xs">
      <WorkbenchTextField bind:this={replacementField} value={annotation.replacement ?? ''} target={replacementTarget} {disabled} maxLength={8000}
        label={tr('Suggested wording')} voiceLabel="Speak suggested wording" tone="suggestion"
        data-review-field="replacement" onChange={(replacement) => update({ replacement })} />
      <span class="text-muted-foreground">{tr('An empty suggestion means delete this text.')}</span>
      {#if !disabled}<button type="button" onclick={() => update({ replacement: '' })} class="justify-self-start text-xs text-destructive underline">{tr('Suggest deletion')}</button>{/if}
      <details open class="my-2 rounded-md border bg-muted/20 p-2" data-review-suggestion-diff>
        <summary class="cursor-pointer text-muted-foreground">{tr('Change preview')}</summary>
        <div class="mt-2 max-h-40 overflow-auto whitespace-pre-wrap break-words leading-5">{diff.prefix}{#if diff.removed}<del class="bg-red-500/10 text-red-700 dark:text-red-300">{diff.removed}</del>{/if}{#if diff.added}<ins class="bg-emerald-500/10 text-emerald-700 no-underline dark:text-emerald-300">{diff.added}</ins>{/if}{diff.suffix}</div>
        {#if !diff.removed && !diff.added}<p class="mt-1 text-muted-foreground">{tr('No wording change yet.')}</p>{/if}
      </details>
    </div>
  {/if}
  <WorkbenchTextField bind:this={bodyField} value={annotation.body} target={bodyTarget} {disabled} maxLength={4000}
    label={tr(annotation.kind === 'suggestion' ? 'Reason (optional)' : 'Your comment')} voiceLabel="Speak comment"
    placeholder={tr('Explain what to change and why…')} data-review-field="body" onChange={(body) => update({ body })} />
  {#if !disabled && attemptedFinish && !valid}<p role="status" class="mb-0 mt-3 text-xs text-amber-700 dark:text-amber-400">{tr('Finish or remove empty comments.')}</p>{/if}
  {#if !disabled && attemptedFinish && unchangedSuggestion}<p role="status" class="mb-0 mt-3 text-xs text-amber-700 dark:text-amber-400">{tr('Change the wording or add a reason before finishing.')}</p>{/if}
  <div class="mt-3 flex flex-wrap items-center justify-between gap-2">
    <span class="text-xs text-muted-foreground">{tr('Draft only · Esc to continue reading')}</span>
    {#if !disabled}<button type="button" onclick={() => void finish()} title={tr('Ctrl/⌘ + Enter')} class="inline-flex items-center gap-1 rounded-md bg-primary px-3 py-2 text-xs text-primary-foreground"><Check class="size-3.5" />{tr('Done')}</button>{/if}
  </div>
</article>

<Dialog.Root bind:open={deleteConfirmOpen}>
  <Dialog.Content data-review-delete-dialog class="max-w-sm gap-5" showCloseButton={false}
    onOpenAutoFocus={(event) => { event.preventDefault(); cancelButton?.focus() }}
    onCloseAutoFocus={(event) => { event.preventDefault(); if (deleteButton?.isConnected && !disabled) deleteButton.focus({ preventScroll: true }) }}>
    <Dialog.Header>
      <Dialog.Title>{tr('Delete this comment?')}</Dialog.Title>
      <Dialog.Description class="mt-1 leading-5">{tr('This comment and its contents will be deleted. This cannot be undone.')}</Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer class="gap-2 sm:justify-end">
      <Button bind:ref={cancelButton} variant="outline" onclick={() => deleteConfirmOpen = false}>{tr('Cancel')}</Button>
      <Button variant="destructive" {disabled} onclick={confirmDelete}><Trash2 data-icon="inline-start" />{tr('Delete comment')}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
