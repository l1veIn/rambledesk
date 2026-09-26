export type SpotlightStep = Readonly<{ id: string; target: string; title: string; body: string }>
export type SpotlightLabels = {
  dialog: string
  skip: string
  back: string
  next: string
  done: string
  /** Index is zero based; the presentation supplies the human-readable number. */
  step: (index: number, total: number) => string
}

export type SpotlightRect = { left: number; top: number; width: number; height: number }
type Size = { width: number; height: number }

export function intersectRects(a: SpotlightRect, b: SpotlightRect): SpotlightRect | null {
  const left = Math.max(a.left, b.left)
  const top = Math.max(a.top, b.top)
  const right = Math.min(a.left + a.width, b.left + b.width)
  const bottom = Math.min(a.top + a.height, b.top + b.height)
  return right > left && bottom > top ? { left, top, width: right - left, height: bottom - top } : null
}

/** The cutout must never reveal content clipped by a nested scrolling panel. */
export function visibleSpotlightRect(element: HTMLElement, viewport: Size): SpotlightRect | null {
  let rect = intersectRects(element.getBoundingClientRect(), { left: 0, top: 0, ...viewport })
  for (let node: HTMLElement | null = element; node && rect; node = node.parentElement) {
    const style = getComputedStyle(node)
    if (style.display === 'none' || style.visibility === 'hidden' || style.visibility === 'collapse' || style.opacity === '0') return null
    if (node === element) continue
    const clipsX = /^(auto|scroll|hidden|clip)$/.test(style.overflowX)
    const clipsY = /^(auto|scroll|hidden|clip)$/.test(style.overflowY)
    if (!clipsX && !clipsY) continue
    const bounds = node.getBoundingClientRect()
    rect = intersectRects(rect, {
      left: clipsX ? bounds.left + node.clientLeft : 0,
      top: clipsY ? bounds.top + node.clientTop : 0,
      width: clipsX ? node.clientWidth : viewport.width,
      height: clipsY ? node.clientHeight : viewport.height,
    })
  }
  return rect
}

const clamp = (value: number, minimum: number, maximum: number) => Math.max(minimum, Math.min(value, maximum))

/** Prefer a fully separate card, then the smallest overlap on a narrow viewport. */
export function placeSpotlightCard(target: SpotlightRect | null, card: Size, viewport: Size): SpotlightRect {
  const margin = 16
  const gap = 18
  const width = Math.min(card.width, Math.max(0, viewport.width - margin * 2))
  const height = Math.min(card.height, Math.max(0, viewport.height - margin * 2))
  const fit = (left: number, top: number): SpotlightRect => ({
    left: clamp(left, margin, viewport.width - width - margin),
    top: clamp(top, margin, viewport.height - height - margin), width, height,
  })
  if (!target) return fit((viewport.width - width) / 2, (viewport.height - height) / 2)
  const centerX = target.left + (target.width - width) / 2
  const centerY = target.top + (target.height - height) / 2
  const candidates = [
    fit(centerX, target.top + target.height + gap),
    fit(target.left + target.width + gap, centerY),
    fit(target.left - width - gap, centerY),
    fit(centerX, target.top - height - gap),
  ]
  const padded = { left: target.left - gap / 2, top: target.top - gap / 2, width: target.width + gap, height: target.height + gap }
  const overlap = (rect: SpotlightRect) => { const intersection = intersectRects(rect, padded); return intersection ? intersection.width * intersection.height : 0 }
  return candidates.reduce((best, candidate) => overlap(candidate) < overlap(best) ? candidate : best)
}
