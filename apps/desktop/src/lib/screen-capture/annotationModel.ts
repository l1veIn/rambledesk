import {
  getAnnotationBounds,
  normalizeCaptureRectangle,
  type AnnotationTool,
  type CaptureAnnotation,
  type CapturePoint,
} from '../screenCapture'
import { annotationHasSize } from './overlayGeometry'

export type AnnotationStyle = Readonly<{ color: string; strokeWidth: number }>

/** Annotation list plus the undo/redo snapshots around it. */
export type AnnotationHistory = Readonly<{
  annotations: CaptureAnnotation[]
  undoStack: CaptureAnnotation[][]
  redoStack: CaptureAnnotation[][]
}>

export const emptyAnnotationHistory: AnnotationHistory = {
  annotations: [],
  undoStack: [],
  redoStack: [],
}

export function cloneAnnotations(annotations: readonly CaptureAnnotation[]) {
  return structuredClone(annotations) as CaptureAnnotation[]
}

export function newAnnotationId() {
  return crypto.randomUUID()
}

export function nextCounterNumber(annotations: readonly CaptureAnnotation[]) {
  return (
    Math.max(
      0,
      ...annotations
        .filter((annotation) => annotation.type === 'counter')
        .map((annotation) => (annotation.type === 'counter' ? annotation.number : 0)),
    ) + 1
  )
}

export function counterAnnotation(
  point: CapturePoint,
  number: number,
  style: AnnotationStyle,
  id: string = newAnnotationId(),
): CaptureAnnotation {
  return {
    id,
    type: 'counter',
    point,
    number,
    radius: Math.max(12, style.strokeWidth * 3.5),
    color: style.color,
    strokeWidth: style.strokeWidth,
  }
}

export function textAnnotation(
  point: CapturePoint,
  text: string,
  style: AnnotationStyle,
  id: string = newAnnotationId(),
): CaptureAnnotation {
  return {
    id,
    type: 'text',
    point,
    text,
    fontSize: Math.max(18, style.strokeWidth * 5),
    color: style.color,
    strokeWidth: style.strokeWidth,
  }
}

export function createDraftAnnotation(
  tool: AnnotationTool,
  start: CapturePoint,
  end: CapturePoint,
  style: AnnotationStyle,
  id: string = newAnnotationId(),
): CaptureAnnotation | null {
  const base = { id, color: style.color, strokeWidth: style.strokeWidth }
  if (tool === 'arrow' || tool === 'line') return { ...base, type: tool, start, end }
  if (tool === 'pen') return { ...base, type: 'pen', points: [start, end] }
  if (tool === 'rectangle' || tool === 'ellipse' || tool === 'highlight' || tool === 'mosaic') {
    return {
      ...base,
      type: tool,
      rect: normalizeCaptureRectangle(start, end),
      ...(tool === 'mosaic' ? { pixelSize: Math.max(8, style.strokeWidth * 3) } : {}),
    }
  }
  return null
}

/** A draft only becomes an annotation when it has a visible size. */
export function shouldCommitDraft(draft: CaptureAnnotation | null, minSize: number) {
  return Boolean(draft && annotationHasSize(draft, getAnnotationBounds(draft), minSize))
}

/** Commits a new annotation list and pushes the previous one onto the undo stack. */
export function commitAnnotations(
  history: AnnotationHistory,
  next: CaptureAnnotation[],
): AnnotationHistory {
  return {
    annotations: next,
    undoStack: [...history.undoStack, cloneAnnotations(history.annotations)],
    redoStack: [],
  }
}

/** Live gesture updates: replaces the list without touching the history. */
export function replaceAnnotations(
  history: AnnotationHistory,
  next: CaptureAnnotation[],
): AnnotationHistory {
  return { ...history, annotations: next }
}

export function pushUndoSnapshot(
  history: AnnotationHistory,
  snapshot: CaptureAnnotation[],
): AnnotationHistory {
  return { ...history, undoStack: [...history.undoStack, snapshot], redoStack: [] }
}

export function undoAnnotations(history: AnnotationHistory): AnnotationHistory {
  const previous = history.undoStack.at(-1)
  if (!previous) return history
  return {
    annotations: previous,
    undoStack: history.undoStack.slice(0, -1),
    redoStack: [cloneAnnotations(history.annotations), ...history.redoStack],
  }
}

export function redoAnnotations(history: AnnotationHistory): AnnotationHistory {
  const next = history.redoStack[0]
  if (!next) return history
  return {
    annotations: next,
    undoStack: [...history.undoStack, cloneAnnotations(history.annotations)],
    redoStack: history.redoStack.slice(1),
  }
}

export function deleteAnnotation(
  history: AnnotationHistory,
  annotationId: string,
): AnnotationHistory {
  return commitAnnotations(
    history,
    history.annotations.filter((annotation) => annotation.id !== annotationId),
  )
}

export function updateAnnotationAppearance(
  history: AnnotationHistory,
  annotationId: string,
  patch: { color?: string; strokeWidth?: number },
): AnnotationHistory {
  return commitAnnotations(
    history,
    history.annotations.map((annotation) =>
      annotation.id === annotationId
        ? ({ ...annotation, ...patch } as CaptureAnnotation)
        : annotation,
    ),
  )
}
