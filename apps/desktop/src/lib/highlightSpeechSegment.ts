export function highlightSpeechSegment(root: ParentNode, segmentId: string, scroll = false) {
  const paragraph = [...root.querySelectorAll<HTMLElement>('[data-speech-segment-id]')]
    .find((element) => element.dataset.speechSegmentId === segmentId)
  if (!paragraph) return
  if (scroll) paragraph.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  if (typeof paragraph.animate !== 'function' || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
  paragraph.animate([
    { backgroundColor: 'color-mix(in srgb, var(--primary) 20%, transparent)' },
    { backgroundColor: 'transparent' },
  ], { duration: 1800, easing: 'ease-out' })
}
