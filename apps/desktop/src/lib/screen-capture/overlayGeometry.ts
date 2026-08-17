// Pure geometry and layout helpers for the screen-capture overlay. All
// functions are stateless: the overlay component passes its current capture,
// display rectangle, and UI state explicitly, so these stay unit-testable.

import {
  pointInRectangle,
  type CaptureAnnotation,
  type CapturePoint,
  type CaptureRectangle,
  type CaptureTarget,
  type ScreenCaptureView,
} from '../screenCapture'

export type OverlayGeometry = {
  capture: ScreenCaptureView | null
  displayRectangle: CaptureRectangle | null
  viewportWidth: number
  viewportHeight: number
}

/** Fit an image of `imageWidth x imageHeight` into a `width x height` viewport. */
export function fitImage(
  imageWidth: number,
  imageHeight: number,
  width: number,
  height: number,
): CaptureRectangle {
  const scale = Math.min(width / imageWidth, height / imageHeight)
  const fittedWidth = imageWidth * scale
  const fittedHeight = imageHeight * scale
  return {
    x: (width - fittedWidth) / 2,
    y: (height - fittedHeight) / 2,
    width: fittedWidth,
    height: fittedHeight,
  }
}

/** Map a client-space pointer event into source image coordinates. */
export function imagePoint(
  event: PointerEvent | MouseEvent,
  shellBounds: DOMRect,
  geometry: OverlayGeometry,
): CapturePoint | null {
  if (!geometry.capture || !geometry.displayRectangle) return null
  const x = event.clientX - shellBounds.left - geometry.displayRectangle.x
  const y = event.clientY - shellBounds.top - geometry.displayRectangle.y
  if (
    x < 0 ||
    y < 0 ||
    x > geometry.displayRectangle.width ||
    y > geometry.displayRectangle.height
  ) return null
  return {
    x: (x / geometry.displayRectangle.width) * geometry.capture.image_width,
    y: (y / geometry.displayRectangle.height) * geometry.capture.image_height,
  }
}

/** The capture target (window/region candidate) containing a source point. */
export function findCaptureTarget(
  point: CapturePoint,
  targets: CaptureTarget[],
): CaptureTarget | null {
  return targets.find((target) => pointInRectangle(point, target)) ?? null
}

export function captureTargetRectangle(target: CaptureTarget): CaptureRectangle {
  return { x: target.x, y: target.y, width: target.width, height: target.height }
}

/** A selection covering the full captured image. */
export function fullScreenSelection(capture: ScreenCaptureView): CaptureRectangle {
  return { x: 0, y: 0, width: capture.image_width, height: capture.image_height }
}

export function clampPointToSelection(
  point: CapturePoint,
  selection: CaptureRectangle | null,
): CapturePoint {
  if (!selection) return point
  return {
    x: Math.min(selection.x + selection.width, Math.max(selection.x, point.x)),
    y: Math.min(selection.y + selection.height, Math.max(selection.y, point.y)),
  }
}

/** Convert a CSS pixel tolerance into source pixels for the current scale. */
export function sourceTolerance(cssPixels: number, geometry: OverlayGeometry): number {
  if (!geometry.capture || !geometry.displayRectangle) return cssPixels
  return cssPixels * (geometry.capture.image_width / Math.max(1, geometry.displayRectangle.width))
}

/** CSS positioning for one source rectangle on the display layer. */
export function cssRectangle(rectangle: CaptureRectangle, geometry: OverlayGeometry): string {
  if (!geometry.capture || !geometry.displayRectangle) return ''
  const scaleX = geometry.displayRectangle.width / geometry.capture.image_width
  const scaleY = geometry.displayRectangle.height / geometry.capture.image_height
  return `left:${geometry.displayRectangle.x + rectangle.x * scaleX}px;top:${geometry.displayRectangle.y + rectangle.y * scaleY}px;width:${rectangle.width * scaleX}px;height:${rectangle.height * scaleY}px`
}

/** CSS positioning for the image display layer. */
export function imageLayerStyle(geometry: OverlayGeometry): string {
  if (!geometry.displayRectangle) return ''
  return `left:${geometry.displayRectangle.x}px;top:${geometry.displayRectangle.y}px;width:${geometry.displayRectangle.width}px;height:${geometry.displayRectangle.height}px`
}

export type TextDraftStyleInput = {
  point: CapturePoint
  color: string
  strokeWidth: number
}

/** CSS positioning and appearance for the in-progress text annotation. */
export function textDraftStyle(
  draft: TextDraftStyleInput,
  geometry: OverlayGeometry,
): string {
  if (!geometry.capture || !geometry.displayRectangle) return ''
  const scaleX = geometry.displayRectangle.width / geometry.capture.image_width
  const scaleY = geometry.displayRectangle.height / geometry.capture.image_height
  return `left:${geometry.displayRectangle.x + draft.point.x * scaleX}px;top:${geometry.displayRectangle.y + draft.point.y * scaleY}px;color:${draft.color};font-size:${Math.max(14, draft.strokeWidth * 5 * scaleY)}px`
}

export type ToolbarPlacement = {
  selection: CaptureRectangle | null
  toolbarWidth: number
  toolbarHeight: number
  toolbarManualX: number | null
  toolbarManualY: number | null
}

/** Preferred toolbar position anchored below the selection. */
export function captureToolbarPosition(
  placement: ToolbarPlacement,
  geometry: OverlayGeometry,
): { left: number; top: number } | null {
  if (!placement.selection || !geometry.capture || !geometry.displayRectangle) return null
  const scaleX = geometry.displayRectangle.width / geometry.capture.image_width
  const scaleY = geometry.displayRectangle.height / geometry.capture.image_height
  const selectionLeft = geometry.displayRectangle.x + placement.selection.x * scaleX
  const selectionTop = geometry.displayRectangle.y + placement.selection.y * scaleY
  const selectionWidth = placement.selection.width * scaleX
  const selectionBottom = selectionTop + placement.selection.height * scaleY
  const viewportWidth = Math.max(1, geometry.viewportWidth)
  const viewportHeight = Math.max(1, geometry.viewportHeight)
  const measuredWidth = placement.toolbarWidth > 0
    ? placement.toolbarWidth
    : Math.min(420, Math.max(280, viewportWidth - 28))
  const measuredHeight = placement.toolbarHeight > 0 ? placement.toolbarHeight : 50
  const centeredLeft = selectionLeft + (selectionWidth - measuredWidth) / 2
  const below = selectionBottom + 12
  const baseTop =
    below + measuredHeight <= viewportHeight - 14
      ? below
      : Math.max(14, selectionTop - measuredHeight - 12)
  const maxLeft = Math.max(14, viewportWidth - measuredWidth - 14)
  const maxTop = Math.max(14, viewportHeight - measuredHeight - 14)
  const left = Math.min(Math.max(14, placement.toolbarManualX ?? centeredLeft), maxLeft)
  const top = Math.min(Math.max(14, placement.toolbarManualY ?? baseTop), maxTop)
  return { left, top }
}

/** Whether the toolbar popover should open below the toolbar. */
export function toolbarPopoverOpensDownward(position: { left: number; top: number } | null) {
  return position ? position.top < 96 : false
}

/** An annotation that was created without being dragged has no visible size. */
export function annotationHasSize(
  annotation: CaptureAnnotation,
  bounds: CaptureRectangle,
  minSize: number,
): boolean {
  if (annotation.type === 'pen') return annotation.points.length > 1
  return Math.max(bounds.width, bounds.height) >= minSize
}
