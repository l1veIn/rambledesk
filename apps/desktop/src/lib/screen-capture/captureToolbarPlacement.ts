import {
  captureToolbarPosition,
  toolbarPopoverOpensDownward,
  type OverlayGeometry,
  type ToolbarPlacement,
} from './overlayGeometry'
import type { CaptureRectangle } from '../screenCapture'

type ToolbarDrag = {
  pointerId: number
  startX: number
  startY: number
  originX: number
  originY: number
}

export type CaptureToolbarPlacementContext = {
  /** Live placement inputs; read on every layout pass. */
  getPlacement: () => Readonly<{
    selection: CaptureRectangle | null
    toolbarWidth: number
    toolbarHeight: number
  }>
  getGeometry: () => OverlayGeometry
  getHost: () => HTMLElement | null
  /** Receives the CSS the toolbar should render with. */
  onChange: (style: string) => void
  onDragStart?: () => void
}

/**
 * Toolbar position, manual drag offset, and the window listeners that keep a
 * drag alive outside the toolbar. The component only forwards pointer events.
 */
export function createCaptureToolbarPlacement(context: CaptureToolbarPlacementContext) {
  let manualX: number | null = null
  let manualY: number | null = null
  let drag: ToolbarDrag | null = null
  let listening = false

  function position() {
    const placement = context.getPlacement()
    const input: ToolbarPlacement = {
      selection: placement.selection,
      toolbarWidth: placement.toolbarWidth,
      toolbarHeight: placement.toolbarHeight,
      toolbarManualX: manualX,
      toolbarManualY: manualY,
    }
    return captureToolbarPosition(input, context.getGeometry())
  }

  function style() {
    const next = position()
    return next ? `left:${next.left}px;top:${next.top}px` : ''
  }

  function apply() {
    const next = position()
    if (!next) return ''
    const css = `left:${next.left}px;top:${next.top}px`
    const host = context.getHost()
    if (host) {
      host.style.left = `${next.left}px`
      host.style.top = `${next.top}px`
    }
    context.onChange(css)
    return css
  }

  function popoverOpensDownward() {
    return toolbarPopoverOpensDownward(position())
  }

  function reset() {
    manualX = null
    manualY = null
    drag = null
    unbind()
    const host = context.getHost()
    if (host) {
      host.style.left = ''
      host.style.top = ''
    }
  }

  function bind() {
    if (listening) return
    listening = true
    window.addEventListener('pointermove', moveFromPointer, true)
    window.addEventListener('pointerup', endFromPointer, true)
    window.addEventListener('pointercancel', endFromPointer, true)
    window.addEventListener('mousemove', moveFromMouse, true)
    window.addEventListener('mouseup', endFromMouse, true)
  }

  function unbind() {
    if (!listening) return
    listening = false
    window.removeEventListener('pointermove', moveFromPointer, true)
    window.removeEventListener('pointerup', endFromPointer, true)
    window.removeEventListener('pointercancel', endFromPointer, true)
    window.removeEventListener('mousemove', moveFromMouse, true)
    window.removeEventListener('mouseup', endFromMouse, true)
  }

  function isDragSource(event: PointerEvent) {
    const target = event.target
    if (!(target instanceof Element)) return false
    if (target.closest('.toolbar-popover, textarea, input')) return false
    const button = target.closest('button')
    return !button || button.classList.contains('toolbar-drag')
  }

  function beginDrag(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary) return
    if (!isDragSource(event)) return
    event.preventDefault()
    event.stopPropagation()
    context.onDragStart?.()
    const next = position()
    if (!next) return
    drag = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: next.left,
      originY: next.top,
    }
    apply()
    bind()
  }

  function move(clientX: number, clientY: number) {
    if (!drag) return
    manualX = drag.originX + clientX - drag.startX
    manualY = drag.originY + clientY - drag.startY
    apply()
  }

  function moveFromPointer(event: PointerEvent) {
    if (!drag || (event.pointerId !== drag.pointerId && event.pointerId !== 0)) return
    if (event.buttons === 0) {
      endFromPointer(event)
      return
    }
    event.preventDefault()
    event.stopPropagation()
    move(event.clientX, event.clientY)
  }

  function moveFromMouse(event: MouseEvent) {
    if (!drag) return
    if (event.buttons === 0) {
      endFromMouse(event)
      return
    }
    event.preventDefault()
    event.stopPropagation()
    move(event.clientX, event.clientY)
  }

  function endFromPointer(event: PointerEvent) {
    if (!drag) return
    if (event.pointerId !== drag.pointerId && event.pointerId !== 0) return
    event.preventDefault()
    event.stopPropagation()
    drag = null
    unbind()
  }

  function endFromMouse(event: MouseEvent) {
    if (!drag) return
    event.preventDefault()
    event.stopPropagation()
    drag = null
    unbind()
  }

  return {
    style,
    apply,
    reset,
    popoverOpensDownward,
    beginDrag,
    dispose: unbind,
    isDragging: () => drag !== null,
  }
}

export type CaptureToolbarPlacement = ReturnType<typeof createCaptureToolbarPlacement>
