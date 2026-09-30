<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { flip } from 'svelte/animate'
  import { ArrowDown, ArrowUp, GripVertical, Maximize2, Pencil, RotateCcw, Trash2 } from '@lucide/svelte'
  import { dragHandleZone, dragHandle, SHADOW_ITEM_MARKER_PROPERTY_NAME, SOURCES, TRIGGERS, type DndEvent } from 'svelte-dnd-action'
  import type { SortData, SortLabelEdit, SortState } from '../../../generated/feedback'
  import type { WorkbenchViewContext } from '../contracts'
  import { sortLabel, sortState, validSortDraft, validSortOrder } from './definition'

  export let context: WorkbenchViewContext
  type SortPreviewItem = { id: string; sourceId: string; label: string; [SHADOW_ITEM_MARKER_PROPERTY_NAME]?: boolean }
  let items: SortPreviewItem[] = []
  let drag: { signature: string; pendingSignature?: string } | null = null
  let editingId: string | null = null
  let editingSource = ''
  let beforeEdit: SortLabelEdit | undefined
  let root: HTMLElement
  let announcement = ''
  const duration = 140
  $: data = context.workspace.workbench!.data as SortData
  $: normalized = sortState(context.state)
  $: state = normalized?.type === 'sort' && validSortDraft(data, normalized) ? normalized
    : { type: 'sort' as const, order: data.items.map((item) => item.id), removed_ids: [], edited_items: [] }
  $: order = state.order
  $: editable = !context.disabled && !context.readOnly
  $: sourceKey = JSON.stringify([context.host.requestId, data])
  $: signature = JSON.stringify([sourceKey, state])
  // An external draft, request or material change invalidates an unfinished drag.
  $: if (drag && (!editable || (signature !== drag.signature && signature !== drag.pendingSignature))) drag = null
  $: if (drag?.pendingSignature === signature) drag = { signature }
  $: if (!drag) items = orderedItems(order, state)
  $: if (editingId && (!editable || editingSource !== sourceKey || !order.includes(editingId))) editingId = null

  onMount(() => {
    if (!context.state && editable) context.host.updateState({ ...state, order: [...order] })
  })

  function stateSignature(value: SortState) { return JSON.stringify([sourceKey, { type: 'sort', ...value }]) }
  function orderedItems(value: readonly string[], current: SortState): SortPreviewItem[] {
    const byId = new Map(data.items.map((item) => [item.id, item]))
    // Keep business IDs out of the library's reserved placeholder namespace.
    return value.map((id) => byId.get(id)!).filter(Boolean).map((item) => ({ id: `sort:${item.id}`, sourceId: item.id, label: sortLabel(data, current, item.id) }))
  }
  function currentDrag() { return editable && !editingId && drag && (signature === drag.signature || signature === drag.pendingSignature) }
  function consider(event: CustomEvent<DndEvent<SortPreviewItem>>) {
    if (!editable || editingId) return
    if (event.detail.info.trigger === TRIGGERS.DRAG_STARTED) drag = { signature }
    if (!currentDrag()) return
    items = event.detail.items
    if (event.detail.info.trigger === TRIGGERS.DRAG_STOPPED) drag = null
  }
  function finalize(event: CustomEvent<DndEvent<SortPreviewItem>>) {
    if (!currentDrag()) { drag = null; return }
    const next = event.detail.items.map((item) => item.sourceId)
    if (!validSortOrder(data, next, state.removed_ids)) { drag = null; return }
    items = orderedItems(next, state)
    const nextState = { ...state, order: next }
    drag = { signature, pendingSignature: stateSignature(nextState) }
    if (next.some((id, index) => id !== order[index])) context.host.updateState(nextState)
    const moved = data.items.find((item) => `sort:${item.id}` === event.detail.info.id)
    if (moved) announcement = `${sortLabel(data, state, moved.id) || '空白选项'}，第 ${next.indexOf(moved.id) + 1} 位。`
    if (event.detail.info.source === SOURCES.POINTER) drag = null
  }
  function move(id: string, direction: -1 | 1) {
    if (!editable || drag) return
    const index = order.indexOf(id), target = index + direction
    if (index < 0 || target < 0 || target >= order.length) return
    const next = [...order]
    ;[next[index], next[target]] = [next[target], next[index]]
    context.host.updateState({ ...state, order: next })
    announcement = `${sortLabel(data, state, id) || '空白选项'}，第 ${target + 1} 位。`
  }
  async function beginEditing(id: string) {
    if (!editable || drag || !order.includes(id)) return
    if (editingId) finishEditing()
    beforeEdit = state.edited_items.find((item) => item.id === id)
    editingSource = sourceKey
    editingId = id
    await tick()
    if (editingId === id && editingSource === sourceKey) { const input = root.querySelector<HTMLInputElement>('[data-sort-editor]'); input?.focus(); input?.select() }
  }
  function changeLabel(id: string, input: HTMLInputElement) {
    if (!editable || editingId !== id || editingSource !== sourceKey || !order.includes(id)) return
    const value = input.value
    const label = [...value].filter((char) => char !== '\0' && !/^[\uD800-\uDFFF]$/.test(char)).slice(0, 200).join('')
    if (label !== value) input.value = label
    const original = data.items.find((item) => item.id === id)!.label
    const edits = state.edited_items.filter((item) => item.id !== id)
    if (label !== original) edits.push({ id, label })
    context.host.updateState({ ...state, edited_items: edits })
  }
  function finishEditing(cancel = false) {
    const id = editingId
    if (cancel && id && editable && editingSource === sourceKey && order.includes(id)) {
      const edits = state.edited_items.filter((item) => item.id !== id)
      if (beforeEdit) edits.push(beforeEdit)
      context.host.updateState({ ...state, edited_items: edits })
    }
    editingId = null
    beforeEdit = undefined
  }
  function editorKey(event: KeyboardEvent) {
    if (event.isComposing) return
    if (event.key === 'Enter' || event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); finishEditing(event.key === 'Escape') }
  }
  function remove(id: string) {
    if (!editable || drag || !order.includes(id)) return
    context.host.updateState({ ...state, order: order.filter((item) => item !== id), removed_ids: [...state.removed_ids, id] })
    announcement = `已删除${sortLabel(data, state, id) || '空白选项'}，可以恢复。`
  }
  function restore(id: string) {
    if (!editable || drag || !state.removed_ids.includes(id)) return
    context.host.updateState({ ...state, order: [...order, id], removed_ids: state.removed_ids.filter((item) => item !== id) })
    announcement = `已恢复${sortLabel(data, state, id) || '空白选项'}，位于列表末尾。`
  }
</script>

<section bind:this={root} class="mx-auto grid w-full max-w-2xl gap-4" data-sort-workbench>
  <div class="flex items-center gap-3">
    <h2 class="m-0 min-w-0 flex-1 break-words text-base font-semibold">{data.title}</h2>
    {#if context.host.openExpanded}
      <button type="button" class="flex shrink-0 items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs" onclick={context.host.openExpanded}>
        <Maximize2 class="size-3.5" aria-hidden="true" />全屏工作台
      </button>
    {/if}
  </div>
  {#if editable}<p class="m-0 text-xs leading-5 text-muted-foreground">拖动手柄调整顺序，也可编辑名称、删除选项。</p>{/if}
  {#if items.length === 0}<p class="m-0 rounded-lg border border-dashed p-4 text-sm text-muted-foreground">{editable ? '所有选项已删除。可以恢复选项，或直接提交。' : '所有选项已删除。'}</p>{/if}
  <ol class="m-0 grid list-none gap-2 rounded-lg p-0" aria-label="排序列表" data-sort-list
    use:dragHandleZone={{ items, type: `rambledesk-sort-${context.host.requestId}`, flipDurationMs: duration,
      dragDisabled: !editable || editingId !== null, dropFromOthersDisabled: true, morphDisabled: true, delayTouchStart: 120, useCursorForDetection: true,
      dropTargetStyle: { outline: '2px solid var(--primary)', outlineOffset: '4px' } }}
    onconsider={consider} onfinalize={finalize}>
    {#each items as item, index (item.id)}
      <li animate:flip={{ duration }} class="flex min-w-0 items-center gap-3 rounded-lg border bg-background p-3"
        class:sort-placeholder={!!item[SHADOW_ITEM_MARKER_PROPERTY_NAME]} aria-label={item.label} data-sort-item-id={item.sourceId}>
        {#if editable}
          <div use:dragHandle role="button" tabindex="0" class="grid size-8 shrink-0 touch-none place-items-center rounded-md text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
            data-sort-handle aria-label={`拖动 ${item.label}`} title="空格或回车开始，方向键移动，再按空格或回车完成。">
            <GripVertical class="size-4" aria-hidden="true" />
          </div>
        {/if}
        <span class="grid size-6 shrink-0 place-items-center rounded-md bg-muted text-xs tabular-nums text-muted-foreground" aria-hidden="true">{index + 1}</span>
        {#if editingId === item.sourceId && editable}
          <input class="min-w-0 flex-1 rounded-md border bg-background px-2 py-1 text-sm leading-6 outline-none focus:border-ring"
            data-sort-editor aria-label="选项名称" value={item.label} oninput={(event) => changeLabel(item.sourceId, event.currentTarget)} onblur={() => finishEditing()} onkeydown={editorKey} />
        {:else}
          <span class="min-w-0 flex-1 break-words text-sm leading-6" class:text-destructive={!item.label.trim()} data-sort-label>{item.label || '（空白选项）'}</span>
        {/if}
        {#if editable}
          <div class="flex shrink-0 gap-1">
            <button type="button" class="grid size-8 place-items-center rounded-md border text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
              aria-label={`上移 ${item.label}`} disabled={index === 0 || !!drag} onclick={() => move(item.sourceId, -1)}><ArrowUp class="size-3.5" aria-hidden="true" /></button>
            <button type="button" class="grid size-8 place-items-center rounded-md border text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
              aria-label={`下移 ${item.label}`} disabled={index === items.length - 1 || !!drag} onclick={() => move(item.sourceId, 1)}><ArrowDown class="size-3.5" aria-hidden="true" /></button>
            <button type="button" class="grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
              aria-label={`编辑 ${item.label || '空白选项'}`} disabled={!!drag} onclick={() => void beginEditing(item.sourceId)}><Pencil class="size-3.5" aria-hidden="true" /></button>
            <button type="button" class="grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-destructive/10 hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
              aria-label={`删除 ${item.label || '空白选项'}`} disabled={!!drag} onclick={() => remove(item.sourceId)}><Trash2 class="size-3.5" aria-hidden="true" /></button>
          </div>
        {/if}
      </li>
    {/each}
  </ol>
  {#if state.removed_ids.length > 0}
    <section class="grid gap-2" aria-label="已删除选项" data-sort-removed>
      <h3 class="m-0 text-xs font-medium text-muted-foreground">已删除（{state.removed_ids.length}）</h3>
      <ul class="m-0 grid list-none gap-2 p-0">{#each state.removed_ids as id (id)}
        {@const label = sortLabel(data, state, id)}
        <li class="flex min-w-0 items-center gap-3 rounded-lg border border-dashed px-3 py-2" data-sort-removed-id={id}>
          <span class="min-w-0 flex-1 break-words text-sm leading-6 text-muted-foreground line-through">{label || '（空白选项）'}</span>
          {#if editable}<button type="button" class="flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-xs hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
            aria-label={`恢复 ${label || '空白选项'}`} disabled={!!drag} onclick={() => restore(id)}><RotateCcw class="size-3.5" aria-hidden="true" />恢复</button>{/if}
        </li>
      {/each}</ul>
    </section>
  {/if}
  <span class="sr-only" aria-live="polite">{announcement}</span>
</section>

<style>
  .sort-placeholder { border-color: var(--primary); background: color-mix(in oklab, var(--primary) 8%, var(--background)); }
</style>
