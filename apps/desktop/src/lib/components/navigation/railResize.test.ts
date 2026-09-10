import { describe, expect, it } from 'vitest'

import {
  COLLAPSED_RAIL_WIDTH,
  RAIL_LIMITS,
  fitNavigationWidths,
  normalizeRailWidth,
  resolveRailDrag,
} from './railResize'

describe('navigation rail resizing', () => {
  it('defaults invalid preferences and bounds finite expanded widths by rail', () => {
    for (const value of [undefined, null, '280', true, {}, [], NaN, Infinity, -Infinity]) {
      expect(normalizeRailWidth('host', value)).toBe(240)
      expect(normalizeRailWidth('request', value)).toBe(240)
    }
    expect(normalizeRailWidth('host', 20)).toBe(192)
    expect(normalizeRailWidth('request', 20)).toBe(200)
    expect(normalizeRailWidth('host', 999)).toBe(360)
    expect(normalizeRailWidth('request', 999)).toBe(400)
    expect(normalizeRailWidth('host', 257.5)).toBe(257.5)
  })

  it('snaps closed at or below the minimum while retaining the expanded preference', () => {
    for (const delta of [-80, -81, -1000]) {
      expect(resolveRailDrag({
        initialWidth: 280,
        initialExpandedWidth: 280,
        delta,
        minWidth: 200,
        maxWidth: 400,
      })).toEqual({ width: 280, collapsed: true })
    }
  })

  it('resizes immediately above the minimum and caps the maximum', () => {
    const drag = { initialWidth: 280, initialExpandedWidth: 280, minWidth: 200, maxWidth: 400 }
    expect(resolveRailDrag({ ...drag, delta: -79 })).toEqual({ width: 201, collapsed: false })
    expect(resolveRailDrag({ ...drag, delta: 40 })).toEqual({ width: 320, collapsed: false })
    expect(resolveRailDrag({ ...drag, delta: 1000 })).toEqual({ width: 400, collapsed: false })
  })

  it('keeps a collapsed rail closed until the drag crosses its expanded minimum', () => {
    const drag = {
      initialWidth: COLLAPSED_RAIL_WIDTH,
      initialExpandedWidth: 310,
      minWidth: RAIL_LIMITS.host.minWidth,
      maxWidth: RAIL_LIMITS.host.maxWidth,
    }
    expect(resolveRailDrag({ ...drag, delta: 0 })).toEqual({ width: 310, collapsed: true })
    expect(resolveRailDrag({ ...drag, delta: 136 })).toEqual({ width: 310, collapsed: true })
    expect(resolveRailDrag({ ...drag, delta: 137 })).toEqual({ width: 193, collapsed: false })
  })

  it('projects preferred widths into available room by shrinking the request rail first', () => {
    const preferred = {
      hostWidth: 360,
      requestWidth: 400,
      hostCollapsed: false,
      requestCollapsed: false,
    }
    expect(fitNavigationWidths({ ...preferred, containerWidth: 1600 })).toEqual({ host: 360, request: 400 })
    expect(fitNavigationWidths({ ...preferred, containerWidth: 1320 })).toEqual({ host: 360, request: 320 })
    expect(fitNavigationWidths({ ...preferred, containerWidth: 1100 })).toEqual({ host: 260, request: 200 })
    expect(fitNavigationWidths({ ...preferred, containerWidth: 800 })).toEqual({ host: 192, request: 200 })
    // A roomier window restores the same preferred widths: projection has no persistence side effects.
    expect(fitNavigationWidths({ ...preferred, containerWidth: 1600 })).toEqual({ host: 360, request: 400 })
    expect(preferred).toEqual({ hostWidth: 360, requestWidth: 400, hostCollapsed: false, requestCollapsed: false })
  })

  it('reserves only visible rail widths and preserves collapsed rails at 56px', () => {
    const preferred = { hostWidth: 360, requestWidth: 400, hostCollapsed: false, requestCollapsed: false }
    expect(fitNavigationWidths({ ...preferred, requestWidth: null, containerWidth: 900 })).toEqual({ host: 260, request: 0 })
    expect(fitNavigationWidths({ ...preferred, hostCollapsed: true, containerWidth: 900 })).toEqual({ host: 56, request: 204 })
    expect(fitNavigationWidths({ ...preferred, requestCollapsed: true, containerWidth: 900 })).toEqual({ host: 204, request: 56 })
    expect(fitNavigationWidths({ ...preferred, hostCollapsed: true, requestCollapsed: true, containerWidth: 100 })).toEqual({ host: 56, request: 56 })
    expect(fitNavigationWidths({ ...preferred, hostWidth: 999, requestWidth: -1, containerWidth: 1600 })).toEqual({ host: 360, request: 200 })
  })
})
