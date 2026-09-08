import { afterEach, describe, expect, it, vi } from 'vitest'

import type { CaptureAnnotation, ScreenCaptureView } from '../screenCapture'
import { createCaptureActions, roundedRectangle, type CaptureActionsContext } from './captureActions'

const capture = {
  capture_session_id: 'session-1',
  image_width: 100,
  image_height: 100,
} as unknown as ScreenCaptureView

function harness(overrides: Partial<CaptureActionsContext> = {}) {
  const complete = vi.fn(async () => undefined)
  const pin = vi.fn(async () => undefined)
  const startScrolling = vi.fn(async () => undefined)
  const cancel = vi.fn(async () => undefined)
  const context = {
    complete,
    pin,
    startScrolling,
    cancel,
    tr: (source: string) => source,
    messageFrom: (cause: unknown) => String(cause),
    getCapture: () => capture,
    getSourceImage: () => ({}) as HTMLCanvasElement,
    getSelection: () => ({ x: 1.4, y: 2.6, width: 30.2, height: 40.8 }),
    getAnnotations: () => [] as readonly CaptureAnnotation[],
    isCompleting: () => false,
    commitText: vi.fn(),
    setCompleting: vi.fn(),
    setError: vi.fn(),
    ...overrides,
  } as CaptureActionsContext
  return { actions: createCaptureActions(context), context, complete, pin, startScrolling, cancel }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('capture actions', () => {
  it('rounds a selection into integer pixel bounds', () => {
    expect(roundedRectangle({ x: -2.4, y: 1.6, width: 0, height: 12.5 })).toEqual({
      x: 0,
      y: 2,
      width: 1,
      height: 13,
    })
  })

  it('completes a capture without an export when there are no annotations', async () => {
    const { actions, context, complete } = harness()

    await actions.finalize(true)

    expect(context.commitText).toHaveBeenCalled()
    expect(context.setCompleting).toHaveBeenCalledWith(true)
    expect(complete).toHaveBeenCalledWith({
      capture_session_id: 'session-1',
      selection: { x: 1, y: 3, width: 30, height: 41 },
      png_base64: null,
      copy_to_clipboard: true,
    })
  })

  it('pins without copying to the clipboard', async () => {
    const { actions, pin } = harness()
    await actions.pinCapture()
    expect(pin).toHaveBeenCalledWith(expect.objectContaining({ copy_to_clipboard: false }))
  })

  it('reports a failed completion and releases the completing flag', async () => {
    const { actions, context } = harness({
      complete: vi.fn(async () => {
        throw new Error('capture failed')
      }) as never,
    })

    await actions.finalize(false)

    expect(context.setError).toHaveBeenCalledWith('Error: capture failed')
    expect(context.setCompleting).toHaveBeenLastCalledWith(false)
  })

  it('refuses scrolling capture once the image has annotations', async () => {
    const { actions, context, startScrolling } = harness({
      getAnnotations: () => [{ id: 'a' }] as CaptureAnnotation[],
    })

    await actions.beginScrolling()

    expect(context.setError).toHaveBeenCalledWith(
      'Start scrolling capture before annotating the stitched image.',
    )
    expect(startScrolling).not.toHaveBeenCalled()
  })

  it('cancels without touching a completing capture', async () => {
    const { actions, cancel } = harness({ isCompleting: () => true })
    await actions.cancelCapture()
    expect(cancel).not.toHaveBeenCalled()
  })
})
