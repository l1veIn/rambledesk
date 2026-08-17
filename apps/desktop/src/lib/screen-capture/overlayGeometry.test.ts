import { describe, expect, it } from 'vitest'

import {
  annotationHasSize,
  captureToolbarPosition,
  cssRectangle,
  fitImage,
  imageLayerStyle,
  sourceTolerance,
  type OverlayGeometry,
} from './overlayGeometry'

const geometry: OverlayGeometry = {
  capture: {
    capture_session_id: 'test-session',
    image_width: 2000,
    image_height: 1000,
    targets: [],
    suggested_selection: null,
  },
  displayRectangle: { x: 10, y: 20, width: 1000, height: 500 },
  viewportWidth: 1200,
  viewportHeight: 800,
}

describe('fitImage', () => {
  it('centers a letterboxed image in the viewport', () => {
    expect(fitImage(2000, 1000, 1200, 800)).toEqual({
      x: 0,
      y: 100,
      width: 1200,
      height: 600,
    })
  })
})

describe('sourceTolerance', () => {
  it('scales css pixels to source pixels', () => {
    expect(sourceTolerance(4, geometry)).toBe(8)
  })
})

describe('cssRectangle / imageLayerStyle', () => {
  it('maps source coordinates onto the display layer', () => {
    expect(cssRectangle({ x: 100, y: 50, width: 200, height: 100 }, geometry)).toBe(
      'left:60px;top:45px;width:100px;height:50px',
    )
    expect(imageLayerStyle(geometry)).toBe('left:10px;top:20px;width:1000px;height:500px')
  })
})

describe('captureToolbarPosition', () => {
  it('places the toolbar below the selection when space allows', () => {
    const position = captureToolbarPosition(
      {
        selection: { x: 100, y: 100, width: 400, height: 200 },
        toolbarWidth: 0,
        toolbarHeight: 0,
        toolbarManualX: null,
        toolbarManualY: null,
      },
      geometry,
    )
    expect(position).not.toBeNull()
    expect(position!.top).toBe(182) // selectionBottom (170) + 12
    expect(position!.left).toBe(14) // centered, then clamped to the left margin
  })

  it('centers a measured toolbar under the selection', () => {
    const position = captureToolbarPosition(
      {
        selection: { x: 100, y: 100, width: 400, height: 200 },
        toolbarWidth: 200,
        toolbarHeight: 44,
        toolbarManualX: null,
        toolbarManualY: null,
      },
      geometry,
    )
    expect(position).toEqual({ left: 60, top: 182 })
  })

  it('honors a manual drag position inside the viewport', () => {
    const position = captureToolbarPosition(
      {
        selection: { x: 100, y: 100, width: 400, height: 200 },
        toolbarWidth: 320,
        toolbarHeight: 44,
        toolbarManualX: 80,
        toolbarManualY: 40,
      },
      geometry,
    )
    expect(position).toEqual({ left: 80, top: 40 })
  })

  it('flips above the selection when the bottom would overflow', () => {
    const position = captureToolbarPosition(
      {
        selection: { x: 100, y: 400, width: 400, height: 300 },
        toolbarWidth: 0,
        toolbarHeight: 0,
        toolbarManualX: null,
        toolbarManualY: null,
      },
      geometry,
    )
    expect(position).not.toBeNull()
    expect(position!.top).toBeLessThan(400)
  })
})

describe('annotationHasSize', () => {
  it('treats a click-sized annotation as empty', () => {
    const annotation = {
      id: 'a',
      type: 'rectangle' as const,
      color: '#000',
      strokeWidth: 2,
      rect: { x: 10, y: 10, width: 0.5, height: 0.5 },
    }
    expect(annotationHasSize(annotation, { x: 10, y: 10, width: 0.5, height: 0.5 }, 3)).toBe(
      false,
    )
  })

  it('treats a pen with a single point as empty', () => {
    const annotation = {
      id: 'a',
      type: 'pen' as const,
      color: '#000',
      strokeWidth: 2,
      points: [{ x: 1, y: 1 }],
    }
    expect(annotationHasSize(annotation, { x: 1, y: 1, width: 0, height: 0 }, 3)).toBe(false)
  })
})
