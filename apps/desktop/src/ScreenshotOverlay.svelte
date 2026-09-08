<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'

  import { t } from './lib/i18n'
  import { locale } from './lib/preferences'
  import CaptureToolbar from './lib/screen-capture/CaptureToolbar.svelte'
  import {
    getAnnotationBounds,
    type AnnotationTool,
    type CaptureAnnotation,
    type CaptureRectangle,
    type CaptureTarget,
    type ResizeHandle,
    type ScreenCaptureView,
  } from './lib/screenCapture'
  import { renderCaptureAnnotations } from './lib/screenshotRenderer'
  import {
    cloneAnnotations,
    commitAnnotations,
    counterAnnotation,
    createDraftAnnotation,
    deleteAnnotation,
    emptyAnnotationHistory,
    nextCounterNumber,
    pushUndoSnapshot,
    redoAnnotations,
    replaceAnnotations,
    textAnnotation,
    undoAnnotations,
    updateAnnotationAppearance,
    type AnnotationHistory,
  } from './lib/screen-capture/annotationModel'
  import { createCaptureActions } from './lib/screen-capture/captureActions'
  import {
    createPointerInteraction,
    type TextDraft,
  } from './lib/screen-capture/pointerInteraction'
  import { createCaptureToolbarPlacement } from './lib/screen-capture/captureToolbarPlacement'
  import {
    cssRectangle,
    fitImage,
    fullScreenSelection,
    imageLayerStyle,
    imagePoint,
    textDraftStyle as computeTextDraftStyle,
  } from './lib/screen-capture/overlayGeometry'

  type DisplayRectangle = CaptureRectangle
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
  let history: AnnotationHistory = emptyAnnotationHistory
  $: annotations = history.annotations
  let draftAnnotation: CaptureAnnotation | null = null
  let activeTool: AnnotationTool = 'select'
  let selectedAnnotationId: string | null = null
  let currentColor = colors[0]!
  let currentStrokeWidth = 4
  let textDraft: TextDraft | null = null
  let loading = true
  let completing = false
  let errorMessage = ''
  let initializingSessionId: string | null = null
  let toolbarHost: HTMLDivElement | null = null
  let toolbarStyle = ''
  let stylePanelOpen = false
  let overflowPanelOpen = false

  const toolbarPlacement = createCaptureToolbarPlacement({
    getPlacement: () => ({ selection, toolbarWidth, toolbarHeight }),
    getGeometry: () => geometry,
    getHost: () => toolbarHost,
    onChange: (style) => {
      toolbarStyle = style
    },
    onDragStart: () => {
      stylePanelOpen = false
      overflowPanelOpen = false
    },
  })

  const captureActions = createCaptureActions({
    complete: (input) => invoke('complete_screen_capture', { input }),
    pin: (input) => invoke('pin_screen_capture', { input }),
    startScrolling: (input) => invoke('begin_scrolling_capture', { input }),
    cancel: () => invoke('cancel_screen_capture'),
    tr: (source) => t($locale, source),
    messageFrom,
    getCapture: () => capture,
    getSourceImage: () => sourceImage,
    getSelection: () => selection,
    getAnnotations: () => annotations,
    isCompleting: () => completing,
    commitText: () => commitText(),
    setCompleting: (value) => {
      completing = value
    },
    setError: (message) => {
      errorMessage = message
    },
  })

  const pointerInteraction = createPointerInteraction({
    getGeometry: () => geometry,
    toImagePoint: (event) => imagePoint(event, shell.getBoundingClientRect(), geometry),
    getState: () => ({
      capture,
      completing,
      selection,
      hoveredTarget,
      activeTool,
      annotations,
      draftAnnotation,
      selectedAnnotationId,
      textDraft,
      style: style(),
    }),
    patch: (next) => {
      if ('selection' in next) selection = next.selection ?? null
      if ('hoveredTarget' in next) hoveredTarget = next.hoveredTarget ?? null
      if ('selectedAnnotationId' in next) selectedAnnotationId = next.selectedAnnotationId ?? null
      if ('draftAnnotation' in next) draftAnnotation = next.draftAnnotation ?? null
      if ('textDraft' in next) textDraft = next.textDraft ?? null
      if (next.annotations) history = replaceAnnotations(history, next.annotations)
    },
    commit,
    pushUndoSnapshot: (snapshot) => {
      history = pushUndoSnapshot(history, snapshot)
    },
    closePanels: () => {
      stylePanelOpen = false
      overflowPanelOpen = false
    },
    clearError: () => {
      errorMessage = ''
    },
    focusTextInput: () => {
      void tick().then(() => textInput?.focus())
    },
    scheduleToolbarLayout: () => {
      void scheduleToolbarLayout()
    },
  })

  function commit(next: CaptureAnnotation[]) {
    history = commitAnnotations(history, next)
  }

  function style() {
    return { color: currentColor, strokeWidth: currentStrokeWidth }
  }

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
      toolbarPlacement.dispose()
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
    history = emptyAnnotationHistory
    draftAnnotation = null
    activeTool = 'select'
    selectedAnnotationId = null
    pointerInteraction.reset()
    textDraft = null
    loading = true
    completing = false
    errorMessage = ''
    toolbarStyle = ''
    toolbarPlacement.reset()
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

  function commitText() {
    if (!textDraft) return
    const value = textDraft.value.trim()
    if (value) {
      commit([...annotations, textAnnotation(textDraft.point, value, style())])
    }
    textDraft = null
  }

  function undo() {
    history = undoAnnotations(history)
    selectedAnnotationId = null
  }

  function redo() {
    history = redoAnnotations(history)
    selectedAnnotationId = null
  }

  function deleteSelected() {
    if (!selectedAnnotationId) return
    history = deleteAnnotation(history, selectedAnnotationId)
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
    history = updateAnnotationAppearance(history, selectedAnnotationId, patch)
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
        pointerInteraction.reset()
      } else if (selectedAnnotationId) selectedAnnotationId = null
      else await captureActions.cancelCapture()
      return
    }
    if (event.key === 'Enter' && selection) {
      event.preventDefault()
      await captureActions.finalize(false)
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

  $: toolbarStyle = toolbarPlacement.style()
  $: if (selection && viewportWidth > 64 && toolbarWidth > 0) {
    geometry
    toolbarPlacement.apply()
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
    toolbarPlacement.apply()
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
  onpointerdown={pointerInteraction.begin}
  onpointermove={pointerInteraction.move}
  onpointerup={pointerInteraction.end}
  onpointercancel={pointerInteraction.end}
  ondblclick={() => {
    if (!selection) useFullScreenSelection()
  }}
  oncontextmenu={(event) => {
    event.preventDefault()
    if (draftAnnotation) {
      draftAnnotation = null
      pointerInteraction.reset()
    } else if (selectedAnnotationId) selectedAnnotationId = null
    else void captureActions.cancelCapture()
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
          onpointerdown={(event) => pointerInteraction.beginSelectionResize(event, handle as ResizeHandle)}
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
          onpointerdown={(event) => pointerInteraction.beginAnnotationResize(event, handle as ResizeHandle)}
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
      popoverDown={toolbarPlacement.popoverOpensDownward()}
      {activeTool}
      {stylePanelOpen}
      {overflowPanelOpen}
      {currentColor}
      {currentStrokeWidth}
      {colors}
      {strokeWidths}
      canUndo={history.undoStack.length > 0}
      canRedo={history.redoStack.length > 0}
      canDelete={selectedAnnotationId !== null}
      onBeginDrag={toolbarPlacement.beginDrag}
      onSetTool={setTool}
      onToggleStylePanel={toggleStylePanel}
      onToggleOverflowPanel={toggleOverflowPanel}
      onSetColor={setColor}
      onSetStrokeWidth={setStrokeWidth}
      onUndo={undo}
      onRedo={redo}
      onDelete={deleteSelected}
      onFinalize={(copyToClipboard) => void captureActions.finalize(copyToClipboard)}
      onCancel={() => void captureActions.cancelCapture()}
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
