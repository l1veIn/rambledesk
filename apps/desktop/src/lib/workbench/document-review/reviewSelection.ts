import type { ReviewAnchor } from './reviewModel'
import { completeInlineBoundaries, leafBoundary, type ReviewDocument } from './reviewMarkdown'

/** Clip the browser range to mapped leaves, then return an exact raw Markdown slice. */
export function selectionAnchor(root: HTMLElement, selection: Selection | null, model: ReviewDocument): ReviewAnchor | 'unmapped' | null {
  if (!selection || selection.isCollapsed || selection.rangeCount !== 1) return null
  const range = selection.getRangeAt(0)
  if (!root.contains(range.startContainer) || !root.contains(range.endContainer)) return null
  const inNote = (node: Node) => (node instanceof Element ? node : node.parentElement)?.closest('[data-review-note-region]')
  if (inNote(range.startContainer) || inNote(range.endContainer)) return null
  if (!range.toString().trim()) return null
  if ([...root.querySelectorAll('[data-review-unmapped]')].some((node) => range.intersectsNode(node))) return 'unmapped'
  let start: number | null = null, end: number | null = null
  for (const element of root.querySelectorAll<HTMLElement>('[data-review-leaf]')) {
    if (!range.intersectsNode(element)) continue
    const leaf = model.leaves.get(element.dataset.reviewLeaf!)
    if (!leaf || element.textContent !== leaf.text) return 'unmapped'
    const localOffset = (container: Node, offset: number, fallback: number) => {
      if (!element.contains(container)) return fallback
      const prefix = range.cloneRange()
      prefix.selectNodeContents(element); prefix.setEnd(container, offset)
      return prefix.toString().length
    }
    const from = localOffset(range.startContainer, range.startOffset, 0)
    const to = localOffset(range.endContainer, range.endOffset, leaf.text.length)
    if (from >= to) continue
    const first = leafBoundary(leaf, from, 'start'), last = leafBoundary(leaf, to, 'end')
    if (first === null || last === null) return 'unmapped'
    start ??= first; end = last
  }
  if (start === null || end === null || end <= start) return null
  ;[start, end] = completeInlineBoundaries(model, start, end)
  return { start, end, quote: [...model.markdown].slice(start, end).join('') }
}
