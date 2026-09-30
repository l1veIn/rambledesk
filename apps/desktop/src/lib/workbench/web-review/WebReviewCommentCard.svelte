<script lang="ts">
  import { ChevronUp, MessageSquare, Trash2 } from '@lucide/svelte'
  import { Button } from '$lib/components/ui/button'
  import * as Dialog from '$lib/components/ui/dialog'
  import type { WebReviewAnnotation } from '../../generated/feedback'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import FieldAttachments from '../../input/FieldAttachments.svelte'
  import { unavailableVoiceInputState, useVoiceInput, webReviewAnnotationVoiceTarget } from '../../speech/voiceInputContext'
  import { locale } from '../../preferences'
  import { elementLabel } from './reviewModel'
  import { webReviewText } from './reviewI18n'

  export let annotation: WebReviewAnnotation
  export let number: number
  export let disabled = false
  export let collapsible = true
  export let floating = false
  export let onUpdate: (annotation: WebReviewAnnotation) => void
  export let onDelete: () => void
  export let onCollapse: () => void
  let bodyField: WorkbenchTextField | undefined
  let deleteConfirmOpen = false
  let deleteButton: HTMLButtonElement
  let cancelButton: HTMLElement | null = null
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const tr = (source: string) => webReviewText($locale, source)
  $: target = webReviewAnnotationVoiceTarget($voiceState, annotation.id, elementLabel(annotation))
  $: if (disabled) deleteConfirmOpen = false

  export function focusComment() { if (!disabled) bodyField?.focusAtEnd() }
  function requestDelete() {
    if (disabled) return
    if (annotation.body.trim()) deleteConfirmOpen = true
    else onDelete()
  }
  function confirmDelete() {
    if (!disabled && deleteConfirmOpen) { deleteConfirmOpen = false; onDelete() }
  }
</script>

<article class="rounded-xl border bg-background p-3 shadow-lg" class:web-review-floating-comment={floating} data-web-comment-id={annotation.id}>
  <div class="mb-3 flex shrink-0 items-center gap-2 text-xs">
    <span class="grid size-6 shrink-0 place-items-center rounded-full bg-primary text-primary-foreground">{number}</span>
    <MessageSquare class="size-3.5 shrink-0 text-primary" />
    <strong class="min-w-0 flex-1 truncate font-medium" title={annotation.element.selector}>{elementLabel(annotation)}</strong>
    {#if !disabled}<button bind:this={deleteButton} type="button" aria-label={tr('Delete comment')} onclick={requestDelete} class="rounded p-1.5 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"><Trash2 class="size-3.5" /></button>{/if}
    {#if collapsible}<button type="button" aria-label={tr('Collapse comment')} onclick={onCollapse} class="rounded p-1.5 text-muted-foreground hover:bg-muted"><ChevronUp class="size-3.5" /></button>{/if}
  </div>
  <div class="web-review-comment-content" class:scrollable={floating} data-web-review-comment-content>
  {#if annotation.element.text}<blockquote class="m-0 mb-3 max-h-24 overflow-auto border-l-2 border-primary/30 pl-2 text-xs leading-5 text-muted-foreground">{annotation.element.text}</blockquote>{/if}
  {#if annotation.screenshot_attachment_id}<FieldAttachments value={`[Screenshot](attachment://${annotation.screenshot_attachment_id})`} disabled={true} onChange={() => {}} />{/if}
  <div class="mb-3 grid gap-1 text-[11px] text-muted-foreground" aria-label={tr('Saved page context')}>
    <span class="break-all">{annotation.page_url}</span>
    <span>{annotation.viewport.width} × {annotation.viewport.height} · &lt;{annotation.element.tag_name}&gt;</span>
    <span class="truncate font-mono" title={annotation.element.selector}>{annotation.element.selector}</span>
  </div>
  <WorkbenchTextField bind:this={bodyField} value={annotation.body} {target} {disabled} maxLength={4000}
    label={tr('Your comment')} voiceLabel="Speak comment" placeholder={tr('Explain what to change and why…')}
    data-web-review-field="body" onChange={(body) => { if (!disabled) onUpdate({ ...annotation, body }) }} />
  {#if !disabled && !annotation.body.trim()}<p class="mb-0 mt-2 text-xs text-amber-700 dark:text-amber-400">{tr('Finish or remove empty comments.')}</p>{/if}
  </div>
</article>

<Dialog.Root bind:open={deleteConfirmOpen}>
  <Dialog.Content data-web-review-delete-dialog class="max-w-sm gap-5" showCloseButton={false}
    onOpenAutoFocus={(event) => { event.preventDefault(); cancelButton?.focus() }}
    onCloseAutoFocus={(event) => { event.preventDefault(); if (deleteButton?.isConnected && !disabled) deleteButton.focus({ preventScroll: true }) }}>
    <Dialog.Header><Dialog.Title>{tr('Delete this comment?')}</Dialog.Title><Dialog.Description class="mt-1 leading-5">{tr('This comment and its contents will be deleted. This cannot be undone.')}</Dialog.Description></Dialog.Header>
    <Dialog.Footer class="gap-2 sm:justify-end">
      <Button bind:ref={cancelButton} variant="outline" onclick={() => deleteConfirmOpen = false}>{tr('Cancel')}</Button>
      <Button variant="destructive" {disabled} onclick={confirmDelete}>{tr('Delete comment')}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  .web-review-floating-comment { display: flex; height: 100%; min-height: 0; flex-direction: column; overflow: hidden; }
  .web-review-comment-content.scrollable { flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; }
</style>
