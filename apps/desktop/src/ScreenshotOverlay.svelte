<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import {
    Check,
    Circle,
    Copy,
    Ellipsis,
    Grid3X3,
    GripVertical,
    Hash,
    Highlighter,
    Minus,
    MousePointer2,
    MoveUpRight,
    Pencil,
    RectangleHorizontal,
    Redo2,
    Trash2,
    Type,
    Undo2,
    X,
  } from '@lucide/svelte'
  import { onMount, tick } from 'svelte'

  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'
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
  let historyPast: CaptureAnnotation[][] = []
  let historyFuture: CaptureAnnotation[][] = []
  let textDraft: TextDraft | null = null
  let loading = true
  let completing = false
  let errorMessage = ''
  let initializingSessionId: string | null = null
  let toolbarManualX: number | null = null
  let toolbarManualY: number | null = null
  let toolbarDrag: ToolbarDrag | null = null
  let stylePanelOpen = false
  let overflowPanelOpen = false

  $: displayRectangle = capture
    ? fitImage(capture.image_width, capture.image_height, viewportWidth, viewportHeight)
    : null
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
    const resize = () => {
      viewportWidth = shell?.clientWidth || window.innerWidth
      viewportHeight = shell?.clientHeight || window.innerHeight
    }
    const keydown = (event: KeyboardEvent) => void handleKeydown(event)
    const preventSelection = (event: Event) => {
      const target = event.target
      if (target instanceof Element && target.closest('textarea, input, [contenteditable="true"]')) return
      event.preventDefault()
    }
    window.addEventListener('resize', resize)
    window.addEventListener('keydown', keydown)
    window.addEventListener('mousemove', moveToolbarDrag)
    window.addEventListener('mouseup', endToolbarDrag)
    document.addEventListener('selectstart', preventSelection)
    resize()
    let disposed = false
    let unlisten: UnlistenFn | undefined
    void listen<{ session_id: string }>('screen-capture-session-ready', (event) => {
      void initialize(event.payload.session_id)
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
      window.removeEventListener('mousemove', moveToolbarDrag)
      window.removeEventListener('mouseup', endToolbarDrag)
      document.removeEventListener('selectstart', preventSelection)
    }
  })

  async function resumeActiveCapture() {
    try {
      const active = await invoke<ScreenCaptureView>('get_active_capture_info')
      await initialize(active.session_id, active)
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
    historyPast = []
    historyFuture = []
    textDraft = null
    loading = true
    completing = false
    errorMessage = ''
    toolbarManualX = null
    toolbarManualY = null
    toolbarDrag = null
    stylePanelOpen = false
    overflowPanelOpen = false
  }

  async function initialize(sessionId: string, active?: ScreenCaptureView) {
    if (initializingSessionId === sessionId) return
    initializingSessionId = sessionId
    resetEditor()
    try {
      capture = active ?? (await invoke<ScreenCaptureView>('get_active_capture_info'))
      if (capture.session_id !== sessionId) throw new Error('截图会话已变化，请重新截图')
      const rgba = await invoke<ArrayBuffer>('read_capture_rgba_bytes', {
        sessionId: capture.session_id,
      })
      const expectedBytes = capture.image_width * capture.image_height * 4
      if (rgba.byteLength !== expectedBytes) {
        throw new Error(`截图像素数据不完整：应为 ${expectedBytes} 字节，实际为 ${rgba.byteLength} 字节`)
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
      initializingSessionId = null
    }
  }

  function drawSourceImage(imageData: ImageData) {
    if (!sourceCanvas) return
    const context = sourceCanvas.getContext('2d')
    if (!context) throw new Error('无法创建截图显示画布')
    context.putImageData(imageData, 0, 0)
    sourceImage = sourceCanvas
  }

  function imagePoint(event: PointerEvent | MouseEvent): CapturePoint | null {
    if (!capture || !displayRectangle) return null
    const shellBounds = shell.getBoundingClientRect()
    const x = event.clientX - shellBounds.left - displayRectangle.x
    const y = event.clientY - shellBounds.top - displayRectangle.y
    if (x < 0 || y < 0 || x > displayRectangle.width || y > displayRectangle.height) return null
    return {
      x: (x / displayRectangle.width) * capture.image_width,
      y: (y / displayRectangle.height) * capture.image_height,
    }
  }

  function beginPointer(event: PointerEvent) {
    if (!capture || completing || event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('[data-capture-ui]')) return
    stylePanelOpen = false
    overflowPanelOpen = false
    event.preventDefault()
    window.getSelection()?.removeAllRanges()
    const point = imagePoint(event)
    if (!point) return
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    errorMessage = ''

    if (!selection) {
      gesture = { kind: 'new-selection', start: point, target: hoveredTarget ?? undefined }
      return
    }

    if (activeTool === 'select') {
      const tolerance = sourceTolerance(8)
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
    const clamped = clampPointToSelection(point)
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
    const point = imagePoint(event)
    if (!point || !capture) {
      if (!gesture && !selection) hoveredTarget = null
      return
    }
    if (!gesture) {
      if (!selection) hoveredTarget = findCaptureTarget(point)
      return
    }

    switch (gesture.kind) {
      case 'new-selection':
        if (distance(gesture.start, point) > sourceTolerance(5)) {
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
            sourceTolerance(12),
          )
        }
        break
      case 'draw': {
        const clamped = clampPointToSelection(point)
        if (draftAnnotation?.type === 'pen') {
          const last = draftAnnotation.points.at(-1)
          if (!last || distance(last, clamped) >= sourceTolerance(1.5)) {
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
            sourceTolerance(8),
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
    if (!gesture || !capture) return
    const point = imagePoint(event) ?? gesture.start
    const completedGesture = gesture
    gesture = null

    if (completedGesture.kind === 'new-selection') {
      if (distance(completedGesture.start, point) <= sourceTolerance(5) && completedGesture.target) {
        selection = captureTargetRectangle(completedGesture.target)
      } else if (selection && (selection.width < sourceTolerance(6) || selection.height < sourceTolerance(6))) {
        selection = null
      }
      hoveredTarget = null
      return
    }
    if (completedGesture.kind === 'draw') {
      const draft = draftAnnotation
      draftAnnotation = null
      if (draft && annotationHasSize(draft)) commitAnnotations([...annotations, draft])
      return
    }
    if (
      (completedGesture.kind === 'move-annotation' || completedGesture.kind === 'resize-annotation') &&
      completedGesture.annotationsSnapshot &&
      JSON.stringify(completedGesture.annotationsSnapshot) !== JSON.stringify(annotations)
    ) {
      historyPast = [...historyPast, completedGesture.annotationsSnapshot]
      historyFuture = []
    }
  }

  function beginSelectionResize(event: PointerEvent, handle: ResizeHandle) {
    if (!selection || !capture || completing) return
    event.stopPropagation()
    ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
    const point = imagePoint(event)
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
    const point = imagePoint(event)
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

  function annotationHasSize(annotation: CaptureAnnotation) {
    if (annotation.type === 'pen') return annotation.points.length > 1
    const bounds = getAnnotationBounds(annotation)
    return Math.max(bounds.width, bounds.height) >= sourceTolerance(3)
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
    historyPast = [...historyPast, cloneAnnotations(annotations)]
    historyFuture = []
    annotations = next
  }

  function undo() {
    const previous = historyPast.at(-1)
    if (!previous) return
    historyPast = historyPast.slice(0, -1)
    historyFuture = [cloneAnnotations(annotations), ...historyFuture]
    annotations = previous
    selectedAnnotationId = null
  }

  function redo() {
    const next = historyFuture[0]
    if (!next) return
    historyFuture = historyFuture.slice(1)
    historyPast = [...historyPast, cloneAnnotations(annotations)]
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
      const pngBase64 = exportAnnotatedCapture(sourceImage, selection, annotations)
      await invoke('complete_screen_capture', {
        input: {
          session_id: capture.session_id,
          png_base64: pngBase64,
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
      const pngBase64 = exportAnnotatedCapture(sourceImage, selection, annotations)
      await invoke('pin_screen_capture', {
        input: {
          session_id: capture.session_id,
          png_base64: pngBase64,
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
      errorMessage = '请先开始滚动截图，再对拼接后的长图添加标注'
      return
    }
    completing = true
    errorMessage = ''
    try {
      await invoke('begin_scrolling_capture', {
        input: {
          session_id: capture.session_id,
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

  function findCaptureTarget(point: CapturePoint) {
    return capture?.targets.find((target) => pointInRectangle(point, target)) ?? null
  }

  function captureTargetRectangle(target: CaptureTarget): CaptureRectangle {
    return { x: target.x, y: target.y, width: target.width, height: target.height }
  }

  function useFullScreenSelection() {
    if (!capture) return
    selection = { x: 0, y: 0, width: capture.image_width, height: capture.image_height }
    hoveredTarget = null
  }

  function clampPointToSelection(point: CapturePoint): CapturePoint {
    if (!selection) return point
    return {
      x: Math.min(selection.x + selection.width, Math.max(selection.x, point.x)),
      y: Math.min(selection.y + selection.height, Math.max(selection.y, point.y)),
    }
  }

  function sourceTolerance(cssPixels: number) {
    if (!capture || !displayRectangle) return cssPixels
    return cssPixels * (capture.image_width / Math.max(1, displayRectangle.width))
  }

  function cssRectangle(rectangle: CaptureRectangle) {
    if (!capture || !displayRectangle) return ''
    const scaleX = displayRectangle.width / capture.image_width
    const scaleY = displayRectangle.height / capture.image_height
    return `left:${displayRectangle.x + rectangle.x * scaleX}px;top:${displayRectangle.y + rectangle.y * scaleY}px;width:${rectangle.width * scaleX}px;height:${rectangle.height * scaleY}px`
  }

  function imageLayerStyle() {
    if (!displayRectangle) return ''
    return `left:${displayRectangle.x}px;top:${displayRectangle.y}px;width:${displayRectangle.width}px;height:${displayRectangle.height}px`
  }

  function textDraftStyle() {
    if (!textDraft || !capture || !displayRectangle) return ''
    const scaleX = displayRectangle.width / capture.image_width
    const scaleY = displayRectangle.height / capture.image_height
    return `left:${displayRectangle.x + textDraft.point.x * scaleX}px;top:${displayRectangle.y + textDraft.point.y * scaleY}px;color:${currentColor};font-size:${Math.max(14, currentStrokeWidth * 5 * scaleY)}px`
  }

  function captureToolbarPosition() {
    if (!selection || !capture || !displayRectangle) return null
    const scaleX = displayRectangle.width / capture.image_width
    const scaleY = displayRectangle.height / capture.image_height
    const selectionLeft = displayRectangle.x + selection.x * scaleX
    const selectionTop = displayRectangle.y + selection.y * scaleY
    const selectionWidth = selection.width * scaleX
    const selectionBottom = selectionTop + selection.height * scaleY
    const measuredWidth = toolbarWidth || Math.min(560, viewportWidth - 28)
    const measuredHeight = toolbarHeight || 50
    const baseLeft = selectionLeft + selectionWidth - measuredWidth
    const below = selectionBottom + 12
    const baseTop =
      below + measuredHeight <= viewportHeight - 14
        ? below
        : Math.max(14, selectionTop - measuredHeight - 12)
    const left = Math.min(
      Math.max(14, toolbarManualX ?? baseLeft),
      Math.max(14, viewportWidth - measuredWidth - 14),
    )
    const top = Math.min(
      Math.max(14, toolbarManualY ?? baseTop),
      Math.max(14, viewportHeight - measuredHeight - 14),
    )
    return { left, top }
  }

  function captureToolbarStyle() {
    const position = captureToolbarPosition()
    return position ? `left:${position.left}px;top:${position.top}px` : ''
  }

  function toolbarPopoverOpensDownward() {
    const position = captureToolbarPosition()
    return position ? position.top < 96 : false
  }

  function beginToolbarDrag(event: MouseEvent) {
    if (event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    stylePanelOpen = false
    overflowPanelOpen = false
    const position = captureToolbarPosition()
    if (!position) return
    toolbarDrag = {
      startX: event.clientX,
      startY: event.clientY,
      originX: position.left,
      originY: position.top,
    }
  }

  function moveToolbarDrag(event: MouseEvent) {
    if (!toolbarDrag) return
    event.preventDefault()
    event.stopPropagation()
    toolbarManualX = toolbarDrag.originX + event.clientX - toolbarDrag.startX
    toolbarManualY = toolbarDrag.originY + event.clientY - toolbarDrag.startY
  }

  function endToolbarDrag(event: MouseEvent) {
    if (!toolbarDrag) return
    event.preventDefault()
    event.stopPropagation()
    toolbarDrag = null
  }

  function fitImage(imageWidth: number, imageHeight: number, width: number, height: number): DisplayRectangle {
    const scale = Math.min(width / imageWidth, height / imageHeight)
    const fittedWidth = imageWidth * scale
    const fittedHeight = imageHeight * scale
    return { x: (width - fittedWidth) / 2, y: (height - fittedHeight) / 2, width: fittedWidth, height: fittedHeight }
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
      style={imageLayerStyle()}
    ></canvas>
    <canvas
      bind:this={annotationCanvas}
      class="annotation-layer"
      width={capture?.image_width ?? 1}
      height={capture?.image_height ?? 1}
      style={imageLayerStyle()}
    ></canvas>
  {/if}

  {#if !selection && displayRectangle}
    <div class="image-mask" style={imageLayerStyle()}></div>
  {/if}

  {#if hoveredTarget && !selection}
    <div class="smart-target" style={cssRectangle(hoveredTarget)}>
      <span>{hoveredTarget.app_name || hoveredTarget.title}</span>
    </div>
  {/if}

  {#if selection}
    <div class="selection-frame" style={cssRectangle(selection)}>
      <span class="selection-size">{Math.round(selection.width)} × {Math.round(selection.height)}</span>
      {#each ['nw', 'ne', 'se', 'sw'] as handle}
        <button
          data-capture-ui
          class={`resize-handle ${handle}`}
          aria-label={`调整选区 ${handle}`}
          onpointerdown={(event) => beginSelectionResize(event, handle as ResizeHandle)}
        ></button>
      {/each}
    </div>
  {/if}

  {#if selectedBounds && activeTool === 'select'}
    <div class="annotation-selection" style={cssRectangle(selectedBounds)}>
      {#each ['nw', 'ne', 'se', 'sw'] as handle}
        <button
          data-capture-ui
          class={`annotation-handle ${handle}`}
          aria-label={`调整标注 ${handle}`}
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
      placeholder="输入文字…"
      style={textDraftStyle()}
      onblur={commitText}
    ></textarea>
  {/if}

  {#if loading}
    <div class="capture-status" data-capture-ui>
      <strong>正在读取屏幕画面…</strong>
    </div>
  {:else if !selection && !errorMessage}
    <div class="capture-help" data-capture-ui>
      <strong>悬停选择窗口，或拖动自由框选</strong>
      <span>点击窗口自动取边界 · 双击使用全屏 · Esc / 右键取消</span>
    </div>
  {/if}

  {#if selection}
    <div
      bind:clientWidth={toolbarWidth}
      bind:clientHeight={toolbarHeight}
      class="capture-toolbar"
      class:popover-down={toolbarPopoverOpensDownward()}
      data-capture-ui
      style={captureToolbarStyle()}
    >
      <button
        class="toolbar-drag"
        aria-label="拖动工具栏"
        title="拖动工具栏"
        onmousedown={beginToolbarDrag}
      ><GripVertical size={17} /></button>
      <span class="divider"></span>
      <div class="tool-group">
        <button class:active={activeTool === 'select'} onclick={() => setTool('select')} title="选择/修改 · V"><MousePointer2 size={18} /></button>
        <button class:active={activeTool === 'rectangle'} onclick={() => setTool('rectangle')} title="矩形 · R"><RectangleHorizontal size={18} /></button>
        <button class:active={activeTool === 'ellipse'} onclick={() => setTool('ellipse')} title="圆形 · E"><Circle size={18} /></button>
        <button class:active={activeTool === 'arrow'} onclick={() => setTool('arrow')} title="箭头 · A"><MoveUpRight size={18} /></button>
        <button class:active={activeTool === 'pen'} onclick={() => setTool('pen')} title="画笔 · P"><Pencil size={18} /></button>
        <button class:active={activeTool === 'text'} onclick={() => setTool('text')} title="文字 · T"><Type size={18} /></button>
        <button class:active={activeTool === 'mosaic'} onclick={() => setTool('mosaic')} title="马赛克 · B"><Grid3X3 size={18} /></button>
      </div>
      <div class="popup-control">
        <button
          class="style-trigger"
          class:active={stylePanelOpen}
          aria-expanded={stylePanelOpen}
          onclick={toggleStylePanel}
          title="颜色与粗细"
        ><i style={`--swatch:${currentColor};transform:scaleY(${currentStrokeWidth / 4})`}></i></button>
        {#if stylePanelOpen}
          <div class="toolbar-popover style-popover" aria-label="颜色与线条粗细">
            <div class="palette" aria-label="标注颜色">
              {#each colors as color}
                <button
                  class:active={currentColor === color}
                  class="color-button"
                  style={`--swatch:${color}`}
                  onclick={() => setColor(color)}
                  title={`颜色 ${color}`}
                ></button>
              {/each}
            </div>
            <span class="popover-divider"></span>
            <div class="stroke-picker" aria-label="线条粗细">
              {#each strokeWidths as width}
                <button class:active={currentStrokeWidth === width} onclick={() => setStrokeWidth(width)} title={`${width}px`}>
                  <i style={`height:${Math.max(2, width / 2)}px`}></i>
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </div>
      <div class="popup-control">
        <button
          class:active={overflowPanelOpen || activeTool === 'line' || activeTool === 'highlight' || activeTool === 'counter'}
          aria-expanded={overflowPanelOpen}
          onclick={toggleOverflowPanel}
          title="更多工具"
        ><Ellipsis size={18} /></button>
        {#if overflowPanelOpen}
          <div class="toolbar-popover more-popover" aria-label="更多工具">
            <button class:active={activeTool === 'line'} onclick={() => setTool('line')} title="直线 · L"><Minus size={18} /></button>
            <button class:active={activeTool === 'highlight'} onclick={() => setTool('highlight')} title="高亮 · H"><Highlighter size={18} /></button>
            <button class:active={activeTool === 'counter'} onclick={() => setTool('counter')} title="序号 · N"><Hash size={18} /></button>
            <button disabled={historyFuture.length === 0} onclick={redo} title="重做 · Ctrl/⌘ Shift Z"><Redo2 size={18} /></button>
            <button disabled={!selectedAnnotationId} onclick={deleteSelected} title="删除选中标注 · Delete"><Trash2 size={18} /></button>
          </div>
        {/if}
      </div>
      <span class="divider"></span>
      <div class="tool-group">
        <button disabled={historyPast.length === 0} onclick={undo} title="撤销 · Ctrl/⌘ Z"><Undo2 size={18} /></button>
      </div>
      <span class="divider"></span>
      <div class="tool-group actions">
        <button onclick={() => finalize(true)} title="复制并插入"><Copy size={18} /></button>
        <button class="confirm" onclick={() => finalize(false)} title="插入文档 · Enter"><Check size={19} /></button>
        <button class="cancel" onclick={cancelCapture} title="取消 · Esc"><X size={19} /></button>
      </div>
    </div>
  {/if}

  {#if errorMessage}
    <div class="capture-error" data-capture-ui>
      <strong>{t($locale, '截图工具遇到问题')}</strong>
      <span>{errorMessage}</span>
      <button onclick={() => (errorMessage = '')}>{t($locale, '关闭')}</button>
    </div>
  {/if}

  {#if completing}
    <div class="completing-mask" data-capture-ui><span>正在处理截图…</span></div>
  {/if}
</main>

<style>
  .capture-editor {
    position: fixed;
    inset: 0;
    overflow: hidden;
    color: #f7fbff;
    background: #000;
    cursor: crosshair;
    -webkit-user-select: none;
    user-select: none;
    touch-action: none;
  }
  .capture-editor * { -webkit-user-select: none; user-select: none; }
  .capture-editor.has-selection { cursor: crosshair; }
  .capture-image, .annotation-layer, .image-mask {
    position: absolute;
    display: block;
  }
  .capture-image { pointer-events: none; }
  .annotation-layer { pointer-events: none; }
  .image-mask { background: rgb(4 8 12 / 46%); pointer-events: none; }
  .smart-target, .selection-frame, .annotation-selection { position: absolute; pointer-events: none; }
  .smart-target {
    z-index: 4;
    border: 2px solid #61d79c;
    background: rgb(86 216 151 / 7%);
    box-shadow: 0 0 0 1px rgb(0 0 0 / 28%);
  }
  .smart-target span, .selection-size {
    position: absolute;
    top: -27px;
    left: 0;
    max-width: 260px;
    overflow: hidden;
    padding: 4px 8px;
    border-radius: 6px;
    color: white;
    background: rgb(16 25 32 / 88%);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .selection-frame {
    z-index: 5;
    border: 2px solid #61d79c;
    box-shadow: 0 0 0 9999px rgb(4 8 12 / 46%);
  }
  .selection-size { top: auto; bottom: -27px; }
  .resize-handle, .annotation-handle {
    position: absolute;
    width: 11px;
    height: 11px;
    padding: 0;
    border: 2px solid #10261d;
    border-radius: 50%;
    background: #72e5ab;
    pointer-events: auto;
  }
  .nw { top: -6px; left: -6px; cursor: nwse-resize; }
  .ne { top: -6px; right: -6px; cursor: nesw-resize; }
  .se { right: -6px; bottom: -6px; cursor: nwse-resize; }
  .sw { bottom: -6px; left: -6px; cursor: nesw-resize; }
  .annotation-selection {
    z-index: 6;
    border: 1px dashed #72c8ff;
    background: rgb(74 164 231 / 5%);
  }
  .annotation-handle { width: 9px; height: 9px; border-color: #153348; background: #72c8ff; }
  .text-editor {
    position: absolute;
    z-index: 12;
    width: min(320px, 35vw);
    min-width: 100px;
    min-height: 38px;
    padding: 5px 7px;
    border: 1px dashed currentColor;
    outline: none;
    background: rgb(10 14 19 / 48%);
    font-weight: 600;
    line-height: 1.25;
    resize: both;
    -webkit-user-select: text;
    user-select: text;
  }
  .capture-help, .capture-status {
    position: fixed;
    top: 22px;
    left: 50%;
    z-index: 20;
    display: grid;
    gap: 3px;
    justify-items: center;
    padding: 10px 17px;
    border: 1px solid rgb(255 255 255 / 18%);
    border-radius: 11px;
    background: rgb(16 23 31 / 84%);
    box-shadow: 0 12px 36px rgb(0 0 0 / 26%);
    backdrop-filter: blur(12px);
    transform: translateX(-50%);
  }
  .capture-help strong, .capture-status strong { font-size: 12px; }
  .capture-help span { color: rgb(255 255 255 / 64%); font-size: 9px; }
  .capture-toolbar {
    position: fixed;
    z-index: 30;
    display: flex;
    max-width: calc(100vw - 28px);
    align-items: center;
    gap: 3px;
    padding: 5px;
    overflow: visible;
    border: 1px solid rgb(255 255 255 / 15%);
    border-radius: 11px;
    background: rgb(18 25 34 / 94%);
    box-shadow: 0 16px 50px rgb(0 0 0 / 38%);
    backdrop-filter: blur(15px);
  }
  .tool-group, .palette, .stroke-picker { display: flex; flex: 0 0 auto; gap: 3px; align-items: center; }
  .capture-toolbar button {
    display: grid;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    place-items: center;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgb(239 247 255 / 76%);
    background: transparent;
    cursor: pointer;
  }
  .capture-toolbar button:hover:not(:disabled) { color: white; background: rgb(255 255 255 / 9%); }
  .capture-toolbar button.active { border-color: rgb(93 199 255 / 36%); color: #8bd5ff; background: rgb(62 159 220 / 18%); }
  .capture-toolbar button:disabled { cursor: default; opacity: 0.3; }
  .capture-toolbar .toolbar-drag { width: 24px; color: rgb(239 247 255 / 42%); cursor: grab; touch-action: none; }
  .capture-toolbar .toolbar-drag:active { cursor: grabbing; }
  .divider { width: 1px; height: 22px; flex: 0 0 auto; margin: 0 2px; background: rgb(255 255 255 / 12%); }
  .popup-control { position: relative; display: flex; flex: 0 0 auto; }
  .toolbar-popover {
    position: absolute;
    right: 0;
    bottom: calc(100% + 8px);
    z-index: 45;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px;
    border: 1px solid rgb(255 255 255 / 15%);
    border-radius: 10px;
    background: rgb(18 25 34 / 97%);
    box-shadow: 0 12px 36px rgb(0 0 0 / 42%);
    backdrop-filter: blur(15px);
  }
  .capture-toolbar.popover-down .toolbar-popover { top: calc(100% + 8px); bottom: auto; }
  .style-popover { right: -34px; }
  .more-popover {
    right: -6px;
    display: grid;
    grid-template-columns: repeat(3, 30px);
    gap: 3px;
  }
  .capture-toolbar .style-trigger i {
    display: block;
    width: 17px;
    height: 4px;
    border: 1px solid rgb(255 255 255 / 45%);
    border-radius: 6px;
    background: var(--swatch);
  }
  .popover-divider { width: 1px; height: 24px; flex: 0 0 auto; background: rgb(255 255 255 / 12%); }
  .palette { gap: 5px; }
  .capture-toolbar .color-button {
    width: 20px;
    height: 20px;
    border: 2px solid rgb(255 255 255 / 22%);
    border-radius: 50%;
    background: var(--swatch);
  }
  .capture-toolbar .color-button.active { border-color: white; box-shadow: 0 0 0 2px rgb(68 171 239 / 70%); }
  .stroke-picker button { width: 25px; height: 30px; }
  .stroke-picker i { display: block; width: 15px; border-radius: 9px; background: currentColor; }
  .actions .confirm { color: #edfff5; background: rgb(49 166 101 / 54%); }
  .actions .cancel { color: #ffafb5; }
  .capture-error {
    position: fixed;
    top: 50%;
    left: 50%;
    z-index: 60;
    display: flex;
    width: min(430px, calc(100vw - 42px));
    flex-direction: column;
    gap: 9px;
    padding: 18px;
    border-radius: 13px;
    color: #531f24;
    background: #fff;
    box-shadow: 0 22px 70px rgb(0 0 0 / 46%);
    transform: translate(-50%, -50%);
  }
  .capture-error strong { font-size: 13px; }
  .capture-error span { font-size: 10px; line-height: 1.5; }
  .capture-error button { align-self: flex-end; padding: 6px 13px; border: 0; border-radius: 7px; color: white; background: #294e39; cursor: pointer; }
  .completing-mask {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    background: rgb(7 10 14 / 28%);
    cursor: wait;
  }
  .completing-mask span { padding: 10px 16px; border-radius: 9px; background: rgb(18 25 34 / 88%); font-size: 11px; }
</style>
