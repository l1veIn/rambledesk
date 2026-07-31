export type ScreenCaptureView = {
  session_id: string
  width: number
  height: number
}

export type ScreenCaptureReady = {
  session_id: string
  file_name: string
}

export type CapturePoint = {
  x: number
  y: number
}

export type CaptureRectangle = {
  x: number
  y: number
  width: number
  height: number
}

export function normalizeCaptureSelection(
  start: CapturePoint,
  end: CapturePoint,
  viewportWidth: number,
  viewportHeight: number,
  imageWidth: number,
  imageHeight: number,
): CaptureRectangle {
  const left = Math.max(0, Math.min(start.x, end.x, viewportWidth))
  const top = Math.max(0, Math.min(start.y, end.y, viewportHeight))
  const right = Math.max(0, Math.min(Math.max(start.x, end.x), viewportWidth))
  const bottom = Math.max(0, Math.min(Math.max(start.y, end.y), viewportHeight))
  const scaleX = viewportWidth > 0 ? imageWidth / viewportWidth : 1
  const scaleY = viewportHeight > 0 ? imageHeight / viewportHeight : 1
  const x = Math.floor(left * scaleX)
  const y = Math.floor(top * scaleY)
  return {
    x,
    y,
    width: Math.min(imageWidth - x, Math.ceil((right - left) * scaleX)),
    height: Math.min(imageHeight - y, Math.ceil((bottom - top) * scaleY)),
  }
}
