const MIN_ABSOLUTE_ZOOM = 0.05
const MAX_ABSOLUTE_ZOOM = 8

export type ImagePreviewBounds = {
  naturalWidth: number
  naturalHeight: number
  viewportWidth: number
  viewportHeight: number
}

export type ImagePreviewZoomModel = {
  fitWidthZoom: number
  fitPageZoom: number
  initialZoom: number
  minZoom: number
  maxZoom: number
}

function usable(value: number) {
  return Number.isFinite(value) && value > 0
}

export function computeImagePreviewZoom(bounds: ImagePreviewBounds): ImagePreviewZoomModel {
  const naturalWidth = usable(bounds.naturalWidth) ? bounds.naturalWidth : 1
  const naturalHeight = usable(bounds.naturalHeight) ? bounds.naturalHeight : 1
  const viewportWidth = usable(bounds.viewportWidth) ? bounds.viewportWidth : naturalWidth
  const viewportHeight = usable(bounds.viewportHeight) ? bounds.viewportHeight : naturalHeight
  const fitWidthZoom = Math.min(1, viewportWidth / naturalWidth)
  const fitPageZoom = Math.min(fitWidthZoom, viewportHeight / naturalHeight)

  return {
    fitWidthZoom,
    fitPageZoom,
    initialZoom: fitWidthZoom,
    minZoom: Math.max(MIN_ABSOLUTE_ZOOM, fitPageZoom),
    maxZoom: MAX_ABSOLUTE_ZOOM,
  }
}

export function clampImageZoom(value: number, model: ImagePreviewZoomModel): number {
  return Math.min(model.maxZoom, Math.max(model.minZoom, value))
}

export function imageDisplaySize(
  naturalWidth: number,
  naturalHeight: number,
  zoom: number,
): { width: number; height: number } | null {
  if (!usable(naturalWidth) || !usable(naturalHeight) || !usable(zoom)) return null
  return {
    width: Math.max(1, Math.round(naturalWidth * zoom)),
    height: Math.max(1, Math.round(naturalHeight * zoom)),
  }
}
