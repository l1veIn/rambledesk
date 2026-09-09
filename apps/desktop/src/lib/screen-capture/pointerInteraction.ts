import {
  clampCaptureRectangle,
  distance,
  getAnnotationBounds,
  hitTestAnnotation,
  normalizeCaptureRectangle,
  pointInRectangle,
  resizeAnnotation,
  resizeCaptureRectangle,
  translateAnnotation,
  type AnnotationTool,
  type CaptureAnnotation,
  type CapturePoint,
  type CaptureRectangle,
  type CaptureTarget,
  type ResizeHandle,
} from './screenCapture'
import {
  annotationHasSize,
  captureTargetRectangle,
  clampPointToSelection,
  findCaptureTarget,
  sourceTolerance,
  type OverlayGeometry,
} from './overlayGeometry'
import {
  cloneAnnotations,
  counterAnnotation,
  createDraftAnnotation,
  nextCounterNumber,
  type AnnotationStyle,
} from './annotationModel'

export type TextDraft = {
  point: CapturePoint
  value: string
}

type GestureKind =
  | 'new-selection'
  | 'move-selection'
  | 'resize-selection'
  | 'draw'
  | 'move-annotation'
  | 'resize-annotation'

type Gesture = {
  kind: GestureKind
  start: CapturePoint
  handle?: ResizeHandle
  target?: CaptureTarget
  originalSelection?: CaptureRectangle
  originalAnnotation?: CaptureAnnotation
  annotationsSnapshot?: CaptureAnnotation[]
}

export type PointerInteractionState = Readonly<{
  capture: { image_width: number; image_height: number; targets?: CaptureTarget[] } | null
  completing: boolean
  selection: CaptureRectangle | null
  hoveredTarget: CaptureTarget | null
  activeTool: AnnotationTool
  annotations: readonly CaptureAnnotation[]
  draftAnnotation: CaptureAnnotation | null
  selectedAnnotationId: string | null
  textDraft: TextDraft | null
  style: AnnotationStyle
}>

export type PointerInteractionPatch = Partial<{
  selection: CaptureRectangle | null
  hoveredTarget: CaptureTarget | null
  selectedAnnotationId: string | null
  draftAnnotation: CaptureAnnotation | null
  textDraft: TextDraft | null
  /** Live gesture updates that must not push an undo entry. */
  annotations: CaptureAnnotation[]
}>

export type PointerInteractionContext = {
  getGeometry: () => OverlayGeometry
  toImagePoint: (event: PointerEvent) => CapturePoint | null
  getState: () => PointerInteractionState
  patch: (patch: PointerInteractionPatch) => void
  /** Commits a new annotation list and pushes an undo entry. */
  commit: (next: CaptureAnnotation[]) => void
  pushUndoSnapshot: (snapshot: CaptureAnnotation[]) => void
  closePanels: () => void
  clearError: () => void
  focusTextInput: () => void
  scheduleToolbarLayout: () => void
}

/**
 * Pointer gesture state machine for the capture overlay: selecting a region,
 * moving/resizing it, and drawing, moving, or resizing annotations. The
 * component only forwards pointer events and applies the patches.
 */
export function createPointerInteraction(context: PointerInteractionContext) {
  let gesture: Gesture | null = null

  function begin(event: PointerEvent) {
    const state = context.getState()
    const geometry = context.getGeometry()
    if (!state.capture || state.completing || event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('[data-capture-ui]')) return
    context.closePanels()
    event.preventDefault()
    window.getSelection()?.removeAllRanges()
    const point = context.toImagePoint(event)
    if (!point) return
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    context.clearError()

    if (!state.selection) {
      gesture = { kind: 'new-selection', start: point, target: state.hoveredTarget ?? undefined }
      return
    }

    if (state.activeTool === 'select') {
      const tolerance = sourceTolerance(8, geometry)
      const hit = [...state.annotations]
        .reverse()
        .find((annotation) => hitTestAnnotation(annotation, point, tolerance))
      if (hit) {
        context.patch({ selectedAnnotationId: hit.id })
        gesture = {
          kind: 'move-annotation',
          start: point,
          originalAnnotation: hit,
          annotationsSnapshot: cloneAnnotations(state.annotations),
        }
        return
      }
      context.patch({ selectedAnnotationId: null })
      if (pointInRectangle(point, state.selection)) {
        gesture = {
          kind: 'move-selection',
          start: point,
          originalSelection: { ...state.selection },
        }
      }
      return
    }

    if (!pointInRectangle(point, state.selection)) return
    const clamped = clampPointToSelection(point, state.selection)
    if (state.activeTool === 'text') {
      event.preventDefault()
      context.patch({ textDraft: { point: clamped, value: '' } })
      context.focusTextInput()
      return
    }
    if (state.activeTool === 'counter') {
      context.commit([
        ...state.annotations,
        counterAnnotation(clamped, nextCounterNumber(state.annotations), state.style),
      ])
      return
    }
    gesture = { kind: 'draw', start: clamped }
    context.patch({
      draftAnnotation: createDraftAnnotation(
        state.activeTool,
        clamped,
        clamped,
        state.style,
      ),
    })
  }

  function move(event: PointerEvent) {
    const state = context.getState()
    const geometry = context.getGeometry()
    const point = context.toImagePoint(event)
    if (!point || !state.capture) {
      if (!gesture && !state.selection) context.patch({ hoveredTarget: null })
      return
    }
    if (!gesture) {
      if (!state.selection) {
        context.patch({ hoveredTarget: findCaptureTarget(point, state.capture.targets ?? []) })
      }
      return
    }

    switch (gesture.kind) {
      case 'new-selection':
        if (distance(gesture.start, point) > sourceTolerance(5, geometry)) {
          context.patch({
            hoveredTarget: null,
            selection: clampCaptureRectangle(
              normalizeCaptureRectangle(gesture.start, point),
              state.capture.image_width,
              state.capture.image_height,
            ),
          })
        }
        break
      case 'move-selection':
        if (gesture.originalSelection) {
          context.patch({
            selection: clampCaptureRectangle(
              {
                ...gesture.originalSelection,
                x: gesture.originalSelection.x + point.x - gesture.start.x,
                y: gesture.originalSelection.y + point.y - gesture.start.y,
              },
              state.capture.image_width,
              state.capture.image_height,
            ),
          })
        }
        break
      case 'resize-selection':
        if (gesture.originalSelection && gesture.handle) {
          context.patch({
            selection: resizeCaptureRectangle(
              gesture.originalSelection,
              gesture.handle,
              point,
              state.capture.image_width,
              state.capture.image_height,
              sourceTolerance(12, geometry),
            ),
          })
        }
        break
      case 'draw': {
        const clamped = state.selection
          ? clampPointToSelection(point, state.selection)
          : point
        if (state.draftAnnotation?.type === 'pen') {
          const last = state.draftAnnotation.points.at(-1)
          if (!last || distance(last, clamped) >= sourceTolerance(1.5, geometry)) {
            context.patch({
              draftAnnotation: {
                ...state.draftAnnotation,
                points: [...state.draftAnnotation.points, clamped],
              },
            })
          }
        } else {
          context.patch({
            draftAnnotation: createDraftAnnotation(
              state.activeTool,
              gesture.start,
              clamped,
              state.style,
            ),
          })
        }
        break
      }
      case 'move-annotation':
        if (gesture.originalAnnotation && gesture.annotationsSnapshot) {
          const moved = translateAnnotation(gesture.originalAnnotation, {
            x: point.x - gesture.start.x,
            y: point.y - gesture.start.y,
          })
          context.patch({
            annotations: gesture.annotationsSnapshot.map((annotation) =>
              annotation.id === moved.id ? moved : annotation,
            ),
          })
        }
        break
      case 'resize-annotation':
        if (gesture.originalAnnotation && gesture.annotationsSnapshot && gesture.handle) {
          const originalBounds = getAnnotationBounds(gesture.originalAnnotation)
          const nextBounds = resizeCaptureRectangle(
            originalBounds,
            gesture.handle,
            point,
            state.capture.image_width,
            state.capture.image_height,
            sourceTolerance(8, geometry),
          )
          const resized = resizeAnnotation(gesture.originalAnnotation, originalBounds, nextBounds)
          context.patch({
            annotations: gesture.annotationsSnapshot.map((annotation) =>
              annotation.id === resized.id ? resized : annotation,
            ),
          })
        }
        break
    }
  }

  function end(event: PointerEvent) {
    const state = context.getState()
    const geometry = context.getGeometry()
    if (!gesture || !state.capture) return
    const point = context.toImagePoint(event) ?? gesture.start
    const completedGesture = gesture
    gesture = null

    if (completedGesture.kind === 'new-selection') {
      if (
        distance(completedGesture.start, point) <= sourceTolerance(5, geometry) &&
        completedGesture.target
      ) {
        context.patch({ selection: captureTargetRectangle(completedGesture.target) })
      } else if (
        state.selection &&
        (state.selection.width < sourceTolerance(6, geometry) ||
          state.selection.height < sourceTolerance(6, geometry))
      ) {
        context.patch({ selection: null })
      }
      context.patch({ hoveredTarget: null })
      if (context.getState().selection) context.scheduleToolbarLayout()
      return
    }
    if (completedGesture.kind === 'draw') {
      const draft = state.draftAnnotation
      context.patch({ draftAnnotation: null })
      if (
        draft &&
        annotationHasSize(draft, getAnnotationBounds(draft), sourceTolerance(3, geometry))
      ) {
        context.commit([...state.annotations, draft])
      }
      return
    }
    if (
      (completedGesture.kind === 'move-annotation' ||
        completedGesture.kind === 'resize-annotation') &&
      completedGesture.annotationsSnapshot &&
      JSON.stringify(completedGesture.annotationsSnapshot) !== JSON.stringify(state.annotations)
    ) {
      context.pushUndoSnapshot(completedGesture.annotationsSnapshot)
    }
  }

  function beginSelectionResize(event: PointerEvent, handle: ResizeHandle) {
    const state = context.getState()
    if (!state.selection || !state.capture || state.completing) return
    event.stopPropagation()
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    const point = context.toImagePoint(event)
    if (!point) return
    gesture = {
      kind: 'resize-selection',
      start: point,
      handle,
      originalSelection: { ...state.selection },
    }
  }

  function beginAnnotationResize(event: PointerEvent, handle: ResizeHandle) {
    const state = context.getState()
    const selected = state.selectedAnnotationId
      ? state.annotations.find((annotation) => annotation.id === state.selectedAnnotationId) ?? null
      : null
    if (!selected || !state.capture || state.completing) return
    event.stopPropagation()
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    const point = context.toImagePoint(event)
    if (!point) return
    gesture = {
      kind: 'resize-annotation',
      start: point,
      handle,
      originalAnnotation: structuredClone(selected),
      annotationsSnapshot: cloneAnnotations(state.annotations),
    }
  }

  function reset() {
    gesture = null
  }

  return { begin, move, end, beginSelectionResize, beginAnnotationResize, reset }
}

export type PointerInteraction = ReturnType<typeof createPointerInteraction>
