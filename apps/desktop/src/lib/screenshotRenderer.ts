import type {
  CaptureAnnotation,
  CaptureRectangle,
  ShapeAnnotation,
} from './screenCapture'

export function renderCaptureAnnotations(
  context: CanvasRenderingContext2D,
  annotations: CaptureAnnotation[],
  sourceImage: CanvasImageSource,
  clear = true,
) {
  if (clear) context.clearRect(0, 0, context.canvas.width, context.canvas.height)
  context.save()
  context.lineCap = 'round'
  context.lineJoin = 'round'

  for (const annotation of annotations) {
    context.save()
    context.strokeStyle = annotation.color
    context.fillStyle = annotation.color
    context.lineWidth = annotation.strokeWidth

    switch (annotation.type) {
      case 'rectangle':
        context.strokeRect(
          annotation.rect.x,
          annotation.rect.y,
          annotation.rect.width,
          annotation.rect.height,
        )
        break
      case 'ellipse':
        context.beginPath()
        context.ellipse(
          annotation.rect.x + annotation.rect.width / 2,
          annotation.rect.y + annotation.rect.height / 2,
          Math.max(1, annotation.rect.width / 2),
          Math.max(1, annotation.rect.height / 2),
          0,
          0,
          Math.PI * 2,
        )
        context.stroke()
        break
      case 'highlight':
        context.globalAlpha = 0.32
        context.fillRect(
          annotation.rect.x,
          annotation.rect.y,
          annotation.rect.width,
          annotation.rect.height,
        )
        break
      case 'mosaic':
        renderMosaic(context, annotation, sourceImage)
        break
      case 'line':
        drawLine(context, annotation.start.x, annotation.start.y, annotation.end.x, annotation.end.y)
        break
      case 'arrow':
        drawArrow(
          context,
          annotation.start.x,
          annotation.start.y,
          annotation.end.x,
          annotation.end.y,
          annotation.strokeWidth,
        )
        break
      case 'pen':
        if (annotation.points.length === 1) {
          context.beginPath()
          context.arc(
            annotation.points[0]!.x,
            annotation.points[0]!.y,
            annotation.strokeWidth / 2,
            0,
            Math.PI * 2,
          )
          context.fill()
        } else if (annotation.points.length > 1) {
          context.beginPath()
          context.moveTo(annotation.points[0]!.x, annotation.points[0]!.y)
          for (const point of annotation.points.slice(1)) context.lineTo(point.x, point.y)
          context.stroke()
        }
        break
      case 'text': {
        context.font = `600 ${annotation.fontSize}px ui-sans-serif, system-ui, sans-serif`
        context.textBaseline = 'top'
        annotation.text.split('\n').forEach((line, index) => {
          context.fillText(line, annotation.point.x, annotation.point.y + index * annotation.fontSize * 1.25)
        })
        break
      }
      case 'counter': {
        context.beginPath()
        context.arc(annotation.point.x, annotation.point.y, annotation.radius, 0, Math.PI * 2)
        context.fill()
        context.fillStyle = '#ffffff'
        context.font = `700 ${annotation.radius * 1.15}px ui-sans-serif, system-ui, sans-serif`
        context.textAlign = 'center'
        context.textBaseline = 'middle'
        context.fillText(String(annotation.number), annotation.point.x, annotation.point.y + 0.5)
        break
      }
    }
    context.restore()
  }

  context.restore()
}

export function exportAnnotatedCapture(
  sourceImage: CanvasImageSource,
  selection: CaptureRectangle,
  annotations: CaptureAnnotation[],
  canvasError = 'Could not create capture export canvas',
): string {
  const output = document.createElement('canvas')
  output.width = Math.max(1, Math.round(selection.width))
  output.height = Math.max(1, Math.round(selection.height))
  const context = output.getContext('2d')
  if (!context) throw new Error(canvasError)

  context.drawImage(
    sourceImage,
    selection.x,
    selection.y,
    selection.width,
    selection.height,
    0,
    0,
    output.width,
    output.height,
  )
  context.save()
  context.translate(-selection.x, -selection.y)
  renderCaptureAnnotations(context, annotations, sourceImage, false)
  context.restore()
  return output.toDataURL('image/png')
}

function drawLine(
  context: CanvasRenderingContext2D,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
) {
  context.beginPath()
  context.moveTo(startX, startY)
  context.lineTo(endX, endY)
  context.stroke()
}

function drawArrow(
  context: CanvasRenderingContext2D,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  strokeWidth: number,
) {
  drawLine(context, startX, startY, endX, endY)
  const angle = Math.atan2(endY - startY, endX - startX)
  const headLength = Math.max(12, strokeWidth * 4.5)
  context.beginPath()
  context.moveTo(endX, endY)
  context.lineTo(
    endX - headLength * Math.cos(angle - Math.PI / 6),
    endY - headLength * Math.sin(angle - Math.PI / 6),
  )
  context.moveTo(endX, endY)
  context.lineTo(
    endX - headLength * Math.cos(angle + Math.PI / 6),
    endY - headLength * Math.sin(angle + Math.PI / 6),
  )
  context.stroke()
}

function renderMosaic(
  context: CanvasRenderingContext2D,
  annotation: ShapeAnnotation,
  sourceImage: CanvasImageSource,
) {
  const rectangle = annotation.rect
  const width = Math.max(1, Math.round(rectangle.width))
  const height = Math.max(1, Math.round(rectangle.height))
  if (width < 1 || height < 1) return

  const pixelSize = Math.max(4, Math.round(annotation.pixelSize ?? 12))
  const sample = document.createElement('canvas')
  sample.width = Math.max(1, Math.ceil(width / pixelSize))
  sample.height = Math.max(1, Math.ceil(height / pixelSize))
  const sampleContext = sample.getContext('2d')
  if (!sampleContext) return
  sampleContext.imageSmoothingEnabled = true
  sampleContext.drawImage(
    sourceImage,
    rectangle.x,
    rectangle.y,
    rectangle.width,
    rectangle.height,
    0,
    0,
    sample.width,
    sample.height,
  )
  context.imageSmoothingEnabled = false
  context.drawImage(sample, rectangle.x, rectangle.y, rectangle.width, rectangle.height)
  context.imageSmoothingEnabled = true
}
