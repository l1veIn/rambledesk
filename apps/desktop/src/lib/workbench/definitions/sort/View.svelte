<script lang="ts">
  import { onMount } from 'svelte'
  import { flip } from 'svelte/animate'
  import { ArrowDown, ArrowUp, GripVertical, Maximize2 } from '@lucide/svelte'
  import { dragHandleZone, dragHandle, SHADOW_ITEM_MARKER_PROPERTY_NAME, SOURCES, TRIGGERS, type DndEvent } from 'svelte-dnd-action'
  import type { SortData } from '../../../generated/feedback'
  import type { WorkbenchViewContext } from '../contracts'
  import { validSortOrder } from './definition'

  export let context: WorkbenchViewContext
  type SortPreviewItem = { id: string; sourceId: string; label: string; [SHADOW_ITEM_MARKER_PROPERTY_NAME]?: boolean }
  let items: SortPreviewItem[] = []
  let drag: { signature: string; pendingSignature?: string } | null = null
  let announcement = ''
  const duration = 140
  $: data = context.workspace.workbench!.data as SortData
  $: order = context.state?.type === 'sort' && validSortOrder(data, context.state.order)
    ? context.state.order : data.items.map((item) => item.id)
  $: editable = !context.disabled && !context.readOnly
  $: signature = orderSignature(order)
  // An external draft, request or material change invalidates an unfinished drag.
  $: if (drag && (!editable || (signature !== drag.signature && signature !== drag.pendingSignature))) drag = null
  $: if (drag?.pendingSignature === signature) drag = { signature }
  $: if (!drag) items = orderedItems(order)

  onMount(() => {
    if (!context.state && editable) context.host.updateState({ type: 'sort', order: [...order] })
  })

  function orderSignature(value: readonly string[]) { return JSON.stringify([context.host.requestId, data, value]) }
  function orderedItems(value: readonly string[]): SortPreviewItem[] {
    const byId = new Map(data.items.map((item) => [item.id, item]))
    // Keep business IDs out of the library's reserved placeholder namespace.
    return value.map((id) => byId.get(id)!).filter(Boolean).map((item) => ({ id: `sort:${item.id}`, sourceId: item.id, label: item.label }))
  }
  function currentDrag() { return editable && drag && (signature === drag.signature || signature === drag.pendingSignature) }
  function consider(event: CustomEvent<DndEvent<SortPreviewItem>>) {
    if (!editable) return
    if (event.detail.info.trigger === TRIGGERS.DRAG_STARTED) drag = { signature }
    if (!currentDrag()) return
    items = event.detail.items
    if (event.detail.info.trigger === TRIGGERS.DRAG_STOPPED) drag = null
  }
  function finalize(event: CustomEvent<DndEvent<SortPreviewItem>>) {
    if (!currentDrag()) { drag = null; return }
    const next = event.detail.items.map((item) => item.sourceId)
    if (!validSortOrder(data, next)) { drag = null; return }
    items = orderedItems(next)
    drag = { signature, pendingSignature: orderSignature(next) }
    if (next.some((id, index) => id !== order[index])) context.host.updateState({ type: 'sort', order: next })
    const moved = data.items.find((item) => `sort:${item.id}` === event.detail.info.id)
    if (moved) announcement = `${moved.label}，第 ${next.indexOf(moved.id) + 1} 位。`
    if (event.detail.info.source === SOURCES.POINTER) drag = null
  }
  function move(id: string, direction: -1 | 1) {
    if (!editable || drag) return
    const index = order.indexOf(id), target = index + direction
    if (index < 0 || target < 0 || target >= order.length) return
    const next = [...order]
    ;[next[index], next[target]] = [next[target], next[index]]
    context.host.updateState({ type: 'sort', order: next })
    announcement = `${data.items.find((item) => item.id === id)!.label}，第 ${target + 1} 位。`
  }
</script>

<section class="mx-auto grid w-full max-w-2xl gap-4" data-sort-workbench>
  <div class="flex items-center gap-3">
    <h2 class="m-0 min-w-0 flex-1 break-words text-base font-semibold">{data.title}</h2>
    {#if context.host.openExpanded}
      <button type="button" class="flex shrink-0 items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs" onclick={context.host.openExpanded}>
        <Maximize2 class="size-3.5" aria-hidden="true" />全屏工作台
      </button>
    {/if}
  </div>
  {#if editable}<p class="m-0 text-xs leading-5 text-muted-foreground">拖动手柄调整顺序，也可用上下按钮移动。</p>{/if}
  <ol class="m-0 grid list-none gap-2 rounded-lg p-0" aria-label="排序列表" data-sort-list
    use:dragHandleZone={{ items, type: `rambledesk-sort-${context.host.requestId}`, flipDurationMs: duration,
      dragDisabled: !editable, dropFromOthersDisabled: true, morphDisabled: true, delayTouchStart: 120, useCursorForDetection: true,
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
        <span class="min-w-0 flex-1 break-words text-sm leading-6" data-sort-label>{item.label}</span>
        {#if editable}
          <div class="flex shrink-0 gap-1">
            <button type="button" class="grid size-8 place-items-center rounded-md border text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
              aria-label={`上移 ${item.label}`} disabled={index === 0 || !!drag} onclick={() => move(item.sourceId, -1)}><ArrowUp class="size-3.5" aria-hidden="true" /></button>
            <button type="button" class="grid size-8 place-items-center rounded-md border text-muted-foreground hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
              aria-label={`下移 ${item.label}`} disabled={index === items.length - 1 || !!drag} onclick={() => move(item.sourceId, 1)}><ArrowDown class="size-3.5" aria-hidden="true" /></button>
          </div>
        {/if}
      </li>
    {/each}
  </ol>
  <span class="sr-only" aria-live="polite">{announcement}</span>
</section>

<style>
  .sort-placeholder { border-color: var(--primary); background: color-mix(in oklab, var(--primary) 8%, var(--background)); }
</style>
