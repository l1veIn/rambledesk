<script lang="ts">
  import { tick } from 'svelte'
  import { Maximize2, MessageSquare, Mic, Pencil, Table2 } from '@lucide/svelte'
  import type { TableReviewData } from '../../generated/feedback'
  import type { InputTarget } from '../../domain/inputTarget'
  import type { TableReviewState } from '../definitions/table_review/state'
  import { locale } from '../../preferences'
  import { unavailableVoiceInputState, useVoiceInput, workbenchFieldVoiceTarget } from '../../speech/voiceInputContext'
  import { cellIdentity, cellLabel, cellPosition, emptyTableReviewState, originalCellValue, prepareTableComment,
    sameCell, suggestCell, validateTableReviewState, type TableCell } from './reviewModel'
  import TableReviewGrid from './TableReviewGrid.svelte'
  import TableReviewDetails from './TableReviewDetails.svelte'
  import { tableReviewText } from './tableReviewI18n'

  export let data: TableReviewData
  export let state: TableReviewState | null = null
  export let requestId: string
  export let disabled = false
  export let readOnly = false
  export let onChange: (state: TableReviewState) => void
  export let onOpenExpanded: (() => void) | undefined = undefined
  const voice = useVoiceInput()
  const voiceState = voice?.state ?? unavailableVoiceInputState
  let root: HTMLElement, grid: TableReviewGrid
  let selected: TableCell = { row_id: '', column_id: '' }, selectedId: string | undefined
  let editing: { cell: TableCell; value: string } | null = null
  let sourceKey = '', revealedSequence: number | undefined
  $: current = state ?? emptyTableReviewState()
  $: editable = !disabled && !readOnly
  $: if (!editable) editing = null
  $: resetSource(`${requestId}:${JSON.stringify(data)}`)
  $: selectedComment = current.comments.find((comment) => comment.id === selectedId)
  $: issue = validateTableReviewState(data, current)
  $: void revealField($voiceState.revealSequence, $voiceState.revealTarget, current)
  const tr = (source: string) => tableReviewText($locale, source)
  function resetSource(key: string) {
    if (key === sourceKey) return
    sourceKey = key; selected = { row_id: data.rows[0].id, column_id: data.columns[0].id }
    selectedId = undefined; editing = null
  }
  function update(next: TableReviewState) { if (editable) onChange(next) }
  function targetFor(field: 'change_value' | 'comment_body', cell: TableCell, id?: string) {
    return workbenchFieldVoiceTarget($voiceState, { workbenchType: 'table_review', version: 1, field,
      entityId: id ?? cellIdentity(cell), sourceVersion: data.source_version, label: cellLabel(data, cell) })
  }
  function select(cell: TableCell, commentId?: string) {
    if (!cellPosition(data, cell)) return
    selected = { row_id: cell.row_id, column_id: cell.column_id }; editing = null
    selectedId = commentId ?? current.comments.find((comment) => sameCell(comment, cell))?.id
  }
  function revealCell(cell: TableCell, commentId?: string) { select(cell, commentId); void grid?.focusCell(cell) }
  async function startEdit(cell = selected) {
    if (!editable) return
    select(cell)
    editing = { cell: { ...cell }, value: current.changes.find((item) => sameCell(item, cell))?.value ?? originalCellValue(data, cell) ?? '' }
    await tick()
    const input = root?.querySelector<HTMLTextAreaElement>('[data-table-cell-editor]')
    input?.focus(); input?.select()
  }
  function saveEdit() {
    if (!editable || !editing) return
    const next = suggestCell(data, current, editing.cell, editing.value)
    if (!next) return
    update(next); editing = null; void grid?.focusCell(selected)
  }
  function cancelEdit() { editing = null; void grid?.focusCell(selected) }
  async function openExpanded() {
    if (editing) { saveEdit(); if (editing) return; await tick() }
    onOpenExpanded?.()
  }
  async function addComment(speak = false) {
    if (!editable) return
    const prepared = prepareTableComment(data, current, selected, () => `comment-${crypto.randomUUID()}`)
    if (!prepared) return
    update(prepared.state); selectedId = prepared.comment.id
    await tick()
    root?.querySelector<HTMLElement>('[data-table-comment-editor] [contenteditable]')?.focus()
    const target = targetFor('comment_body', prepared.comment, prepared.comment.id)
    if (speak && target) await voice?.start(target)
  }
  async function speakSuggestion() {
    if (!editable) return
    const value = current.changes.find((change) => sameCell(change, selected))?.value ?? ''
    const next = suggestCell(data, current, selected, value, true)
    if (!next) return
    update(next); editing = null
    await tick()
    const target = targetFor('change_value', selected)
    if (target) await voice?.start(target)
  }
  function changeValue(value: string) {
    const next = suggestCell(data, current, selected, value, true)
    if (!editable || !next) return false
    update(next)
    return true
  }
  async function revealField(sequence: number | undefined, target: InputTarget | null | undefined, draft: TableReviewState) {
    if (sequence === undefined || sequence === revealedSequence) return
    revealedSequence = sequence
    const field = target?.destination
    if (target?.requestId !== requestId || field?.kind !== 'workbench_field' || field.workbenchType !== 'table_review'
      || field.version !== 1 || field.sourceVersion !== data.source_version) return
    const entry = field.field === 'comment_body' ? draft.comments.find((comment) => comment.id === field.entityId)
      : field.field === 'change_value' ? draft.changes.find((change) => cellIdentity(change) === field.entityId) : undefined
    if (entry) revealCell(entry, 'id' in entry ? entry.id : undefined)
  }
</script>

<div bind:this={root} class="@container overflow-hidden rounded-lg border bg-background">
  <div data-tour="table-review-toolbar" class="flex flex-wrap items-center gap-2 border-b px-3 py-2">
    <Table2 size={16} class="text-muted-foreground" /><h2 class="min-w-0 flex-1 truncate text-sm font-semibold" title={data.title}>{data.title}</h2>
    <span class="text-xs text-muted-foreground">{data.rows.length} {tr('Rows')} · {data.columns.length} {tr('Columns')}</span>
    {#if onOpenExpanded}<button class="inline-flex items-center gap-1 rounded border px-2 py-1 text-xs hover:bg-muted" on:click={openExpanded}>
      <Maximize2 size={13} />{tr('Full screen review')}</button>{/if}
  </div>
  <div data-tour="table-selection" class="flex flex-wrap items-center gap-2 border-b bg-muted/20 px-3 py-2">
    <span data-table-selected class="mr-auto text-xs font-medium">{cellLabel(data, selected)}</span>
    {#if editable}
      {#if editing}
        <button class="rounded bg-primary px-2 py-1 text-xs text-primary-foreground" on:click={saveEdit}>{tr('Save suggestion')}</button>
        <button class="rounded border px-2 py-1 text-xs" on:click={cancelEdit}>{tr('Cancel editing')}</button>
      {:else}
        <button data-table-edit class="inline-flex items-center gap-1 rounded border px-2 py-1 text-xs hover:bg-muted" on:click={() => startEdit()}><Pencil size={13} />{tr('Suggest value')}</button>
      {/if}
      <button data-table-add-comment class="inline-flex items-center gap-1 rounded border px-2 py-1 text-xs hover:bg-muted" on:click={() => addComment()}><MessageSquare size={13} />{tr('Add comment')}</button>
      {#if voice && !$voiceState.disabled}
        <button data-table-voice-comment class="inline-flex items-center gap-1 rounded border px-2 py-1 text-xs hover:bg-muted" disabled={$voiceState.recording} on:click={() => addComment(true)}><Mic size={13} />{tr('Voice comment')}</button>
        <button data-table-voice-suggestion class="rounded border px-2 py-1 text-xs hover:bg-muted" disabled={$voiceState.recording} on:click={speakSuggestion}>{tr('Voice suggestion')}</button>
      {/if}
    {/if}
  </div>
  <div class="grid min-w-0 @3xl:grid-cols-[minmax(0,1fr)_20rem]">
    <div class="min-w-0">
      <TableReviewGrid bind:this={grid} {data} state={current} {selected} {editable} {editing} onSelect={select} onEdit={startEdit}
        onEditText={(value) => { if (editing) editing = { ...editing, value } }} onSave={saveEdit} onCancel={cancelEdit} />
      <div class="flex flex-wrap gap-3 border-t px-3 py-2 text-xs text-muted-foreground">
        <span>{tr('Suggestions')} {current.changes.length}</span><span>{tr('Comments')} {current.comments.length}</span>
        {#if editable}<span class="ml-auto">{tr('Arrow keys move · Enter/F2 edit · Escape cancels')}</span>{/if}
      </div>
    </div>
    <TableReviewDetails {data} state={current} {selected} {selectedComment} {editable} {tr} {targetFor} onSelect={revealCell}
      onChangeValue={changeValue} onRemoveChange={() => update({ ...current, changes: current.changes.filter((change) => !sameCell(change, selected)) })}
      onChangeComment={(id, body) => update({ ...current, comments: current.comments.map((comment) => comment.id === id ? { ...comment, body } : comment) })}
      onRemoveComment={(id) => { update({ ...current, comments: current.comments.filter((comment) => comment.id !== id) }); selectedId = undefined }} />
  </div>
  {#if issue}<p role="status" class="border-t px-3 py-2 text-xs text-amber-700 dark:text-amber-400">{tr(issue)}</p>{/if}
</div>
