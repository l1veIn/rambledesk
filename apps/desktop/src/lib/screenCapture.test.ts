import { describe, expect, it } from 'vitest'

import { normalizeCaptureSelection } from './screenCapture'

describe('normalizeCaptureSelection', () => {
  it('normalizes reverse dragging and maps CSS pixels to captured pixels', () => {
    expect(
      normalizeCaptureSelection(
        { x: 500, y: 300 },
        { x: 100, y: 50 },
        960,
        540,
        1920,
        1080,
      ),
    ).toEqual({ x: 200, y: 100, width: 800, height: 500 })
  })

  it('clamps a selection to the monitor image', () => {
    expect(
      normalizeCaptureSelection(
        { x: -20, y: -10 },
        { x: 1200, y: 800 },
        1000,
        700,
        1000,
        700,
      ),
    ).toEqual({ x: 0, y: 0, width: 1000, height: 700 })
  })
})
