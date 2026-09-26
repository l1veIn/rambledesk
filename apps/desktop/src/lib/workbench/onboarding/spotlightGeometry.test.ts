// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { intersectRects, placeSpotlightCard, visibleSpotlightRect } from './spotlightGeometry'

describe('spotlight geometry', () => {
  it('clips a target through both viewport and nested scroll panels', () => {
    const outer = document.createElement('div')
    const inner = document.createElement('div')
    const target = document.createElement('button')
    outer.style.overflowY = 'auto'
    inner.style.overflowX = 'hidden'
    outer.append(inner)
    inner.append(target)
    document.body.append(outer)
    outer.getBoundingClientRect = () => new DOMRect(0, 100, 500, 180)
    inner.getBoundingClientRect = () => new DOMRect(30, 0, 200, 500)
    target.getBoundingClientRect = () => new DOMRect(-10, 80, 360, 300)
    Object.defineProperty(outer, 'clientHeight', { value: 180 })
    Object.defineProperty(inner, 'clientWidth', { value: 200 })
    expect(visibleSpotlightRect(target, { width: 800, height: 240 })).toEqual({ left: 30, top: 100, width: 200, height: 140 })
    outer.remove()
  })

  it('never highlights hidden or completely clipped targets', () => {
    const target = document.createElement('div')
    document.body.append(target)
    target.getBoundingClientRect = () => new DOMRect(20, 20, 100, 100)
    target.style.visibility = 'hidden'
    expect(visibleSpotlightRect(target, { width: 800, height: 600 })).toBeNull()
    target.style.visibility = 'visible'
    target.style.opacity = '0'
    expect(visibleSpotlightRect(target, { width: 800, height: 600 })).toBeNull()
    target.remove()
    expect(intersectRects({ left: -100, top: 0, width: 20, height: 20 }, { left: 0, top: 0, width: 800, height: 600 })).toBeNull()
  })

  it('places the card alongside a tall target without covering it', () => {
    const target = { left: 40, top: 30, width: 450, height: 700 }
    const card = placeSpotlightCard(target, { width: 380, height: 270 }, { width: 1200, height: 800 })
    expect(intersectRects(card, target)).toBeNull()
    expect(card.left).toBeGreaterThan(target.left + target.width)
  })

  it('keeps cards on screen on phones, short viewports, and for absent targets', () => {
    for (const viewport of [{ width: 320, height: 640 }, { width: 600, height: 220 }]) {
      for (const target of [null, { left: 0, top: 0, ...viewport }]) {
        const card = placeSpotlightCard(target, { width: 380, height: 300 }, viewport)
        expect(card.left).toBeGreaterThanOrEqual(16)
        expect(card.top).toBeGreaterThanOrEqual(16)
        expect(card.left + card.width).toBeLessThanOrEqual(viewport.width - 16)
        expect(card.top + card.height).toBeLessThanOrEqual(viewport.height - 16)
      }
    }
  })
})
