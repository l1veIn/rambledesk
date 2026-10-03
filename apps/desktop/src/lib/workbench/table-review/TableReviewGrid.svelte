<script lang="ts">
  import { tick } from 'svelte'
  import type { TableReviewData } from '../../generated/feedback'
  import type { TableReviewState } from '../definitions/table_review/state'
  import { cellIdentity, cellPosition, columnLetter, sameCell, type TableCell } from './reviewModel'

  export let data: TableReviewData
  export let state: TableReviewState
  export let selected: TableCell
  export let editable: boolean
  export let editing: { cell: TableCell; value: string } | null
  export let onSelect: (cell: TableCell) => void
  export let onEdit: (cell: TableCell) => void
  export let onEditText: (value: string) => void
  export let onSave: () => void
  export let onCancel: () => void
  let scroller: HTMLDivElement
  let scrollTop = 0, height = 480, sourceKey = ''
  const rowHeight = 40
  $: start = Math.max(0, Math.floor(scrollTop / rowHeight) - 5)
  $: end = Math.min(data.rows.length, start + Math.ceil(height / rowHeight) + 12)
  $: rows = data.rows.slice(start, end)
  $: changes = new Map(state.changes.map((change) => [cellIdentity(change), change.value]))
  $: comments = new Set(state.comments.map(cellIdentity))
  $: resetSource(JSON.stringify(data))
  function resetSource(key: string) {
    if (key === sourceKey) return
    sourceKey = key; scrollTop = 0
    if (scroller) scroller.scrollTop = 0
  }
  export async function focusCell(cell: TableCell) {
    const position = cellPosition(data, cell)
    if (!position || !scroller) return
    const top = position.row * rowHeight
    if (top < scrollTop || top + rowHeight > scrollTop + height - 56) {
      scroller.scrollTop = Math.max(0, top - rowHeight * 2)
      scrollTop = scroller.scrollTop
    }
    await tick()
    const node = [...scroller.querySelectorAll<HTMLElement>('[data-table-cell]')].find((item) => item.dataset.tableCell === cellIdentity(cell))
    node?.focus()
    node?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }
  function cellKey(event: KeyboardEvent, cell: TableCell) {
    const position = cellPosition(data, cell)
    if (!position || event.ctrlKey || event.metaKey || event.altKey) return
    const movement: Record<string, [number, number]> = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] }
    if (movement[event.key]) {
      event.preventDefault()
      const [rowOffset, columnOffset] = movement[event.key]
      const row = Math.max(0, Math.min(data.rows.length - 1, position.row + rowOffset))
      const column = Math.max(0, Math.min(data.columns.length - 1, position.column + columnOffset))
      const next = { row_id: data.rows[row].id, column_id: data.columns[column].id }
      onSelect(next); void focusCell(next)
    } else if (editable && (event.key === 'Enter' || event.key === 'F2')) { event.preventDefault(); onEdit(cell) }
  }
  function editKey(event: KeyboardEvent) {
    event.stopPropagation()
    if (event.isComposing) return
    if (event.key === 'Escape') { event.preventDefault(); onCancel() }
    else if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); onSave() }
  }
</script>

<div bind:this={scroller} bind:clientHeight={height} class="table-scroll" data-tour="table-grid"
  on:scroll={() => { scrollTop = scroller.scrollTop }}>
  <table role="grid" aria-label={data.title} aria-rowcount={data.rows.length + 1} aria-colcount={data.columns.length + 1}>
    <thead><tr>
      <th class="corner" aria-label="Row number"></th>
      {#each data.columns as column, index (column.id)}
        <th scope="col" aria-colindex={index + 2} title={column.label}>
          <span class="column-letter">{columnLetter(index)}</span><span class="column-label">{column.label}</span>
        </th>
      {/each}
    </tr></thead>
    <tbody>
      {#if start > 0}<tr aria-hidden="true"><td colspan={data.columns.length + 1} class="spacer" style:height={`${start * rowHeight}px`}></td></tr>{/if}
      {#each rows as row, offset (row.id)}
        <tr aria-rowindex={start + offset + 2}>
          <th scope="row" class:selected-row={selected.row_id === row.id}>{start + offset + 1}</th>
          {#each data.columns as column, index (column.id)}
            {@const cell = { row_id: row.id, column_id: column.id }}
            {@const key = cellIdentity(cell)}
            {@const chosen = sameCell(selected, cell)}
            {@const value = changes.get(key) ?? row.cells[index]}
            <td role="gridcell" aria-colindex={index + 2} aria-selected={chosen} tabindex={chosen ? 0 : -1}
              data-table-cell={key} class:selected={chosen} class:suggested={changes.has(key)}
              title={value} on:click={() => onSelect(cell)} on:dblclick={() => { if (editable) onEdit(cell) }} on:keydown={(event) => cellKey(event, cell)}>
              {#if editing && sameCell(editing.cell, cell)}
                <textarea data-table-cell-editor aria-label={column.label} value={editing.value} disabled={!editable}
                  on:input={(event) => onEditText(event.currentTarget.value)} on:keydown={editKey}
                  on:click={(event) => event.stopPropagation()} on:dblclick={(event) => event.stopPropagation()}></textarea>
              {:else}
                <span class="cell-text">{value || '\u00a0'}</span>
                {#if comments.has(key)}<span class="comment-corner" aria-label="Has comment"></span>{/if}
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
      {#if end < data.rows.length}<tr aria-hidden="true"><td colspan={data.columns.length + 1} class="spacer" style:height={`${(data.rows.length - end) * rowHeight}px`}></td></tr>{/if}
    </tbody>
  </table>
</div>

<style>
  .table-scroll { overflow: auto; height: clamp(300px, 54vh, 760px); width: 100%; background: var(--background); }
  table { border-collapse: separate; border-spacing: 0; table-layout: fixed; font-size: 13px; width: max-content; min-width: 100%; }
  th, td { border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); }
  thead th { position: sticky; top: 0; height: 56px; min-width: 170px; max-width: 240px; z-index: 2; background: var(--muted); padding: 5px 12px; text-align: left; }
  .column-letter { display: block; color: var(--muted-foreground); font-size: 11px; font-weight: 400; }
  .column-label { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  tbody th, .corner { position: sticky; left: 0; width: 44px; min-width: 44px; max-width: 44px; background: var(--muted); text-align: center; color: var(--muted-foreground); font-weight: 400; z-index: 1; }
  thead .corner { z-index: 3; }
  tbody th.selected-row { background: color-mix(in srgb, var(--primary) 14%, var(--background)); color: var(--primary); }
  td { position: relative; height: 40px; min-width: 170px; max-width: 240px; padding: 0 12px; outline: none; cursor: cell; }
  td.selected { box-shadow: inset 0 0 0 2px var(--primary); background: color-mix(in srgb, var(--primary) 5%, var(--background)); }
  td.suggested .cell-text { color: var(--primary); font-weight: 500; }
  .cell-text { display: block; max-width: 216px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .comment-corner { position: absolute; top: 0; right: 0; border-top: 7px solid #f59e0b; border-left: 7px solid transparent; }
  textarea { position: absolute; inset: 1px; width: calc(100% - 2px); height: calc(100% - 2px); background: var(--background); color: var(--foreground); padding: 7px 10px; resize: none; outline: 2px solid var(--primary); z-index: 4; }
  td.spacer { padding: 0; border: 0; }
</style>
