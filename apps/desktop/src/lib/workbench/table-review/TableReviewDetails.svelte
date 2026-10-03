<script lang="ts">
  import { Trash2, X } from '@lucide/svelte'
  import type { InputTarget } from '../../domain/inputTarget'
  import type { TableReviewComment, TableReviewData } from '../../generated/feedback'
  import type { TableReviewState } from '../definitions/table_review/state'
  import WorkbenchTextField from '../../input/WorkbenchTextField.svelte'
  import { fieldAttachmentText } from '../../input/fieldAttachmentText'
  import { unavailableInputToolsState, useInputTools } from '../../input/inputToolsContext'
  import SpeechOriginBadge from '../../speech/SpeechOriginBadge.svelte'
  import { useVoiceInput } from '../../speech/voiceInputContext'
  import { text } from '../definitions/validation'
  import { cellLabel, originalCellValue, sameCell, type TableCell } from './reviewModel'

  export let data: TableReviewData
  export let state: TableReviewState
  export let selected: TableCell
  export let selectedComment: TableReviewComment | undefined
  export let editable: boolean
  export let tr: (source: string) => string
  export let targetFor: (field: 'change_value' | 'comment_body', cell: TableCell, id?: string) => InputTarget | null
  export let onSelect: (cell: TableCell, commentId?: string) => void
  export let onChangeValue: (value: string) => boolean
  export let onChangeComment: (id: string, body: string) => void
  export let onRemoveChange: () => void
  export let onRemoveComment: (id: string) => void
  const toolsState = useInputTools()?.state ?? unavailableInputToolsState
  const voice = useVoiceInput()
  const suggestionInputId = `table-suggestion-${crypto.randomUUID()}`
  $: change = state.changes.find((item) => sameCell(item, selected))
  $: original = originalCellValue(data, selected) ?? ''
  const preview = (body: string) => fieldAttachmentText(body, $toolsState.attachments).text || tr('Draft comment')
  function editSuggestion(event: Event) {
    const input = event.currentTarget as HTMLTextAreaElement
    const value = [...input.value].slice(0, 4000).join('')
    if (text(value, 4000, false) && onChangeValue(value)) input.value = value
    else input.value = change?.value ?? ''
  }
  function selectSuggestionTarget() {
    const target = change && targetFor('change_value', change)
    if (editable && target) voice?.selectTarget(target)
  }
</script>

<aside class="min-w-0 border-t bg-muted/10 p-3 @3xl:border-l @3xl:border-t-0" aria-label={tr('Comments')}>
  <div class="grid gap-3">
    <div class="grid gap-1 text-xs">
      <span class="text-muted-foreground">{tr('Original value')} · {cellLabel(data, selected)}</span>
      <div data-table-original class="max-h-32 overflow-auto whitespace-pre-wrap break-words rounded border bg-background px-3 py-2 text-sm">{original || tr('Empty')}</div>
    </div>
    {#if change}
      <div data-table-suggestion-editor class="grid gap-2">
        <div class="flex items-center gap-2 text-xs"><label for={suggestionInputId}>{tr('Suggested value')}</label>
          <SpeechOriginBadge target={targetFor('change_value', change)} /></div>
        <textarea id={suggestionInputId} data-table-suggestion-value aria-label={tr('Suggested value')} value={change.value} disabled={!editable}
          class="min-h-24 w-full resize-y rounded-lg border border-primary/25 bg-primary/5 px-3 py-2 text-sm leading-6 outline-none focus:border-ring disabled:opacity-70"
          on:input={editSuggestion} on:focus={selectSuggestionTarget}></textarea>
        {#if editable}<button class="inline-flex items-center gap-1 justify-self-end text-xs text-muted-foreground hover:text-foreground"
          aria-label={tr('Remove suggestion')} on:click={onRemoveChange}><X size={13} />{tr('Remove suggestion')}</button>{/if}
      </div>
    {:else}<p class="text-xs text-muted-foreground">{tr('No suggestion for this cell.')}</p>{/if}

    {#if state.changes.length > 0}
      <div class="grid gap-2 border-t pt-3">
        <h3 class="text-xs font-semibold">{tr('Suggestions')} <span class="text-muted-foreground">{state.changes.length}</span></h3>
        <div class="flex max-h-28 flex-wrap gap-1 overflow-auto">
          {#each state.changes as item (`${item.row_id}:${item.column_id}`)}
            <button data-table-suggestion={`${item.row_id}:${item.column_id}`} class="rounded border px-2 py-1 text-xs hover:bg-muted"
              class:bg-muted={sameCell(item, selected)} on:click={() => onSelect(item)}>{cellLabel(data, item).split(' · ')[0]}</button>
          {/each}
        </div>
      </div>
    {/if}
    <section data-tour="table-comments" class="grid gap-2 border-t pt-3">
      <h3 class="text-xs font-semibold">{tr('Comments')} <span class="text-muted-foreground">{state.comments.length}</span></h3>
      {#if state.comments.length === 0}<p class="py-3 text-xs text-muted-foreground">{tr('No comments yet. Select a cell to leave feedback.')}</p>
      {:else}
        <div class="grid max-h-48 gap-1 overflow-auto">
          {#each state.comments as comment (comment.id)}
            <button data-table-comment={comment.id} class="rounded border px-2 py-2 text-left hover:bg-muted"
              class:bg-muted={selectedComment?.id === comment.id} on:click={() => onSelect(comment, comment.id)}>
              <span class="block truncate text-xs font-medium">{cellLabel(data, comment)}</span>
              <span class="block truncate text-xs text-muted-foreground">{preview(comment.body)}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#if selectedComment}
        <div data-table-comment-editor class="grid gap-2">
          {#key selectedComment.id}
            <WorkbenchTextField value={selectedComment.body} label={tr('Comment')} voiceLabel={tr('Record comment')}
              target={targetFor('comment_body', selectedComment, selectedComment.id)} maxLength={4000} disabled={!editable}
              placeholder={tr('Write your feedback…')} onChange={(body) => onChangeComment(selectedComment!.id, body)} />
          {/key}
          {#if editable}<button class="inline-flex items-center gap-1 justify-self-end text-xs text-muted-foreground hover:text-destructive"
            aria-label={tr('Delete comment')} on:click={() => onRemoveComment(selectedComment!.id)}><Trash2 size={13} />{tr('Delete comment')}</button>{/if}
        </div>
      {/if}
    </section>
  </div>
</aside>
