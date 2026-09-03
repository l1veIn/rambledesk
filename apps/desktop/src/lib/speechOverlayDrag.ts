const POSITION_KEY = 'rambledesk.speech.overlay-position'
type Position = { left: number; bottom: number }

/** Keep the browser capsule's bottom edge fixed when its transcript expands. */
export function speechOverlayDrag(node: HTMLElement, enabled: boolean) {
  let position: Position | null = null
  let drag: { pointerId: number; x: number; y: number; left: number; bottom: number } | null = null
  try {
    const saved = JSON.parse(localStorage.getItem(POSITION_KEY) ?? 'null') as Position | null
    if (saved && Number.isFinite(saved.left) && Number.isFinite(saved.bottom)) position = saved
  } catch { /* Fall back to the default placement. */ }
  function layout() {
    if (!enabled || !position) return
    position.left = Math.max(8, Math.min(position.left, window.innerWidth - node.offsetWidth - 8))
    position.bottom = Math.max(8, Math.min(position.bottom, window.innerHeight - node.offsetHeight - 8))
    node.style.left = `${position.left}px`
    node.style.bottom = `${position.bottom}px`
    node.style.transform = 'none'
  }
  function down(event: PointerEvent) {
    if (!enabled || event.button !== 0 || !(event.target instanceof Element) || !event.target.closest('[data-speech-drag-handle]')) return
    event.preventDefault()
    const rect = node.getBoundingClientRect()
    drag = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, left: rect.left, bottom: window.innerHeight - rect.bottom }
    node.setPointerCapture(event.pointerId)
  }
  function move(event: PointerEvent) {
    if (!drag || drag.pointerId !== event.pointerId) return
    position = { left: drag.left + event.clientX - drag.x, bottom: drag.bottom - event.clientY + drag.y }
    layout()
  }
  function end(event: PointerEvent) {
    if (!drag || drag.pointerId !== event.pointerId) return
    drag = null
    if (node.hasPointerCapture(event.pointerId)) node.releasePointerCapture(event.pointerId)
    try { if (position) localStorage.setItem(POSITION_KEY, JSON.stringify(position)) } catch { /* Dragging still works without persistence. */ }
  }
  node.addEventListener('pointerdown', down)
  node.addEventListener('pointermove', move)
  node.addEventListener('pointerup', end)
  node.addEventListener('pointercancel', end)
  window.addEventListener('resize', layout)
  const observer = new ResizeObserver(layout)
  observer.observe(node)
  layout()
  return {
    update(next: boolean) { enabled = next; layout() },
    destroy() {
      observer.disconnect()
      window.removeEventListener('resize', layout)
      node.removeEventListener('pointerdown', down)
      node.removeEventListener('pointermove', move)
      node.removeEventListener('pointerup', end)
      node.removeEventListener('pointercancel', end)
    },
  }
}
