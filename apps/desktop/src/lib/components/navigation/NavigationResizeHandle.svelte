<script lang="ts">
  import { onDestroy } from 'svelte'
  import { locale } from '$lib/preferences'
  import { COLLAPSED_RAIL_WIDTH, resolveRailDrag } from './railResize'

  export let label: string
  export let controls: string
  export let expandedWidth: number
  export let displayWidth: number
  export let collapsed: boolean
  export let minWidth: number
  export let maxWidth: number
  export let onResize: (next: { width: number; collapsed: boolean }) => void
  export let onCommit: () => void = () => {}
  export let onDraggingChange: (active: boolean) => void = () => {}

  let handle: HTMLDivElement
  let drag: { pointerId: number; startX: number; initialWidth: number; initialExpandedWidth: number; collapsed: boolean; scale: number; moved: boolean } | null = null
  let previousCursor = ''
  let previousSelection = ''

  function scale() {
    const pane = handle.parentElement!
    // WebView zoom already uses logical coordinates; CSS zoom in Web Access
    // changes this ratio. Measure geometry instead of applying the zoom twice.
    return pane.offsetWidth ? pane.getBoundingClientRect().width / pane.offsetWidth : 1
  }

  function start(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary || drag) return
    event.preventDefault()
    event.stopPropagation()
    handle.focus({ preventScroll: true })
    drag = { pointerId: event.pointerId, startX: event.clientX, initialWidth: displayWidth, initialExpandedWidth: expandedWidth, collapsed, scale: scale(), moved: false }
    previousCursor = document.documentElement.style.cursor
    previousSelection = document.documentElement.style.userSelect
    document.documentElement.style.cursor = 'col-resize'
    document.documentElement.style.userSelect = 'none'
    try { handle.setPointerCapture(event.pointerId) } catch { /* Window listeners still finish the drag. */ }
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', end)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', escape)
    window.addEventListener('blur', cancel)
    window.addEventListener('resize', cancel)
    onDraggingChange(true)
  }

  function move(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointerId) return
    if (Math.abs(scale() - drag.scale) > 0.02) { finish(true); return }
    event.preventDefault()
    const delta = (event.clientX - drag.startX) / drag.scale
    if (!drag.moved && Math.abs(delta) < 3) return
    drag.moved = true
    onResize(resolveRailDrag({
      ...drag, delta, minWidth, maxWidth,
    }))
  }

  function end(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointerId) return
    move(event)
    finish(false)
  }

  function cancel(event: Event) {
    if (event instanceof PointerEvent && event.pointerId !== drag?.pointerId) return
    finish(true)
  }

  function escape(event: KeyboardEvent) {
    if (event.key !== 'Escape' || !drag) return
    event.preventDefault()
    event.stopPropagation()
    finish(true)
  }

  function finish(cancelled: boolean) {
    if (!drag) return
    const previous = drag
    drag = null
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', end)
    window.removeEventListener('pointercancel', cancel)
    window.removeEventListener('keydown', escape)
    window.removeEventListener('blur', cancel)
    window.removeEventListener('resize', cancel)
    document.documentElement.style.cursor = previousCursor
    document.documentElement.style.userSelect = previousSelection
    if (handle.hasPointerCapture(previous.pointerId)) handle.releasePointerCapture(previous.pointerId)
    if (cancelled) onResize({ width: previous.initialExpandedWidth, collapsed: previous.collapsed })
    else if (previous.moved) onCommit()
    onDraggingChange(false)
  }

  function keyDown(event: KeyboardEvent) {
    if (drag || event.altKey || event.ctrlKey || event.metaKey || event.isComposing) return
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End', 'Enter'].includes(event.key)) return
    event.preventDefault()
    event.stopPropagation()
    if (event.key === 'Home' || (event.key === 'Enter' && !collapsed)) {
      onResize({ width: expandedWidth, collapsed: true })
    } else if (event.key === 'End') {
      onResize({ width: maxWidth, collapsed: false })
    } else if (collapsed && (event.key === 'ArrowRight' || event.key === 'Enter')) {
      onResize({ width: expandedWidth, collapsed: false })
    } else if (!collapsed) {
      onResize(resolveRailDrag({ initialWidth: displayWidth, initialExpandedWidth: expandedWidth,
        delta: (event.key === 'ArrowLeft' ? -1 : 1) * (event.shiftKey ? 40 : 16), minWidth, maxWidth }))
    }
    onCommit()
  }

  onDestroy(() => finish(false))
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions (A focusable window splitter uses the separator role, range values and arrow-key controls.) -->
<div bind:this={handle} class="navigation-resize-handle" class:dragging={drag !== null}
  role="separator" tabindex="0" aria-orientation="vertical" aria-label={label} aria-controls={controls}
  aria-valuemin={COLLAPSED_RAIL_WIDTH} aria-valuemax={maxWidth} aria-valuenow={Math.round(displayWidth)}
  aria-valuetext={collapsed ? ($locale === 'zh-CN' ? '已收起' : 'Collapsed') : `${Math.round(displayWidth)} px`}
  title={$locale === 'zh-CN' ? '拖动调整宽度，拖到最小宽度收起；也可使用左右方向键。' : 'Drag to resize; drag to the minimum to collapse. Left and right arrow keys also resize.'}
  onpointerdown={start} onkeydown={keyDown}>
</div>

<style>
  .navigation-resize-handle {
    position: absolute;
    z-index: 25;
    top: 0;
    right: -3px;
    bottom: 0;
    width: 7px;
    touch-action: none;
    cursor: col-resize;
    outline: none;
  }
  .navigation-resize-handle::after {
    content: '';
    position: absolute;
    inset: 0 2px;
    background: transparent;
    transition: background-color 120ms ease;
  }
  .navigation-resize-handle:is(:hover, :focus-visible, .dragging)::after {
    background: var(--primary);
  }
</style>
