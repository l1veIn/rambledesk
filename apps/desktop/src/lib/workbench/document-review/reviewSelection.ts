import type { ReviewAnchor } from './reviewModel'

/** DOM offsets are UTF-16; the wire contract deliberately uses Unicode scalars. */
export function selectionAnchor(root: HTMLElement, selection: Selection | null): ReviewAnchor | 'cross_paragraph' | null {
  if (!selection || selection.isCollapsed || selection.rangeCount !== 1) return null
  const range = selection.getRangeAt(0)
  const elementFor = (node: Node) => node.nodeType === Node.ELEMENT_NODE ? node as Element : node.parentElement
  const start = elementFor(range.startContainer)?.closest<HTMLElement>('[data-review-text]')
  const end = elementFor(range.endContainer)?.closest<HTMLElement>('[data-review-text]')
  if (!start || !end || !root.contains(start) || !root.contains(end)) return null
  if (start !== end) return 'cross_paragraph'
  const prefix = range.cloneRange()
  prefix.selectNodeContents(start)
  prefix.setEnd(range.startContainer, range.startOffset)
  const startOffset = [...prefix.toString()].length
  const quote = range.toString()
  if (!quote.trim()) return null
  return { paragraph_id: start.dataset.reviewText!, start: startOffset, end: startOffset + [...quote].length, quote }
}
