import { afterEach, describe, expect, it, vi } from 'vitest'

import type { CaptureRectangle, ScreenCaptureView } from './screenCapture'
import { captureToolbarPosition, type OverlayGeometry } from './overlayGeometry'
import { createCaptureToolbarPlacement } from './captureToolbarPlacement'

const capture = {
  capture_session_id: 'session-1',
  image_width: 100,
  image_height: 100,
} as unknown as ScreenCaptureView

const geometry: OverlayGeometry = {
  capture,
  displayRectangle: { x: 0, y: 0, width: 100, height: 100 },
  viewportWidth: 800,
  viewportHeight: 800,
}

class FakeElement {
  closest(): null {
    return null
  }
}

function pointerEvent(overrides: Record<string, unknown> = {}) {
  return {
    button: 0,
    isPrimary: true,
    pointerId: 1,
    clientX: 0,
    clientY: 0,
    buttons: 1,
    target: new FakeElement(),
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    ...overrides,
  } as unknown as PointerEvent
}

function harness(selection: CaptureRectangle | null = { x: 0, y: 0, width: 20, height: 20 }) {
  const host = { style: { left: '', top: '' } } as unknown as HTMLElement
  const onChange = vi.fn()
  const addEventListener = vi.fn()
  const removeEventListener = vi.fn()
  vi.stubGlobal('Element', FakeElement)
  vi.stubGlobal('window', { addEventListener, removeEventListener })
  const placement = createCaptureToolbarPlacement({
    getPlacement: () => ({ selection, toolbarWidth: 120, toolbarHeight: 32 }),
    getGeometry: () => geometry,
    getHost: () => host,
    onChange,
  })
  const listener = (type: string) =>
    (addEventListener.mock.calls.find(([name]) => name === type)?.[1] ?? (() => {})) as (
      event: unknown,
    ) => void
  return { placement, host, onChange, addEventListener, removeEventListener, listener }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('capture toolbar placement', () => {
  it('computes and applies the anchored style', () => {
    const { placement, host, onChange } = harness()
    const expected = captureToolbarPosition(
      { selection: { x: 0, y: 0, width: 20, height: 20 }, toolbarWidth: 120, toolbarHeight: 32, toolbarManualX: null, toolbarManualY: null },
      geometry,
    )!

    const css = placement.apply()

    expect(css).toBe(`left:${expected.left}px;top:${expected.top}px`)
    expect(host.style.left).toBe(`${expected.left}px`)
    expect(onChange).toHaveBeenCalledWith(css)
  })

  it('returns no style without a selection', () => {
    const { placement, onChange } = harness(null)
    expect(placement.style()).toBe('')
    expect(placement.apply()).toBe('')
    expect(onChange).not.toHaveBeenCalled()
  })

  it('tracks a manual drag and keeps the host in sync', () => {
    const { placement, host, listener, addEventListener } = harness()
    const origin = captureToolbarPosition(
      { selection: { x: 0, y: 0, width: 20, height: 20 }, toolbarWidth: 120, toolbarHeight: 32, toolbarManualX: null, toolbarManualY: null },
      geometry,
    )!

    placement.beginDrag(pointerEvent())
    expect(addEventListener).toHaveBeenCalledWith('pointermove', expect.any(Function), true)

    listener('pointermove')(pointerEvent({ clientX: 15, clientY: 25 }))

    expect(host.style.left).toBe(`${origin.left + 15}px`)
    expect(host.style.top).toBe(`${origin.top + 25}px`)
  })

  it('stops listening and clears the manual offset on reset', () => {
    const { placement, host, listener, removeEventListener } = harness()
    placement.beginDrag(pointerEvent())
    listener('pointermove')(pointerEvent({ clientX: 15, clientY: 25 }))

    placement.reset()

    expect(host.style.left).toBe('')
    expect(host.style.top).toBe('')
    expect(removeEventListener).toHaveBeenCalledWith('pointermove', expect.any(Function), true)
  })

  it('ignores a drag that starts on a popover control', () => {
    const { placement, addEventListener } = harness()
    placement.beginDrag(
      pointerEvent({ target: { closest: (selector: string) => (selector.includes('button') ? null : {}) } }),
    )
    expect(addEventListener).not.toHaveBeenCalled()
  })
})
