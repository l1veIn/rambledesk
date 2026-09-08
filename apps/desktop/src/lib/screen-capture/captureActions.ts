import { exportAnnotatedCapture } from '../screenshotRenderer'
import type {
  CaptureAnnotation,
  CaptureRectangle,
  ScreenCaptureView,
} from '../screenCapture'

export type CompleteCapturePayload = {
  capture_session_id: string
  selection: CaptureRectangle
  png_base64: string | null
  copy_to_clipboard: boolean
}

export type ScrollCapturePayload = {
  capture_session_id: string
  selection: CaptureRectangle
}

export function roundedRectangle(rectangle: CaptureRectangle): CaptureRectangle {
  return {
    x: Math.max(0, Math.round(rectangle.x)),
    y: Math.max(0, Math.round(rectangle.y)),
    width: Math.max(1, Math.round(rectangle.width)),
    height: Math.max(1, Math.round(rectangle.height)),
  }
}

export type CaptureActionsContext = {
  /** Literal Tauri calls stay in the platform-window component. */
  complete: (input: CompleteCapturePayload) => Promise<unknown>
  pin: (input: CompleteCapturePayload) => Promise<unknown>
  startScrolling: (input: ScrollCapturePayload) => Promise<unknown>
  cancel: () => Promise<unknown>
  tr: (source: string) => string
  messageFrom: (cause: unknown) => string
  getCapture: () => ScreenCaptureView | null
  getSourceImage: () => HTMLCanvasElement | null
  getSelection: () => CaptureRectangle | null
  getAnnotations: () => readonly CaptureAnnotation[]
  isCompleting: () => boolean
  /** Flushes an in-progress text annotation before the capture is exported. */
  commitText: () => void
  setCompleting: (value: boolean) => void
  setError: (message: string) => void
}

/**
 * Completing a capture: publish, pin, hand off to scrolling capture, or cancel.
 * Every path exports the annotated image with the same payload shape.
 */
export function createCaptureActions(context: CaptureActionsContext) {
  function exportPayload(
    sourceImage: HTMLCanvasElement,
    selection: CaptureRectangle,
    annotations: readonly CaptureAnnotation[],
  ) {
    if (annotations.length === 0) return null
    return exportAnnotatedCapture(
      sourceImage,
      selection,
      annotations as CaptureAnnotation[],
      context.tr('Could not create the capture export canvas'),
    )
  }

  async function complete(
    send: (input: CompleteCapturePayload) => Promise<unknown>,
    copyToClipboard: boolean,
  ) {
    const capture = context.getCapture()
    const sourceImage = context.getSourceImage()
    const selection = context.getSelection()
    if (!capture || !sourceImage || !selection || context.isCompleting()) return
    context.commitText()
    context.setCompleting(true)
    context.setError('')
    try {
      await send({
        capture_session_id: capture.capture_session_id,
        selection: roundedRectangle(selection),
        png_base64: exportPayload(sourceImage, selection, context.getAnnotations()),
        copy_to_clipboard: copyToClipboard,
      })
    } catch (cause) {
      context.setError(context.messageFrom(cause))
      context.setCompleting(false)
    }
  }

  async function finalize(copyToClipboard: boolean) {
    await complete(context.complete, copyToClipboard)
  }

  async function pinCapture() {
    await complete(context.pin, false)
  }

  async function beginScrolling() {
    const capture = context.getCapture()
    const selection = context.getSelection()
    if (!capture || !selection || context.isCompleting()) return
    if (context.getAnnotations().length > 0) {
      context.setError(context.tr('Start scrolling capture before annotating the stitched image.'))
      return
    }
    context.setCompleting(true)
    context.setError('')
    try {
      await context.startScrolling({
        capture_session_id: capture.capture_session_id,
        selection: roundedRectangle(selection),
      })
    } catch (cause) {
      context.setError(context.messageFrom(cause))
      context.setCompleting(false)
    }
  }

  async function cancelCapture() {
    if (context.isCompleting()) return
    context.setCompleting(true)
    try {
      await context.cancel()
    } catch (cause) {
      context.setError(context.messageFrom(cause))
      context.setCompleting(false)
    }
  }

  return { finalize, pinCapture, beginScrolling, cancelCapture }
}

export type CaptureActions = ReturnType<typeof createCaptureActions>
