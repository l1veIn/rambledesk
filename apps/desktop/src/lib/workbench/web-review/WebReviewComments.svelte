<script lang="ts">
  import { MessageSquare, X } from '@lucide/svelte'
  import type { WebReviewAnnotation } from '../../generated/feedback'
  import { locale } from '../../preferences'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from '../../input/inputToolsContext'
  import { elementLabel } from './reviewModel'
  import { webReviewText } from './reviewI18n'

  export let annotations: WebReviewAnnotation[]
  export let selectedId: string | null
  export let onSelect: (annotation: WebReviewAnnotation) => void
  export let onClose: () => void
  const toolsState = useInputTools()?.state ?? unavailableInputToolsState
  const tr = (source: string) => webReviewText($locale, source)
  function preview(note: WebReviewAnnotation) {
    const projection = fieldAttachmentText(note.body, $toolsState.attachments)
    return projection.text.trim() || tr(projection.attachmentIds.length ? 'Attachments' : 'Write your comment…')
  }
</script>

<aside aria-label={tr('Review comments')} class="flex h-full min-h-0 flex-col rounded-xl border bg-background/95 shadow-lg">
  <header class="flex items-center gap-2 border-b p-3">
    <MessageSquare class="size-4 text-primary" /><strong class="flex-1 text-sm font-medium">{tr('Review comments')} · {annotations.length}</strong>
    <button type="button" aria-label={tr('Close review comments')} onclick={onClose} class="rounded p-1 hover:bg-muted"><X class="size-4" /></button>
  </header>
  <div class="min-h-0 flex-1 overflow-auto p-2">
    {#if !annotations.length}<p class="m-0 p-2 text-xs leading-6 text-muted-foreground">{tr('No element comments yet.')} {tr('Choose Select elements, then click an element to comment.')}</p>{/if}
    <div class="grid gap-2">
      {#each annotations as note, index (note.id)}
        <button type="button" data-web-review-note-preview={note.id} aria-pressed={selectedId === note.id} onclick={() => onSelect(note)}
          class="flex min-w-0 gap-2 rounded-lg border bg-background p-3 text-left hover:border-primary/50 aria-pressed:border-primary aria-pressed:bg-primary/5">
          <span class="grid size-6 shrink-0 place-items-center rounded-full bg-primary/10 text-xs text-primary">{index + 1}</span>
          <span class="grid min-w-0 flex-1 gap-1"><strong class="truncate text-xs font-medium" title={note.element.selector}>{elementLabel(note)}</strong><span class="line-clamp-3 whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">{preview(note)}</span><span class="truncate text-[10px] text-muted-foreground">{note.viewport.width} × {note.viewport.height} · {note.page_url}</span></span>
        </button>
      {/each}
    </div>
  </div>
  <p class="m-0 border-t p-3 text-[11px] leading-5 text-muted-foreground">{tr('Click a comment to return to its element.')}</p>
</aside>
