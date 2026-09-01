export type ScreenCaptureView = {
  capture_session_id: string
  image_width: number
  image_height: number
  targets: CaptureTarget[]
  suggested_selection: CaptureRectangle | null
}

export type CaptureTarget = CaptureRectangle & {
  id: string
  title: string
  app_name: string
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

export type AnnotationTool =
  | 'select'
  | 'rectangle'
  | 'ellipse'
  | 'arrow'
  | 'line'
  | 'pen'
  | 'text'
  | 'highlight'
  | 'mosaic'
  | 'counter'

type AnnotationBase = {
  id: string
  color: string
  strokeWidth: number
}

export type ShapeAnnotation = AnnotationBase & {
  type: 'rectangle' | 'ellipse' | 'highlight' | 'mosaic'
  rect: CaptureRectangle
  pixelSize?: number
}

export type LinearAnnotation = AnnotationBase & {
  type: 'arrow' | 'line'
  start: CapturePoint
  end: CapturePoint
}

export type PenAnnotation = AnnotationBase & {
  type: 'pen'
  points: CapturePoint[]
}

export type TextAnnotation = AnnotationBase & {
  type: 'text'
  point: CapturePoint
  text: string
  fontSize: number
}

export type CounterAnnotation = AnnotationBase & {
  type: 'counter'
  point: CapturePoint
  number: number
  radius: number
}

export type CaptureAnnotation =
  | ShapeAnnotation
  | LinearAnnotation
  | PenAnnotation
  | TextAnnotation
  | CounterAnnotation

export type ResizeHandle = 'nw' | 'ne' | 'se' | 'sw'

export function normalizeCaptureRectangle(
  start: CapturePoint,
  end: CapturePoint,
): CaptureRectangle {
  return {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  }
}

export function clampCaptureRectangle(
  rectangle: CaptureRectangle,
  imageWidth: number,
  imageHeight: number,
  minimumSize = 1,
): CaptureRectangle {
  const width = Math.min(Math.max(minimumSize, rectangle.width), imageWidth)
  const height = Math.min(Math.max(minimumSize, rectangle.height), imageHeight)
  return {
    x: clamp(rectangle.x, 0, Math.max(0, imageWidth - width)),
    y: clamp(rectangle.y, 0, Math.max(0, imageHeight - height)),
    width,
    height,
  }
}

export function moveCaptureRectangle(
  rectangle: CaptureRectangle,
  delta: CapturePoint,
  imageWidth: number,
  imageHeight: number,
): CaptureRectangle {
  return clampCaptureRectangle(
    { ...rectangle, x: rectangle.x + delta.x, y: rectangle.y + delta.y },
    imageWidth,
    imageHeight,
  )
}

export function resizeCaptureRectangle(
  rectangle: CaptureRectangle,
  handle: ResizeHandle,
  point: CapturePoint,
  imageWidth: number,
  imageHeight: number,
  minimumSize = 8,
): CaptureRectangle {
  const opposite: CapturePoint = {
    x: handle.includes('w') ? rectangle.x + rectangle.width : rectangle.x,
    y: handle.includes('n') ? rectangle.y + rectangle.height : rectangle.y,
  }
  const dragged = {
    x: clamp(point.x, 0, imageWidth),
    y: clamp(point.y, 0, imageHeight),
  }
  const next = normalizeCaptureRectangle(opposite, dragged)
  if (next.width < minimumSize) {
    next.width = minimumSize
    next.x = handle.includes('w') ? opposite.x - minimumSize : opposite.x
  }
  if (next.height < minimumSize) {
    next.height = minimumSize
    next.y = handle.includes('n') ? opposite.y - minimumSize : opposite.y
  }
  return clampCaptureRectangle(next, imageWidth, imageHeight, minimumSize)
}

export function getAnnotationBounds(annotation: CaptureAnnotation): CaptureRectangle {
  switch (annotation.type) {
    case 'rectangle':
    case 'ellipse':
    case 'highlight':
    case 'mosaic':
      return annotation.rect
    case 'arrow':
    case 'line':
      return padRectangle(normalizeCaptureRectangle(annotation.start, annotation.end), annotation.strokeWidth)
    case 'pen': {
      const xs = annotation.points.map((point) => point.x)
      const ys = annotation.points.map((point) => point.y)
      if (xs.length === 0) return { x: 0, y: 0, width: 0, height: 0 }
      return padRectangle(
        {
          x: Math.min(...xs),
          y: Math.min(...ys),
          width: Math.max(...xs) - Math.min(...xs),
          height: Math.max(...ys) - Math.min(...ys),
        },
        annotation.strokeWidth,
      )
    }
    case 'text': {
      const lines = annotation.text.split('\n')
      const longest = Math.max(1, ...lines.map((line) => Array.from(line).length))
      return {
        x: annotation.point.x,
        y: annotation.point.y,
        width: Math.max(annotation.fontSize, longest * annotation.fontSize * 0.62),
        height: Math.max(annotation.fontSize, lines.length * annotation.fontSize * 1.25),
      }
    }
    case 'counter':
      return {
        x: annotation.point.x - annotation.radius,
        y: annotation.point.y - annotation.radius,
        width: annotation.radius * 2,
        height: annotation.radius * 2,
      }
  }
}

export function hitTestAnnotation(
  annotation: CaptureAnnotation,
  point: CapturePoint,
  tolerance = 8,
): boolean {
  switch (annotation.type) {
    case 'arrow':
    case 'line':
      return distanceToSegment(point, annotation.start, annotation.end) <= tolerance + annotation.strokeWidth
    case 'pen':
      return annotation.points.some((current, index) => {
        const next = annotation.points[index + 1]
        return next
          ? distanceToSegment(point, current, next) <= tolerance + annotation.strokeWidth
          : distance(point, current) <= tolerance + annotation.strokeWidth
      })
    case 'ellipse': {
      const rx = Math.max(1, annotation.rect.width / 2)
      const ry = Math.max(1, annotation.rect.height / 2)
      const cx = annotation.rect.x + rx
      const cy = annotation.rect.y + ry
      const normalized = Math.sqrt(((point.x - cx) / rx) ** 2 + ((point.y - cy) / ry) ** 2)
      const normalizedTolerance = tolerance / Math.max(rx, ry)
      return normalized <= 1 + normalizedTolerance && normalized >= 1 - normalizedTolerance * 2
    }
    case 'rectangle': {
      const outer = padRectangle(annotation.rect, tolerance)
      const inner = padRectangle(annotation.rect, -tolerance - annotation.strokeWidth)
      return pointInRectangle(point, outer) && !pointInRectangle(point, inner)
    }
    case 'highlight':
    case 'mosaic':
    case 'text':
    case 'counter':
      return pointInRectangle(point, padRectangle(getAnnotationBounds(annotation), tolerance))
  }
}

export function translateAnnotation(
  annotation: CaptureAnnotation,
  delta: CapturePoint,
): CaptureAnnotation {
  switch (annotation.type) {
    case 'rectangle':
    case 'ellipse':
    case 'highlight':
    case 'mosaic':
      return {
        ...annotation,
        rect: { ...annotation.rect, x: annotation.rect.x + delta.x, y: annotation.rect.y + delta.y },
      }
    case 'arrow':
    case 'line':
      return {
        ...annotation,
        start: addPoints(annotation.start, delta),
        end: addPoints(annotation.end, delta),
      }
    case 'pen':
      return { ...annotation, points: annotation.points.map((point) => addPoints(point, delta)) }
    case 'text':
    case 'counter':
      return { ...annotation, point: addPoints(annotation.point, delta) }
  }
}

export function resizeAnnotation(
  annotation: CaptureAnnotation,
  originalBounds: CaptureRectangle,
  nextBounds: CaptureRectangle,
): CaptureAnnotation {
  const mapPoint = (point: CapturePoint): CapturePoint => ({
    x:
      nextBounds.x +
      ((point.x - originalBounds.x) / Math.max(1, originalBounds.width)) * nextBounds.width,
    y:
      nextBounds.y +
      ((point.y - originalBounds.y) / Math.max(1, originalBounds.height)) * nextBounds.height,
  })
  const scale = Math.max(
    0.25,
    ((nextBounds.width / Math.max(1, originalBounds.width)) +
      nextBounds.height / Math.max(1, originalBounds.height)) /
      2,
  )

  switch (annotation.type) {
    case 'rectangle':
    case 'ellipse':
    case 'highlight':
    case 'mosaic':
      return { ...annotation, rect: nextBounds }
    case 'arrow':
    case 'line':
      return { ...annotation, start: mapPoint(annotation.start), end: mapPoint(annotation.end) }
    case 'pen':
      return { ...annotation, points: annotation.points.map(mapPoint) }
    case 'text':
      return {
        ...annotation,
        point: mapPoint(annotation.point),
        fontSize: Math.max(10, annotation.fontSize * scale),
      }
    case 'counter':
      return {
        ...annotation,
        point: mapPoint(annotation.point),
        radius: Math.max(8, annotation.radius * scale),
      }
  }
}

export function pointInRectangle(point: CapturePoint, rectangle: CaptureRectangle): boolean {
  return (
    point.x >= rectangle.x &&
    point.x <= rectangle.x + rectangle.width &&
    point.y >= rectangle.y &&
    point.y <= rectangle.y + rectangle.height
  )
}

export function distance(first: CapturePoint, second: CapturePoint): number {
  return Math.hypot(first.x - second.x, first.y - second.y)
}

function addPoints(first: CapturePoint, second: CapturePoint): CapturePoint {
  return { x: first.x + second.x, y: first.y + second.y }
}

function padRectangle(rectangle: CaptureRectangle, amount: number): CaptureRectangle {
  const width = Math.max(0, rectangle.width + amount * 2)
  const height = Math.max(0, rectangle.height + amount * 2)
  return {
    x: rectangle.x - amount,
    y: rectangle.y - amount,
    width,
    height,
  }
}

function distanceToSegment(
  point: CapturePoint,
  start: CapturePoint,
  end: CapturePoint,
): number {
  const dx = end.x - start.x
  const dy = end.y - start.y
  const lengthSquared = dx * dx + dy * dy
  if (lengthSquared === 0) return distance(point, start)
  const ratio = clamp(((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared, 0, 1)
  return distance(point, { x: start.x + ratio * dx, y: start.y + ratio * dy })
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}
