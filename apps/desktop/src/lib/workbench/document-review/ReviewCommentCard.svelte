<script lang="ts">
  import { ChevronUp, MessageSquare, PencilLine, Trash2 } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { ReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { reviewText } from './reviewI18n'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { reviewAnnotationVoiceTarget, unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'

  export let annotation: ReviewAnnotation
  export let paragraphLabel: string
  export let sourceVersion: string
  export let disabled = false
  export let onUpdate: (annotation: ReviewAnnotation) => void
  export let onDelete: () => void
  export let onCollapse: () => void
  let deleteConfirmOpen = false
  let deleteButton: HTMLButtonElement
  let cancelButton: HTMLElement | null = null
  let bodyField: WorkbenchTextField | undefined
  const tr = (text: string) => reviewText($locale, text)
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  $: bodyTarget = reviewAnnotationVoiceTarget($voiceState, annotation.id, 'body', sourceVersion, paragraphLabel)
  $: replacementTarget = reviewAnnotationVoiceTarget($voiceState, annotation.id, 'replacement', sourceVersion, paragraphLabel)
  $: if (disabled) deleteConfirmOpen = false
  const update = (patch: Partial<ReviewAnnotation>) => { if (!disabled) onUpdate({ ...annotation, ...patch }) }

  export function focusComment() {
    if (!disabled) bodyField?.focusAtEnd()
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

<article class="review-comment rounded-xl border bg-background p-4 shadow-sm" data-comment-id={annotation.id}>
  <div class="mb-3 flex items-center gap-2 text-xs">
    {#if annotation.kind === 'suggestion'}<PencilLine class="size-3.5 text-primary" />{:else}<MessageSquare class="size-3.5 text-primary" />{/if}
    <strong class="min-w-0 flex-1 truncate font-medium">{paragraphLabel}</strong>
    <button bind:this={deleteButton} type="button" {disabled} aria-label={tr('Delete comment')} title={tr('Delete comment')} onclick={requestDelete} class="rounded-md p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"><Trash2 class="size-3.5" /></button>
    <button type="button" aria-label={tr('Collapse comment')} title={tr('Collapse comment')} aria-expanded="true" onclick={onCollapse} class="rounded-md p-1.5 text-muted-foreground hover:bg-muted"><ChevronUp class="size-3.5" /></button>
  </div>
  {#if annotation.quote}<blockquote class="m-0 mb-3 max-h-24 overflow-auto border-l-2 border-primary/40 pl-3 text-xs leading-5 text-muted-foreground">{annotation.quote}</blockquote>
  {:else}<p class="mb-3 text-xs text-muted-foreground">{tr('Whole paragraph')}</p>{/if}
  <div>
    <WorkbenchTextField bind:this={bodyField} value={annotation.body} target={bodyTarget} {disabled} maxLength={4000}
      label={tr('Your comment')} voiceLabel="Speak comment" focusOnMount={!annotation.body}
      placeholder={tr('Explain what to change and why…')} data-review-field="body"
      onChange={(body) => update({ body })} />
  </div>
  {#if annotation.kind === 'suggestion'}
    <div class="mt-3 grid gap-1.5 text-xs">
      <WorkbenchTextField value={annotation.replacement ?? ''} target={replacementTarget} {disabled} maxLength={8000}
        label={tr('Suggested wording')} voiceLabel="Speak suggested wording" tone="suggestion"
        data-review-field="replacement" onChange={(replacement) => update({ replacement })} />
      <span class="text-muted-foreground">{tr('An empty suggestion means delete this text.')}</span>
    </div>
  {/if}
  {#if !disabled && !annotation.body.trim()}<p class="mb-0 mt-3 text-xs text-amber-700 dark:text-amber-400">{tr('Finish or remove empty comments.')}</p>{/if}
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
