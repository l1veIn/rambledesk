import type { VisualFeedbackAnnotation, VisualFeedbackData, WorkbenchState } from '../../generated/feedback'
import { fields, list, object, text, uniqueId } from '../definitions/validation'

export type VisualState = Extract<WorkbenchState, { type: 'visual_feedback' }>
export type VisualPoint = VisualFeedbackAnnotation['points'][number]
export type VisualTool = 'select' | VisualFeedbackAnnotation['kind']
export const emptyVisualState = (): VisualState => ({ type: 'visual_feedback', annotations: [], composite_attachment_id: null })
export const annotationsKey = (annotations: readonly VisualFeedbackAnnotation[]) => JSON.stringify(annotations)

export function validVisualAnnotations(value: unknown, width = 4096, height = 4096): value is VisualFeedbackAnnotation[] {
  if (!list(value, 0, 500)) return false
  const seen = new Set<string>()
  let count = 0
  for (const mark of value) {
    if (!object(mark) || !fields(mark, ['id', 'kind', 'points', 'color', 'stroke_width', 'text', 'body'])
      || !uniqueId(mark.id, seen) || !['freehand', 'arrow', 'rectangle', 'text'].includes(String(mark.kind))
      || typeof mark.color !== 'string' || !/^#[0-9a-f]{6}$/i.test(mark.color)
      || !Number.isInteger(mark.stroke_width) || Number(mark.stroke_width) < 1 || Number(mark.stroke_width) > 32
      || !text(mark.text, 2000, false) || (mark.kind !== 'text' && mark.text !== '')
      || !text(mark.body, 4000, false)) return false
    const min = mark.kind === 'text' ? 1 : 2, max = mark.kind === 'freehand' ? 2048 : min
    if (!list(mark.points, min, max)) return false
    count += mark.points.length
    if (count > 20000 || !mark.points.every((point) => object(point) && fields(point, ['x', 'y'])
      && Number.isInteger(point.x) && Number(point.x) >= 0 && Number(point.x) <= width
      && Number.isInteger(point.y) && Number(point.y) >= 0 && Number(point.y) <= height)) return false
    const [a, b] = mark.points as VisualPoint[]
    if (mark.kind === 'arrow' && a.x === b.x && a.y === b.y) return false
    if (mark.kind === 'rectangle' && (a.x === b.x || a.y === b.y)) return false
  }
  return true
}
export function visualFeedbackState(value: unknown): VisualState | null {
  if (!object(value) || !fields(value, ['type', 'annotations', 'composite_attachment_id'])
    || value.type !== 'visual_feedback' || !validVisualAnnotations(value.annotations)
    || !(value.composite_attachment_id === null || uniqueId(value.composite_attachment_id, new Set()))) return null
  return { type: 'visual_feedback', annotations: value.annotations, composite_attachment_id: value.composite_attachment_id }
}
export function validVisualState(data: VisualFeedbackData, state: VisualState) {
  return validVisualAnnotations(state.annotations, data.width, data.height)
}
export function canvasPoint(clientX: number, clientY: number, rect: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>, data: Pick<VisualFeedbackData, 'width' | 'height'>): VisualPoint {
  return { x: Math.round(Math.max(0, Math.min(data.width, (clientX - rect.left) * data.width / Math.max(1, rect.width)))),
    y: Math.round(Math.max(0, Math.min(data.height, (clientY - rect.top) * data.height / Math.max(1, rect.height)))) }
}
export function arrowHead(a: VisualPoint, b: VisualPoint, strokeWidth: number): VisualPoint[] {
  const angle = Math.atan2(b.y - a.y, b.x - a.x), size = Math.max(10, strokeWidth * 3)
  return [{ x: b.x - size * Math.cos(angle - Math.PI / 6), y: b.y - size * Math.sin(angle - Math.PI / 6) }, b,
    { x: b.x - size * Math.cos(angle + Math.PI / 6), y: b.y - size * Math.sin(angle + Math.PI / 6) }]
}
export const pointsPath = (points: readonly VisualPoint[]) => points.map((point) => `${point.x},${point.y}`).join(' ')
export function drawable(mark: VisualFeedbackAnnotation) {
  if (mark.kind === 'rectangle') return mark.points[0].x !== mark.points[1].x && mark.points[0].y !== mark.points[1].y
  return mark.kind === 'text' ? text(mark.text, 2000) : mark.points.some((point) => point.x !== mark.points[0].x || point.y !== mark.points[0].y)
}

/** Only committed edits enter undo history. Recovered drafts remain authoritative. */
export function createVisualHistory(initial: VisualFeedbackAnnotation[] = []) {
  let current = initial, undo: VisualFeedbackAnnotation[][] = [], redo: VisualFeedbackAnnotation[][] = []
  return {
    commit(next: VisualFeedbackAnnotation[]) {
      if (annotationsKey(next) === annotationsKey(current)) return false
      undo = [...undo.slice(-49), current]; redo = []; current = next; return true
    },
    recover(next: VisualFeedbackAnnotation[]) { if (annotationsKey(next) !== annotationsKey(current)) { current = next; undo = []; redo = [] } },
    undo() { const next = undo.pop(); if (!next) return null; redo.push(current); current = next; return next },
    redo() { const next = redo.pop(); if (!next) return null; undo.push(current); current = next; return next },
    get canUndo() { return undo.length > 0 }, get canRedo() { return redo.length > 0 },
  }
}
