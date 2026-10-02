<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte'
  import type { VisualFeedbackAnnotation, VisualFeedbackData } from '../../generated/feedback'
  import type { AttachmentView } from '../../feedback'
  import { locale } from '../../preferences'
  import { unavailableVoiceInputState, useVoiceInput } from '../../speech/voiceInputContext'
  import type { VisualWorkbenchController } from './visualController'
  import { canvasPoint, createVisualHistory, drawable, emptyVisualState, validVisualAnnotations, type VisualState, type VisualTool } from './visualModel'
  import { visualText } from './visualI18n'
  import { visualAttachmentText } from './visualAttachmentText'
  import VisualMarks from './VisualMarks.svelte'
  import VisualToolbar from './VisualToolbar.svelte'
  import VisualAnnotationEditor from './VisualAnnotationEditor.svelte'
  export let data: VisualFeedbackData
  export let state: VisualState | null = null
  export let attachments: readonly Pick<AttachmentView, 'attachment_id'>[] = []
  export let runtime: VisualWorkbenchController | undefined = undefined
  export let disabled = false
  export let readOnly = false
  export let onChange: (state: VisualState) => void
  export let onOpenReview: (() => void) | undefined = undefined
  let viewport: HTMLDivElement, sheet: SVGSVGElement, editorPanel: HTMLDivElement
  let tool: VisualTool = 'select', color = '#e5484d', width = 4, zoom = 1, selectedId: string | null = null
  let gesture: { id: number; mark: VisualFeedbackAnnotation; source: string } | null = null
  let history = createVisualHistory(), historyVersion = 0, mounted = false, automaticZoom = true
  let backgroundUrl = '', loadedSource = '', loadGeneration = 0, loading = false, loadError = '', message = '', revealed = 0
  const voiceState = useVoiceInput()?.state ?? unavailableVoiceInputState
  const tr = (source: string) => visualText($locale, source)
  $: current = state ?? emptyVisualState()
  $: editable = !disabled && !readOnly
  $: source = JSON.stringify(data)
  $: selected = current.annotations.find((mark) => mark.id === selectedId) ?? null
  $: canUndo = historyVersion >= 0 && history.canUndo
  $: canRedo = historyVersion >= 0 && history.canRedo
  $: recoverHistory(current.annotations)
  $: clearSelection(current.annotations)
  $: if (!editable || (gesture && gesture.source !== source)) gesture = null
  $: if (mounted && source !== loadedSource) { history = createVisualHistory(current.annotations); historyVersion += 1; selectedId = null; automaticZoom = true; fit(); void loadBackground(source) }
  $: if (($voiceState.revealSequence ?? 0) !== revealed) {
    revealed = $voiceState.revealSequence ?? 0
    const destination = $voiceState.revealTarget?.destination
    if (destination?.kind === 'workbench_field' && destination.workbenchType === 'visual_feedback' && destination.sourceVersion === data.source_version
      && current.annotations.some((mark) => mark.id === destination.entityId)) select(destination.entityId)
  }
  onMount(() => {
    mounted = true
    if (!state && editable) onChange(emptyVisualState())
    const observer = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(() => { if (automaticZoom) fit() }) : null
    observer?.observe(viewport); fit()
    return () => observer?.disconnect()
  })
  onDestroy(() => { mounted = false; loadGeneration += 1; releaseBackground() })
  function releaseBackground() { if (backgroundUrl) URL.revokeObjectURL(backgroundUrl); backgroundUrl = '' }
  async function loadBackground(key: string) {
    loadedSource = key; const generation = ++loadGeneration
    releaseBackground(); loadError = ''; loading = data.image_file_name !== null
    try {
      const blob = data.image_file_name === null ? null : await runtime?.loadBackground(data)
      if (generation !== loadGeneration || !mounted) return
      if (data.image_file_name !== null && !blob) throw new Error(tr('Original image unavailable'))
      if (blob) backgroundUrl = URL.createObjectURL(blob)
    } catch (cause) { if (generation === loadGeneration && mounted) loadError = cause instanceof Error ? cause.message : String(cause) }
    finally { if (generation === loadGeneration) loading = false }
  }
  function fit() { if (viewport) zoom = Math.max(.1, Math.min(1, (viewport.clientWidth - 32 || 640) / data.width)) }
  function recoverHistory(annotations: VisualFeedbackAnnotation[]) { history.recover(annotations); historyVersion += 1 }
  function clearSelection(annotations: VisualFeedbackAnnotation[]) { if (selectedId && !annotations.some((mark) => mark.id === selectedId)) selectedId = null }
  function publish(annotations: VisualFeedbackAnnotation[], record = true) {
    if (!editable) return
    if (!validVisualAnnotations(annotations, data.width, data.height)) { message = tr('The annotation limit was reached. Delete a mark to continue.'); return }
    if (record && !history.commit(annotations)) return
    historyVersion += 1; message = ''; onChange({ ...current, annotations, composite_attachment_id: null })
  }
  function undo() { if (!editable) return; const next = history.undo(); if (next) publish(next, false) }
  function redo() { if (!editable) return; const next = history.redo(); if (next) publish(next, false) }
  function remove() { if (selectedId && editable) { publish(current.annotations.filter((mark) => mark.id !== selectedId)); selectedId = null } }
  async function select(id: string) { selectedId = id; tool = 'select'; await tick(); editorPanel?.scrollIntoView?.({ block: 'nearest' }) }
  function chooseTool(next: VisualTool) { tool = next; selectedId = null; gesture = null; if (next === 'text') width = 24; else if (width > 12) width = 4 }
  function changeColor(value: string) { color = value; if (selected) publish(current.annotations.map((mark) => mark.id === selectedId ? { ...mark, color: value } : mark)) }
  function changeWidth(value: number) { width = value; if (selected) publish(current.annotations.map((mark) => mark.id === selectedId ? { ...mark, stroke_width: value } : mark)) }
  function point(event: PointerEvent) { return canvasPoint(event.clientX, event.clientY, sheet.getBoundingClientRect(), data) }
  function start(event: PointerEvent) {
    if (!editable || loading || loadError || event.button !== 0 || gesture) return
    if (tool === 'select') {
      const id = (event.target as Element).closest('[data-visual-mark-id]')?.getAttribute('data-visual-mark-id')
      if (id) void select(id); else selectedId = null
      return
    }
    event.preventDefault(); viewport.focus({ preventScroll: true })
    if (current.annotations.length >= 500) { message = tr('The annotation limit was reached. Delete a mark to continue.'); return }
    const a = point(event), mark: VisualFeedbackAnnotation = { id: `mark_${crypto.randomUUID()}`, kind: tool,
      points: tool === 'text' ? [a] : [a, a], color, stroke_width: width, text: tool === 'text' ? tr('Untitled text') : '', body: '' }
    if (tool === 'text') { publish([...current.annotations, mark]); void select(mark.id); return }
    gesture = { id: event.pointerId, mark, source }; viewport.setPointerCapture?.(event.pointerId)
  }
  function move(event: PointerEvent) {
    if (!gesture || gesture.id !== event.pointerId || !editable) return
    const next = point(event), mark = gesture.mark
    if (mark.kind === 'freehand') {
      if (mark.points.length >= 2048 || (mark.points.at(-1)?.x === next.x && mark.points.at(-1)?.y === next.y)) return
      gesture = { ...gesture, mark: { ...mark, points: [...mark.points, next] } }
    } else gesture = { ...gesture, mark: { ...mark, points: [mark.points[0], next] } }
  }
  function finish(event: PointerEvent) {
    if (!gesture || gesture.id !== event.pointerId) return
    move(event); const mark = gesture.mark; gesture = null
    if (drawable(mark)) { publish([...current.annotations, mark]); selectedId = mark.id }
    if (viewport.hasPointerCapture?.(event.pointerId)) viewport.releasePointerCapture(event.pointerId)
  }
  function cancel(event: PointerEvent) { if (gesture?.id === event.pointerId) gesture = null }
  function keyboard(event: KeyboardEvent) {
    if (!editable || event.target !== viewport || event.isComposing) return
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') { event.preventDefault(); if (event.shiftKey) redo(); else undo() }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'y') { event.preventDefault(); redo() }
    if (event.key === 'Delete' || event.key === 'Backspace') { event.preventDefault(); remove() }
    if (event.key === 'Escape') { gesture = null; selectedId = null; tool = 'select' }
  }
</script>

<section class="mx-auto grid w-full max-w-6xl gap-3" data-visual-workbench class:visual-readonly={readOnly}>
  <div><h2 class="m-0 text-base font-semibold">{data.title}</h2><p class="mb-0 mt-1 text-xs leading-6 text-muted-foreground">{tr(readOnly ? 'Saved visual feedback' : 'Draw on the canvas, or select an annotation to add a comment.')}</p></div>
  <VisualToolbar {tool} color={selected?.color ?? color} width={selected?.stroke_width ?? width} {zoom} disabled={!editable}
    {canUndo} {canRedo} selected={!!selected} textSize={selected?.kind === 'text' || tool === 'text'} onTool={chooseTool} onColor={changeColor} onWidth={changeWidth} onUndo={undo} onRedo={redo} onDelete={remove}
    onZoom={(value) => { automaticZoom = false; zoom = value }} onFit={() => { automaticZoom = true; fit() }} {onOpenReview} />
  {#if tool === 'text' && editable}<p class="m-0 text-xs text-muted-foreground">{tr('Click the canvas to place text.')}</p>{/if}
  {#if loadError}<div role="alert" class="flex items-center gap-3 rounded-lg border border-destructive/30 p-3 text-xs"><span class="flex-1">{tr('Original image unavailable')}: {loadError}</span><button type="button" class="rounded border px-3 py-1.5" onclick={() => void loadBackground(source)}>{tr('Retry')}</button></div>{/if}
  {#if loading}<p role="status" class="m-0 text-xs text-muted-foreground">{tr('Loading original image…')}</p>{/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions (The drawing surface supports keyboard undo/delete, with accessible toolbar and annotation-list alternatives.) -->
  <div bind:this={viewport} role="application" aria-label={tr('Canvas')} tabindex="0" class="visual-viewport" class:drawing={tool !== 'select' && editable}
    onpointerdown={start} onpointermove={move} onpointerup={finish} onpointercancel={cancel} onlostpointercapture={cancel} onkeydown={keyboard}>
    <svg bind:this={sheet} width={data.width * zoom} height={data.height * zoom} viewBox={`0 0 ${data.width} ${data.height}`} role="img" aria-label={tr('Visual feedback')} class="visual-sheet">
      <rect x="0" y="0" width={data.width} height={data.height} fill={data.background_color ?? '#ffffff'} />
      {#if backgroundUrl}<image href={backgroundUrl} x="0" y="0" width={data.width} height={data.height} preserveAspectRatio="none" />{/if}
      <VisualMarks annotations={current.annotations} {selectedId} {attachments} />
      {#if gesture}<VisualMarks annotations={[gesture.mark]} {attachments} />{/if}
    </svg>
  </div>
  {#if message}<p role="status" class="m-0 text-xs text-amber-700 dark:text-amber-400">{message}</p>{/if}
  <div class="grid gap-3 lg:grid-cols-[220px_minmax(0,1fr)]">
    <div class="min-w-0 rounded-xl border p-3"><h3 class="m-0 mb-2 text-xs font-medium">{tr('Annotations')} <span class="text-muted-foreground">{current.annotations.length}</span></h3>
      {#if !current.annotations.length}<p class="m-0 text-xs leading-6 text-muted-foreground">{tr('No annotations yet.')}</p>{:else}<ol class="m-0 grid max-h-60 list-none gap-1 overflow-auto p-0">
        {#each current.annotations as mark, index (mark.id)}<li><button type="button" data-visual-list-id={mark.id} aria-pressed={selectedId === mark.id} onclick={() => void select(mark.id)}
          class="flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-xs hover:bg-muted aria-pressed:bg-primary/10"><span class="size-2 shrink-0 rounded-full" style:background={mark.color}></span><span class="shrink-0 text-muted-foreground">{index + 1}</span><span class="truncate">{visualAttachmentText(mark.text || mark.body, attachments) || tr(mark.kind === 'freehand' ? 'Pen' : mark.kind === 'arrow' ? 'Arrow' : mark.kind === 'text' ? 'Text' : 'Rectangle')}</span></button></li>{/each}
      </ol>{/if}
    </div>
    <div bind:this={editorPanel} class="min-w-0">{#if selected}<VisualAnnotationEditor {data} annotation={selected} disabled={!editable}
      onUpdate={(mark) => publish(current.annotations.map((item) => item.id === mark.id ? mark : item))} onDelete={remove} />{/if}</div>
  </div>
  <p class="m-0 text-[11px] leading-5 text-muted-foreground">{tr('The original stays unchanged. A PNG with your annotations is submitted with feedback.')}</p>
</section>
<style>
  .visual-viewport { display: flex; min-height: 180px; max-height: 65vh; align-items: flex-start; overflow: auto; border: 1px solid var(--border); border-radius: 12px; background-color: var(--muted); background-image: radial-gradient(var(--border) 1px, transparent 1px); background-size: 12px 12px; padding: 16px; overscroll-behavior: contain; }
  .visual-viewport:focus-visible { outline: 2px solid var(--ring); outline-offset: 2px; }
  .visual-sheet { flex-shrink: 0; display: block; background: white; box-shadow: 0 2px 12px #00000015; touch-action: pan-x pan-y; user-select: none; }
  .drawing .visual-sheet { touch-action: none; }
  .drawing { cursor: crosshair; }
</style>
