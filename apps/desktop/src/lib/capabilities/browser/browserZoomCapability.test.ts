import { describe, expect, it } from 'vitest'
import { createBrowserZoomCapability } from './browserZoomCapability'

function root() {
  return { style: { zoom: '', width: '', height: '' } } as Pick<HTMLElement, 'style'>
}

describe('browser workbench zoom', () => {
  it('scales only the supplied document root and restores its original sizing', async () => {
    const main = root()
    main.style.width = '100%'
    main.style.height = '100%'
    const overlay = root()
    const zoom = createBrowserZoomCapability(main)
    await zoom.setZoom(1.5)
    expect(main.style.zoom).toBe('1.5')
    expect(main.style.width).not.toBe('100%')
    expect(main.style.height).not.toBe('100%')
    expect(overlay.style).toEqual({ zoom: '', width: '', height: '' })
    await zoom.setZoom(1)
    expect(main.style).toEqual({ zoom: '', width: '100%', height: '100%' })
  })

  it('rejects out-of-range and non-finite zoom without changing the current view', async () => {
    const main = root()
    const zoom = createBrowserZoomCapability(main)
    await zoom.setZoom(0.8)
    const before = { ...main.style }
    for (const factor of [0, 0.79, 3.01, Number.NaN, Number.POSITIVE_INFINITY]) {
      await expect(zoom.setZoom(factor)).rejects.toThrow(RangeError)
    }
    expect(main.style).toEqual(before)
    await zoom.setZoom(3)
    expect(main.style.zoom).toBe('3')
  })
})
