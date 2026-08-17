<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'

  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'
  import CaptureToolbar from './lib/screen-capture/CaptureToolbar.svelte'
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
    type ScreenCaptureView,
  } from './lib/screenCapture'
  import { exportAnnotatedCapture, renderCaptureAnnotations } from './lib/screenshotRenderer'
  import {
    annotationHasSize,
    captureTargetRectangle,
    clampPointToSelection,
    cssRectangle,
    fitImage,
    findCaptureTarget,
    fullScreenSelection,
    imageLayerStyle,
    imagePoint,
    sourceTolerance,
    textDraftStyle as computeTextDraftStyle,
    captureToolbarPosition as computeToolbarPosition,
    toolbarPopoverOpensDownward as toolbarPopoverDown,
  } from './lib/screen-capture/overlayGeometry'

  type DisplayRectangle = CaptureRectangle
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

  type TextDraft = {
    point: CapturePoint
    value: string
  }

  type ToolbarDrag = {
    pointerId: number
    startX: number
    startY: number
    originX: number
    originY: number
  }

  const colors = ['#ff4d5d', '#ffb020', '#37c878', '#3ca7ff', '#ffffff', '#15191f']
  const strokeWidths = [2, 4, 8]

  let shell: HTMLElement
  let sourceCanvas: HTMLCanvasElement
  let annotationCanvas: HTMLCanvasElement
  let textInput: HTMLTextAreaElement
  let capture: ScreenCaptureView | null = null
  let sourceImage: HTMLCanvasElement | null = null
  let sourceReady = false
  let viewportWidth = window.innerWidth
  let viewportHeight = window.innerHeight
  let toolbarWidth = 0
  let toolbarHeight = 0
  let selection: CaptureRectangle | null = null
  let hoveredTarget: CaptureTarget | null = null
  let annotations: CaptureAnnotation[] = []
  let draftAnnotation: CaptureAnnotation | null = null
  let activeTool: AnnotationTool = 'select'
  let selectedAnnotationId: string | null = null
  let currentColor = colors[0]!
  let currentStrokeWidth = 4
  let gesture: Gesture | null = null
  let undoStack: CaptureAnnotation[][] = []
  let redoStack: CaptureAnnotation[][] = []
  let textDraft: TextDraft | null = null
  let loading = true
  let completing = false
  let errorMessage = ''
  let initializingSessionId: string | null = null
  let toolbarManualX: number | null = null
  let toolbarManualY: number | null = null
  let toolbarDrag: ToolbarDrag | null = null
  let toolbarDragListening = false
  let toolbarHost: HTMLDivElement | null = null
  let toolbarStyle = ''
  let stylePanelOpen = false
  let overflowPanelOpen = false

  $: displayRectangle = capture
    ? fitImage(capture.image_width, capture.image_height, viewportWidth, viewportHeight)
    : null
  $: geometry = { capture, displayRectangle, viewportWidth, viewportHeight }
  $: selectedAnnotation = selectedAnnotationId
    ? annotations.find((annotation) => annotation.id === selectedAnnotationId) ?? null
    : null
  $: selectedBounds = selectedAnnotation ? getAnnotationBounds(selectedAnnotation) : null
  $: if (annotationCanvas && sourceImage) {
    const context = annotationCanvas.getContext('2d')
    if (context) {
      renderCaptureAnnotations(
        context,
        draftAnnotation ? [...annotations, draftAnnotation] : annotations,
        sourceImage,
      )
    }
  }

  onMount(() => {
    const resize = () => resizeViewport()
    const keydown = (event: KeyboardEvent) => void handleKeydown(event)
    const preventSelection = (event: Event) => {
      const target = event.target
      if (target instanceof Element && target.closest('textarea, input, [contenteditable="true"]')) return
      event.preventDefault()
    }
    window.addEventListener('resize', resize)
    window.addEventListener('keydown', keydown)
    document.addEventListener('selectstart', preventSelection)
    resize()
    let disposed = false
    let unlisten: UnlistenFn | undefined
    void listen<{ capture_session_id: string }>('screen-capture-session-ready', (event) => {
      void initialize(event.payload.capture_session_id)
    }).then((dispose) => {
      if (disposed) dispose()
      else unlisten = dispose
    })
    void resumeActiveCapture()
    return () => {
      disposed = true
      unlisten?.()
      window.removeEventListener('resize', resize)
      window.removeEventListener('keydown', keydown)
      document.removeEventListener('selectstart', preventSelection)
      unbindToolbarDragListeners()
    }
  })

  async function resumeActiveCapture() {
    try {
      const active = await invoke<ScreenCaptureView>('get_active_capture_info')
      await initialize(active.capture_session_id, active)
    } catch {
      // The prewarmed editor normally has no active session until capture starts.
    }
  }

  function resetEditor() {
    capture = null
    sourceImage = null
    sourceReady = false
    selection = null
    hoveredTarget = null
    annotations = []
    draftAnnotation = null
    activeTool = 'select'
    selectedAnnotationId = null
    gesture = null
    undoStack = []
    redoStack = []
    textDraft = null
    loading = true
    completing = false
    errorMessage = ''
    toolbarManualX = null
    toolbarManualY = null
    toolbarDrag = null
    toolbarStyle = ''
    if (toolbarHost) {
      toolbarHost.style.left = ''
      toolbarHost.style.top = ''
    }
    unbindToolbarDragListeners()
    stylePanelOpen = false
    overflowPanelOpen = false
  }

  async function initialize(captureSessionId: string, active?: ScreenCaptureView) {
    if (initializingSessionId === captureSessionId) return
    initializingSessionId = captureSessionId
    resetEditor()
    try {
      capture = active ?? (await invoke<ScreenCaptureView>('get_active_capture_info'))
      if (capture.capture_session_id !== captureSessionId) {
        throw new Error(t($locale, 'The capture session changed. Please capture again.'))
      }
      const rgba = await invoke<ArrayBuffer>('read_capture_rgba_bytes', {
        captureSessionId: capture.capture_session_id,
      })
      const expectedBytes = capture.image_width * capture.image_height * 4
      if (rgba.byteLength !== expectedBytes) {
        throw new Error(
          t($locale, 'Capture pixel data is incomplete: expected {expected} bytes, received {actual} bytes.', {
            expected: expectedBytes,
            actual: rgba.byteLength,
          }),
        )
      }
      const imageData = new ImageData(
        new Uint8ClampedArray(rgba),
        capture.image_width,
        capture.image_height,
      )
      sourceReady = true
      if (capture.suggested_selection) {
        selection = capture.suggested_selection
      }
      await tick()
      drawSourceImage(imageData)
    } catch (cause) {
      errorMessage = messageFrom(cause)
    } finally {
      loading = false
      await tick()
      await invoke('show_screen_capture_overlay').catch((cause) => {
        errorMessage ||= messageFrom(cause)
      })
      resizeViewport()
      if (selection) await scheduleToolbarLayout()
      initializingSessionId = null
    }
  }

  function drawSourceImage(imageData: ImageData) {
    if (!sourceCanvas) return
    const context = sourceCanvas.getContext('2d')
    if (!context) throw new Error(t($locale, 'Could not create the capture display canvas'))
    context.putImageData(imageData, 0, 0)
    sourceImage = sourceCanvas
  }

  function beginPointer(event: PointerEvent) {
    if (!capture || completing || event.button !== 0 || toolbarDrag) return
    const target = event.target
    if (target instanceof Element && target.closest('[data-capture-ui]')) return
    stylePanelOpen = false
    overflowPanelOpen = false
    event.preventDefault()
    window.getSelection()?.removeAllRanges()
    const point = imagePoint(event, shell.getBoundingClientRect(), geometry)
    if (!point) return
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    errorMessage = ''

    if (!selection) {
      gesture = { kind: 'new-selection', start: point, target: hoveredTarget ?? undefined }
      return
    }

    if (activeTool === 'select') {
      const tolerance = sourceTolerance(8, geometry)
      const hit = [...annotations]
        .reverse()
        .find((annotation) => hitTestAnnotation(annotation, point, tolerance))
      if (hit) {
        selectedAnnotationId = hit.id
        gesture = {
          kind: 'move-annotation',
          start: point,
          originalAnnotation: hit,
          annotationsSnapshot: cloneAnnotations(annotations),
        }
        return
      }
      selectedAnnotationId = null
      if (pointInRectangle(point, selection)) {
        gesture = {
          kind: 'move-selection',
          start: point,
          originalSelection: { ...selection },
        }
      }
      return
    }

    if (!pointInRectangle(point, selection)) return
    const clamped = clampPointToSelection(point, selection)
    if (activeTool === 'text') {
      event.preventDefault()
      textDraft = { point: clamped, value: '' }
      void tick().then(() => textInput?.focus())
      return
    }
    if (activeTool === 'counter') {
      const nextNumber =
        Math.max(
          0,
          ...annotations
            .filter((annotation) => annotation.type === 'counter')
            .map((annotation) => (annotation.type === 'counter' ? annotation.number : 0)),
        ) + 1
      commitAnnotations([
        ...annotations,
        {
          id: newId(),
          type: 'counter',
          point: clamped,
          number: nextNumber,
          radius: Math.max(12, currentStrokeWidth * 3.5),
          color: currentColor,
          strokeWidth: currentStrokeWidth,
        },
      ])
      return
    }
    gesture = { kind: 'draw', start: clamped }
    draftAnnotation = createDraftAnnotation(activeTool, clamped, clamped)
  }

  function movePointer(event: PointerEvent) {
    if (toolbarDrag) return
    const point = imagePoint(event, shell.getBoundingClientRect(), geometry)
    if (!point || !capture) {
      if (!gesture && !selection) hoveredTarget = null
      return
    }
    if (!gesture) {
      if (!selection) hoveredTarget = findCaptureTarget(point, capture?.targets ?? [])
      return
    }

    switch (gesture.kind) {
      case 'new-selection':
        if (distance(gesture.start, point) > sourceTolerance(5, geometry)) {
          hoveredTarget = null
          selection = clampCaptureRectangle(
            normalizeCaptureRectangle(gesture.start, point),
            capture.image_width,
            capture.image_height,
          )
        }
        break
      case 'move-selection':
        if (gesture.originalSelection) {
          selection = clampCaptureRectangle(
            {
              ...gesture.originalSelection,
              x: gesture.originalSelection.x + point.x - gesture.start.x,
              y: gesture.originalSelection.y + point.y - gesture.start.y,
            },
            capture.image_width,
            capture.image_height,
          )
        }
        break
      case 'resize-selection':
        if (gesture.originalSelection && gesture.handle) {
          selection = resizeCaptureRectangle(
            gesture.originalSelection,
            gesture.handle,
            point,
            capture.image_width,
            capture.image_height,
            sourceTolerance(12, geometry),
          )
        }
        break
      case 'draw': {
        const clamped = clampPointToSelection(point, selection)
        if (draftAnnotation?.type === 'pen') {
          const last = draftAnnotation.points.at(-1)
          if (!last || distance(last, clamped) >= sourceTolerance(1.5, geometry)) {
            draftAnnotation = { ...draftAnnotation, points: [...draftAnnotation.points, clamped] }
          }
        } else {
          draftAnnotation = createDraftAnnotation(activeTool, gesture.start, clamped)
        }
        break
      }
      case 'move-annotation':
        if (gesture.originalAnnotation && gesture.annotationsSnapshot) {
          const moved = translateAnnotation(gesture.originalAnnotation, {
            x: point.x - gesture.start.x,
            y: point.y - gesture.start.y,
          })
          annotations = gesture.annotationsSnapshot.map((annotation) =>
            annotation.id === moved.id ? moved : annotation,
          )
        }
        break
      case 'resize-annotation':
        if (gesture.originalAnnotation && gesture.annotationsSnapshot && gesture.handle) {
          const originalBounds = getAnnotationBounds(gesture.originalAnnotation)
          const nextBounds = resizeCaptureRectangle(
            originalBounds,
            gesture.handle,
            point,
            capture.image_width,
            capture.image_height,
            sourceTolerance(8, geometry),
          )
          const resized = resizeAnnotation(gesture.originalAnnotation, originalBounds, nextBounds)
          annotations = gesture.annotationsSnapshot.map((annotation) =>
            annotation.id === resized.id ? resized : annotation,
          )
        }
        break
    }
  }

  function endPointer(event: PointerEvent) {
    if (toolbarDrag || !gesture || !capture) return
    const point = imagePoint(event, shell.getBoundingClientRect(), geometry) ?? gesture.start
    const completedGesture = gesture
    gesture = null

    if (completedGesture.kind === 'new-selection') {
      if (distance(completedGesture.start, point) <= sourceTolerance(5, geometry) && completedGesture.target) {
        selection = captureTargetRectangle(completedGesture.target)
      } else if (selection && (selection.width < sourceTolerance(6, geometry) || selection.height < sourceTolerance(6, geometry))) {
        selection = null
      }
      hoveredTarget = null
      if (selection) void scheduleToolbarLayout()
      return
    }
    if (completedGesture.kind === 'draw') {
      const draft = draftAnnotation
      draftAnnotation = null
      if (draft && annotationHasSize(draft, getAnnotationBounds(draft), sourceTolerance(3, geometry))) commitAnnotations([...annotations, draft])
      return
    }
    if (
      (completedGesture.kind === 'move-annotation' || completedGesture.kind === 'resize-annotation') &&
      completedGesture.annotationsSnapshot &&
      JSON.stringify(completedGesture.annotationsSnapshot) !== JSON.stringify(annotations)
    ) {
      undoStack = [...undoStack, completedGesture.annotationsSnapshot]
      redoStack = []
    }
  }

  function beginSelectionResize(event: PointerEvent, handle: ResizeHandle) {
    if (!selection || !capture || completing) return
    event.stopPropagation()
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    const point = imagePoint(event, shell.getBoundingClientRect(), geometry)
    if (!point) return
    gesture = {
      kind: 'resize-selection',
      start: point,
      handle,
      originalSelection: { ...selection },
    }
  }

  function beginAnnotationResize(event: PointerEvent, handle: ResizeHandle) {
    if (!selectedAnnotation || !capture || completing) return
    event.stopPropagation()
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    const point = imagePoint(event, shell.getBoundingClientRect(), geometry)
    if (!point) return
    gesture = {
      kind: 'resize-annotation',
      start: point,
      handle,
      originalAnnotation: structuredClone(selectedAnnotation),
      annotationsSnapshot: cloneAnnotations(annotations),
    }
  }

  function createDraftAnnotation(
    tool: AnnotationTool,
    start: CapturePoint,
    end: CapturePoint,
  ): CaptureAnnotation | null {
    const base = { id: newId(), color: currentColor, strokeWidth: currentStrokeWidth }
    if (tool === 'arrow' || tool === 'line') return { ...base, type: tool, start, end }
    if (tool === 'pen') return { ...base, type: 'pen', points: [start, end] }
    if (tool === 'rectangle' || tool === 'ellipse' || tool === 'highlight' || tool === 'mosaic') {
      return {
        ...base,
        type: tool,
        rect: normalizeCaptureRectangle(start, end),
        ...(tool === 'mosaic' ? { pixelSize: Math.max(8, currentStrokeWidth * 3) } : {}),
      }
    }
    return null
  }

  function commitText() {
    if (!textDraft) return
    const value = textDraft.value.trim()
    if (value) {
      commitAnnotations([
        ...annotations,
        {
          id: newId(),
          type: 'text',
          point: textDraft.point,
          text: value,
          fontSize: Math.max(18, currentStrokeWidth * 5),
          color: currentColor,
          strokeWidth: currentStrokeWidth,
        },
      ])
    }
    textDraft = null
  }

  function commitAnnotations(next: CaptureAnnotation[]) {
    undoStack = [...undoStack, cloneAnnotations(annotations)]
    redoStack = []
    annotations = next
  }

  function undo() {
    const previous = undoStack.at(-1)
    if (!previous) return
    undoStack = undoStack.slice(0, -1)
    redoStack = [cloneAnnotations(annotations), ...redoStack]
    annotations = previous
    selectedAnnotationId = null
  }

  function redo() {
    const next = redoStack[0]
    if (!next) return
    redoStack = redoStack.slice(1)
    undoStack = [...undoStack, cloneAnnotations(annotations)]
    annotations = next
    selectedAnnotationId = null
  }

  function deleteSelected() {
    if (!selectedAnnotationId) return
    commitAnnotations(annotations.filter((annotation) => annotation.id !== selectedAnnotationId))
    selectedAnnotationId = null
  }

  function setTool(tool: AnnotationTool) {
    activeTool = tool
    selectedAnnotationId = null
    textDraft = null
    draftAnnotation = null
    overflowPanelOpen = false
  }

  function setColor(color: string) {
    currentColor = color
    updateSelectedAppearance({ color })
  }

  function setStrokeWidth(strokeWidth: number) {
    currentStrokeWidth = strokeWidth
    updateSelectedAppearance({ strokeWidth })
  }

  function toggleStylePanel() {
    stylePanelOpen = !stylePanelOpen
    if (stylePanelOpen) overflowPanelOpen = false
  }

  function toggleOverflowPanel() {
    overflowPanelOpen = !overflowPanelOpen
    if (overflowPanelOpen) stylePanelOpen = false
  }

  function updateSelectedAppearance(patch: { color?: string; strokeWidth?: number }) {
    if (!selectedAnnotationId) return
    commitAnnotations(
      annotations.map((annotation) =>
        annotation.id === selectedAnnotationId ? ({ ...annotation, ...patch } as CaptureAnnotation) : annotation,
      ),
    )
  }

  async function finalize(copyToClipboard: boolean) {
    if (!capture || !sourceImage || !selection || completing) return
    commitText()
    completing = true
    errorMessage = ''
    try {
      await invoke('complete_screen_capture', {
        input: {
          capture_session_id: capture.capture_session_id,
          selection: roundedRectangle(selection),
          png_base64:
            annotations.length > 0
              ? exportAnnotatedCapture(
                  sourceImage,
                  selection,
                  annotations,
                  t($locale, 'Could not create the capture export canvas'),
                )
              : null,
          copy_to_clipboard: copyToClipboard,
        },
      })
    } catch (cause) {
      errorMessage = messageFrom(cause)
      completing = false
    }
  }

  async function pinCapture() {
    if (!capture || !sourceImage || !selection || completing) return
    commitText()
    completing = true
    errorMessage = ''
    try {
      await invoke('pin_screen_capture', {
        input: {
          capture_session_id: capture.capture_session_id,
          selection: roundedRectangle(selection),
          png_base64:
            annotations.length > 0
              ? exportAnnotatedCapture(
                  sourceImage,
                  selection,
                  annotations,
                  t($locale, 'Could not create the capture export canvas'),
                )
              : null,
          copy_to_clipboard: false,
        },
      })
    } catch (cause) {
      errorMessage = messageFrom(cause)
      completing = false
    }
  }

  async function beginScrolling() {
    if (!capture || !selection || completing) return
    if (annotations.length > 0) {
      errorMessage = t($locale, 'Start scrolling capture before annotating the stitched image.')
      return
    }
    completing = true
    errorMessage = ''
    try {
      await invoke('begin_scrolling_capture', {
        input: {
          capture_session_id: capture.capture_session_id,
          selection: roundedRectangle(selection),
        },
      })
    } catch (cause) {
      errorMessage = messageFrom(cause)
      completing = false
    }
  }

  async function cancelCapture() {
    if (completing) return
    completing = true
    try {
      await invoke('cancel_screen_capture')
    } catch (cause) {
      errorMessage = messageFrom(cause)
      completing = false
    }
  }

  async function handleKeydown(event: KeyboardEvent) {
    if (textDraft) {
      if (event.key === 'Escape') textDraft = null
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') commitText()
      return
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
      event.preventDefault()
      if (event.shiftKey) redo()
      else undo()
      return
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'y') {
      event.preventDefault()
      redo()
      return
    }
    if ((event.key === 'Delete' || event.key === 'Backspace') && selectedAnnotationId) {
      event.preventDefault()
      deleteSelected()
      return
    }
    if (event.key === 'Escape') {
      if (stylePanelOpen || overflowPanelOpen) {
        stylePanelOpen = false
        overflowPanelOpen = false
      } else if (draftAnnotation) {
        draftAnnotation = null
        gesture = null
      } else if (selectedAnnotationId) selectedAnnotationId = null
      else await cancelCapture()
      return
    }
    if (event.key === 'Enter' && selection) {
      event.preventDefault()
      await finalize(false)
      return
    }
    if (!event.metaKey && !event.ctrlKey && !event.altKey) {
      const shortcuts: Record<string, AnnotationTool> = {
        v: 'select', r: 'rectangle', e: 'ellipse', a: 'arrow', l: 'line',
        p: 'pen', t: 'text', h: 'highlight', b: 'mosaic', n: 'counter',
      }
      const tool = shortcuts[event.key.toLowerCase()]
      if (tool && selection) setTool(tool)
    }
  }

  function useFullScreenSelection() {
    if (!capture) return
    selection = fullScreenSelection(capture)
    hoveredTarget = null
    void scheduleToolbarLayout()
  }

  function textDraftStyle() {
    if (!textDraft) return ''
    return computeTextDraftStyle(
      { point: textDraft.point, color: currentColor, strokeWidth: currentStrokeWidth },
      geometry,
    )
  }

  function captureToolbarPosition() {
    return computeToolbarPosition(
      { selection, toolbarWidth, toolbarHeight, toolbarManualX, toolbarManualY },
      geometry,
    )
  }

  function captureToolbarStyle() {
    const position = captureToolbarPosition()
    return position ? `left:${position.left}px;top:${position.top}px` : ''
  }

  $: toolbarStyle = captureToolbarStyle()
  $: if (selection && viewportWidth > 64 && toolbarWidth > 0) {
    toolbarManualX
    toolbarManualY
    geometry
    applyToolbarPosition()
  }

  function resizeViewport() {
    const width = Math.round(shell?.clientWidth || document.documentElement.clientWidth || window.innerWidth)
    const height = Math.round(shell?.clientHeight || document.documentElement.clientHeight || window.innerHeight)
    if (width > 0) viewportWidth = width
    if (height > 0) viewportHeight = height
  }

  async function scheduleToolbarLayout() {
    resizeViewport()
    await tick()
    applyToolbarPosition()
  }

  function applyToolbarPosition() {
    const position = captureToolbarPosition()
    if (!position) return
    toolbarStyle = `left:${position.left}px;top:${position.top}px`
    if (toolbarHost) {
      toolbarHost.style.left = `${position.left}px`
      toolbarHost.style.top = `${position.top}px`
    }
  }

  function toolbarPopoverOpensDownward() {
    return toolbarPopoverDown(captureToolbarPosition())
  }

  function bindToolbarDragListeners() {
    if (toolbarDragListening) return
    toolbarDragListening = true
    window.addEventListener('pointermove', moveToolbarDrag, true)
    window.addEventListener('pointerup', endToolbarDrag, true)
    window.addEventListener('pointercancel', endToolbarDrag, true)
    window.addEventListener('mousemove', moveToolbarDragFromMouse, true)
    window.addEventListener('mouseup', endToolbarDragFromMouse, true)
  }

  function unbindToolbarDragListeners() {
    if (!toolbarDragListening) return
    toolbarDragListening = false
    window.removeEventListener('pointermove', moveToolbarDrag, true)
    window.removeEventListener('pointerup', endToolbarDrag, true)
    window.removeEventListener('pointercancel', endToolbarDrag, true)
    window.removeEventListener('mousemove', moveToolbarDragFromMouse, true)
    window.removeEventListener('mouseup', endToolbarDragFromMouse, true)
  }

  function isToolbarDragSource(event: PointerEvent) {
    const target = event.target
    if (!(target instanceof Element)) return false
    if (target.closest('.toolbar-popover, textarea, input')) return false
    const button = target.closest('button')
    return !button || button.classList.contains('toolbar-drag')
  }

  function beginToolbarDrag(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary) return
    if (!isToolbarDragSource(event)) return
    event.preventDefault()
    event.stopPropagation()
    stylePanelOpen = false
    overflowPanelOpen = false
    const position = captureToolbarPosition()
    if (!position) return
    toolbarDrag = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: position.left,
      originY: position.top,
    }
    applyToolbarPosition()
    bindToolbarDragListeners()
  }

  function moveToolbarDrag(event: PointerEvent) {
    if (!toolbarDrag || (event.pointerId !== toolbarDrag.pointerId && event.pointerId !== 0)) return
    if (event.buttons === 0) {
      endToolbarDrag(event)
      return
    }
    event.preventDefault()
    event.stopPropagation()
    toolbarManualX = toolbarDrag.originX + event.clientX - toolbarDrag.startX
    toolbarManualY = toolbarDrag.originY + event.clientY - toolbarDrag.startY
    applyToolbarPosition()
  }

  function moveToolbarDragFromMouse(event: MouseEvent) {
    if (!toolbarDrag) return
    if (event.buttons === 0) {
      endToolbarDragFromMouse(event)
      return
    }
    event.preventDefault()
    event.stopPropagation()
    toolbarManualX = toolbarDrag.originX + event.clientX - toolbarDrag.startX
    toolbarManualY = toolbarDrag.originY + event.clientY - toolbarDrag.startY
    applyToolbarPosition()
  }

  function endToolbarDrag(event: PointerEvent) {
    if (!toolbarDrag) return
    if (event.pointerId !== toolbarDrag.pointerId && event.pointerId !== 0) return
    event.preventDefault()
    event.stopPropagation()
    toolbarDrag = null
    unbindToolbarDragListeners()
  }

  function endToolbarDragFromMouse(event: MouseEvent) {
    if (!toolbarDrag) return
    event.preventDefault()
    event.stopPropagation()
    toolbarDrag = null
    unbindToolbarDragListeners()
  }

  function roundedRectangle(rectangle: CaptureRectangle): CaptureRectangle {
    return {
      x: Math.max(0, Math.round(rectangle.x)),
      y: Math.max(0, Math.round(rectangle.y)),
      width: Math.max(1, Math.round(rectangle.width)),
      height: Math.max(1, Math.round(rectangle.height)),
    }
  }

  function cloneAnnotations(value: CaptureAnnotation[]) {
    return structuredClone(value)
  }

  function newId() {
    return crypto.randomUUID()
  }

  function messageFrom(cause: unknown) {
    return cause instanceof Error ? cause.message : String(cause)
  }
</script>

<main
  bind:this={shell}
  class:has-selection={selection}
  class:completing
  class="capture-editor"
  onpointerdown={beginPointer}
  onpointermove={movePointer}
  onpointerup={endPointer}
  onpointercancel={endPointer}
  ondblclick={() => {
    if (!selection) useFullScreenSelection()
  }}
  oncontextmenu={(event) => {
    event.preventDefault()
    if (draftAnnotation) {
      draftAnnotation = null
      gesture = null
    } else if (selectedAnnotationId) selectedAnnotationId = null
    else void cancelCapture()
  }}
>
  {#if sourceReady && displayRectangle}
    <canvas
      bind:this={sourceCanvas}
      class="capture-image"
      width={capture?.image_width ?? 1}
      height={capture?.image_height ?? 1}
      style={imageLayerStyle(geometry)}
    ></canvas>
    <canvas
      bind:this={annotationCanvas}
      class="annotation-layer"
      width={capture?.image_width ?? 1}
      height={capture?.image_height ?? 1}
      style={imageLayerStyle(geometry)}
    ></canvas>
  {/if}

  {#if !selection && displayRectangle}
    <div class="image-mask" style={imageLayerStyle(geometry)}></div>
  {/if}

  {#if hoveredTarget && !selection}
    <div class="smart-target" style={cssRectangle(hoveredTarget, geometry)}>
      <span>{hoveredTarget.app_name || hoveredTarget.title}</span>
    </div>
  {/if}

  {#if selection}
    <div class="selection-frame" style={cssRectangle(selection, geometry)}>
      <span class="selection-size">{Math.round(selection.width)} × {Math.round(selection.height)}</span>
      {#each ['nw', 'ne', 'se', 'sw'] as handle}
        <button
          data-capture-ui
          class={`resize-handle ${handle}`}
          aria-label={t($locale, 'Resize selection: {handle}', { handle })}
          onpointerdown={(event) => beginSelectionResize(event, handle as ResizeHandle)}
        ></button>
      {/each}
    </div>
  {/if}

  {#if selectedBounds && activeTool === 'select'}
    <div class="annotation-selection" style={cssRectangle(selectedBounds, geometry)}>
      {#each ['nw', 'ne', 'se', 'sw'] as handle}
        <button
          data-capture-ui
          class={`annotation-handle ${handle}`}
          aria-label={t($locale, 'Resize annotation: {handle}', { handle })}
          onpointerdown={(event) => beginAnnotationResize(event, handle as ResizeHandle)}
        ></button>
      {/each}
    </div>
  {/if}

  {#if textDraft}
    <textarea
      bind:this={textInput}
      bind:value={textDraft.value}
      data-capture-ui
      class="text-editor"
      placeholder={t($locale, 'Enter text…')}
      style={textDraftStyle()}
      onblur={commitText}
    ></textarea>
  {/if}

  {#if loading}
    <div class="capture-status" data-capture-ui>
      <strong>{t($locale, 'Reading screen…')}</strong>
    </div>
  {:else if !selection && !errorMessage}
    <div class="capture-help" data-capture-ui>
      <strong>{t($locale, 'Hover to select a window, or drag to select freely')}</strong>
      <span>{t($locale, 'Click a window to select its bounds · Double-click for full screen · Esc / right-click to cancel')}</span>
    </div>
  {/if}

  {#if selection}
    <CaptureToolbar
      bind:host={toolbarHost}
      bind:toolbarWidth
      bind:toolbarHeight
      {toolbarStyle}
      popoverDown={toolbarPopoverOpensDownward()}
      {activeTool}
      {stylePanelOpen}
      {overflowPanelOpen}
      {currentColor}
      {currentStrokeWidth}
      {colors}
      {strokeWidths}
      canUndo={undoStack.length > 0}
      canRedo={redoStack.length > 0}
      canDelete={selectedAnnotationId !== null}
      onBeginDrag={beginToolbarDrag}
      onSetTool={setTool}
      onToggleStylePanel={toggleStylePanel}
      onToggleOverflowPanel={toggleOverflowPanel}
      onSetColor={setColor}
      onSetStrokeWidth={setStrokeWidth}
      onUndo={undo}
      onRedo={redo}
      onDelete={deleteSelected}
      onFinalize={(copyToClipboard) => void finalize(copyToClipboard)}
      onCancel={() => void cancelCapture()}
    />
  {/if}

  {#if errorMessage}
    <div class="capture-error" data-capture-ui>
      <strong>{t($locale, 'The capture tool encountered a problem')}</strong>
      <span>{errorMessage}</span>
      <button onclick={() => (errorMessage = '')}>{t($locale, 'Close')}</button>
    </div>
  {/if}

  {#if completing}
    <div class="completing-mask" data-capture-ui><span>{t($locale, 'Processing capture…')}</span></div>
  {/if}
</main>
