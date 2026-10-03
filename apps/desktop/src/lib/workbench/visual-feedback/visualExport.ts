import type { VisualFeedbackAnnotation, VisualFeedbackData } from '../../generated/feedback'
import type { AttachmentView } from '../../feedback'
import { arrowHead, validVisualAnnotations } from './visualModel'
import { visualAttachmentText } from './visualAttachmentText'

export function loadVisualImage(blob: Blob): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image(), url = URL.createObjectURL(blob)
    image.onload = () => { URL.revokeObjectURL(url); resolve(image) }
    image.onerror = () => { URL.revokeObjectURL(url); reject(new Error('The original image could not be decoded.')) }
    image.src = url
  })
}
export function drawVisualAnnotations(ctx: CanvasRenderingContext2D, annotations: readonly VisualFeedbackAnnotation[], attachments: readonly Pick<AttachmentView, 'attachment_id'>[] = []) {
  for (const mark of annotations) {
    ctx.strokeStyle = mark.color; ctx.fillStyle = mark.color; ctx.lineWidth = mark.stroke_width
    ctx.lineCap = 'round'; ctx.lineJoin = 'round'
    const [a, b] = mark.points
    if (mark.kind === 'text') {
      ctx.font = `${mark.stroke_width}px Arial, sans-serif`; ctx.textBaseline = 'top'
      visualAttachmentText(mark.text, attachments).split('\n').forEach((line, index) => ctx.fillText(line, a.x, a.y + index * mark.stroke_width * 1.25))
    } else if (mark.kind === 'rectangle') {
      ctx.strokeRect(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.abs(b.x - a.x), Math.abs(b.y - a.y))
    } else {
      ctx.beginPath(); ctx.moveTo(a.x, a.y)
      for (const point of mark.points.slice(1)) ctx.lineTo(point.x, point.y)
      ctx.stroke()
      if (mark.kind === 'arrow') {
        const head = arrowHead(a, b, mark.stroke_width)
        ctx.beginPath(); ctx.moveTo(head[0].x, head[0].y); ctx.lineTo(head[1].x, head[1].y); ctx.lineTo(head[2].x, head[2].y); ctx.stroke()
      }
    }
  }
}
export async function exportVisualPng(data: VisualFeedbackData, annotations: VisualFeedbackAnnotation[], background: Blob | null, attachments: readonly Pick<AttachmentView, 'attachment_id'>[] = []): Promise<ArrayBuffer> {
  if (!validVisualAnnotations(annotations, data.width, data.height)) throw new Error('The visual annotations are invalid.')
  if (data.image_file_name !== null && !background) throw new Error('The original image is unavailable.')
  const canvas = document.createElement('canvas'); canvas.width = data.width; canvas.height = data.height
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('The canvas is unavailable.')
  ctx.fillStyle = data.background_color ?? '#ffffff'; ctx.fillRect(0, 0, data.width, data.height)
  if (background) {
    const image = await loadVisualImage(background)
    if (image.naturalWidth !== data.width || image.naturalHeight !== data.height) throw new Error('The original image dimensions do not match the canvas.')
    ctx.drawImage(image, 0, 0)
  }
  drawVisualAnnotations(ctx, annotations, attachments)
  const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((value) => value ? resolve(value) : reject(new Error('The PNG could not be created.')), 'image/png'))
  if (blob.size > 20 * 1024 * 1024) throw new Error('The composed PNG exceeds 20 MiB. Your draft remains editable. Ask the agent to create a new request with a smaller image or canvas.')
  return blob.arrayBuffer()
}
