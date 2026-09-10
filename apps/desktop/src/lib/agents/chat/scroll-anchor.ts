export type ActivityScrollAnchor = Readonly<{ id: string; offset: number; scrollTop: number; scrollHeight: number }>

export function captureActivityAnchor(viewport: HTMLElement): ActivityScrollAnchor | null {
  const top = viewport.getBoundingClientRect().top
  const firstVisible = Array.from(viewport.querySelectorAll<HTMLElement>('[data-activity-id]'))
    .find((element) => element.getBoundingClientRect().bottom > top)
  if (!firstVisible?.dataset.activityId) return null
  return { id: firstVisible.dataset.activityId, offset: firstVisible.getBoundingClientRect().top - top,
    scrollTop: viewport.scrollTop, scrollHeight: viewport.scrollHeight }
}

export function restoreActivityAnchor(viewport: HTMLElement, anchor: ActivityScrollAnchor): void {
  const element = Array.from(viewport.querySelectorAll<HTMLElement>('[data-activity-id]'))
    .find((element) => element.dataset.activityId === anchor.id)
  viewport.scrollTop = element
    ? viewport.scrollTop + element.getBoundingClientRect().top - viewport.getBoundingClientRect().top - anchor.offset
    : anchor.scrollTop + viewport.scrollHeight - anchor.scrollHeight
}
