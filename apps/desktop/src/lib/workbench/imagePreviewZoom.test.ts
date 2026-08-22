import { describe, expect, it } from 'vitest'

import {
  clampImageZoom,
  computeImagePreviewZoom,
  imageDisplaySize,
} from './imagePreviewZoom'

describe('image preview zoom', () => {
  it('starts long images at fit-width but allows shrinking to full-height', () => {
    const model = computeImagePreviewZoom({
      naturalWidth: 2000,
      naturalHeight: 10000,
      viewportWidth: 1000,
      viewportHeight: 700,
    })

    expect(model.initialZoom).toBeCloseTo(0.5)
    expect(model.minZoom).toBeLessThan(model.initialZoom)
    expect(model.minZoom).toBeCloseTo(0.07)
    expect(imageDisplaySize(2000, 10000, model.initialZoom)).toEqual({
      width: 1000,
      height: 5000,
    })
    expect(imageDisplaySize(2000, 10000, model.minZoom)).toEqual({
      width: 140,
      height: 700,
    })
  })

  it('clamps zoom to the image-specific minimum', () => {
    const model = computeImagePreviewZoom({
      naturalWidth: 1200,
      naturalHeight: 3600,
      viewportWidth: 600,
      viewportHeight: 600,
    })

    expect(clampImageZoom(0.01, model)).toBe(model.minZoom)
  })

  it('does not collapse an image with unknown intrinsic size to one pixel', () => {
    expect(imageDisplaySize(0, 0, 1)).toBeNull()
  })
})
