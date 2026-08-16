<script lang="ts">
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

  import { t } from '$lib/i18n'
  import { locale } from '$lib/preferences'
  import type { AnnotationTool } from '../screenCapture'

  export let toolbarWidth = 0
  export let toolbarHeight = 0
  export let toolbarStyle = ''
  export let popoverDown = false
  export let activeTool: AnnotationTool = 'select'
  export let stylePanelOpen = false
  export let overflowPanelOpen = false
  export let currentColor = '#ff4d5d'
  export let currentStrokeWidth = 4
  export let colors: string[] = []
  export let strokeWidths: number[] = []
  export let canUndo = false
  export let canRedo = false
  export let canDelete = false
  export let onBeginDrag: (event: PointerEvent) => void = () => {}
  export let onMoveDrag: (event: PointerEvent) => void = () => {}
  export let onEndDrag: (event: PointerEvent) => void = () => {}
  export let onSetTool: (tool: AnnotationTool) => void = () => {}
  export let onToggleStylePanel: () => void = () => {}
  export let onToggleOverflowPanel: () => void = () => {}
  export let onSetColor: (color: string) => void = () => {}
  export let onSetStrokeWidth: (width: number) => void = () => {}
  export let onUndo: () => void = () => {}
  export let onRedo: () => void = () => {}
  export let onDelete: () => void = () => {}
  export let onFinalize: (copyToClipboard: boolean) => void = () => {}
  export let onCancel: () => void = () => {}
</script>

<div
  bind:clientWidth={toolbarWidth}
  bind:clientHeight={toolbarHeight}
  class="capture-toolbar"
  class:popover-down={popoverDown}
  data-capture-ui
  style={toolbarStyle}
>
  <button
    class="toolbar-drag"
    aria-label={t($locale, 'Drag toolbar')}
    title={t($locale, 'Drag toolbar')}
    onpointerdown={onBeginDrag}
    onpointermove={onMoveDrag}
    onpointerup={onEndDrag}
    onpointercancel={onEndDrag}
    onlostpointercapture={onEndDrag}
  ><GripVertical size={17} /></button>
  <span class="divider"></span>
  <div class="tool-group">
    <button class:active={activeTool === 'select'} onclick={() => onSetTool('select')} title={t($locale, 'Select/edit · V')}><MousePointer2 size={18} /></button>
    <button class:active={activeTool === 'rectangle'} onclick={() => onSetTool('rectangle')} title={t($locale, 'Rectangle · R')}><RectangleHorizontal size={18} /></button>
    <button class:active={activeTool === 'ellipse'} onclick={() => onSetTool('ellipse')} title={t($locale, 'Ellipse · E')}><Circle size={18} /></button>
    <button class:active={activeTool === 'arrow'} onclick={() => onSetTool('arrow')} title={t($locale, 'Arrow · A')}><MoveUpRight size={18} /></button>
    <button class:active={activeTool === 'pen'} onclick={() => onSetTool('pen')} title={t($locale, 'Pen · P')}><Pencil size={18} /></button>
    <button class:active={activeTool === 'text'} onclick={() => onSetTool('text')} title={t($locale, 'Text · T')}><Type size={18} /></button>
    <button class:active={activeTool === 'mosaic'} onclick={() => onSetTool('mosaic')} title={t($locale, 'Mosaic · B')}><Grid3X3 size={18} /></button>
  </div>
  <div class="popup-control">
    <button
      class="style-trigger"
      class:active={stylePanelOpen}
      aria-expanded={stylePanelOpen}
      onclick={onToggleStylePanel}
      title={t($locale, 'Color and thickness')}
    ><i style={`--swatch:${currentColor};transform:scaleY(${currentStrokeWidth / 4})`}></i></button>
    {#if stylePanelOpen}
      <div class="toolbar-popover style-popover" aria-label={t($locale, 'Color and line thickness')}>
        <div class="palette" aria-label={t($locale, 'Annotation color')}>
          {#each colors as color}
            <button
              class:active={currentColor === color}
              class="color-button"
              style={`--swatch:${color}`}
              onclick={() => onSetColor(color)}
              title={t($locale, 'Color {color}', { color })}
            ></button>
          {/each}
        </div>
        <span class="popover-divider"></span>
        <div class="stroke-picker" aria-label={t($locale, 'Line thickness')}>
          {#each strokeWidths as width}
            <button class:active={currentStrokeWidth === width} onclick={() => onSetStrokeWidth(width)} title={`${width}px`}>
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
      onclick={onToggleOverflowPanel}
      title={t($locale, 'More tools')}
    ><Ellipsis size={18} /></button>
    {#if overflowPanelOpen}
      <div class="toolbar-popover more-popover" aria-label={t($locale, 'More tools')}>
        <button class:active={activeTool === 'line'} onclick={() => onSetTool('line')} title={t($locale, 'Line · L')}><Minus size={18} /></button>
        <button class:active={activeTool === 'highlight'} onclick={() => onSetTool('highlight')} title={t($locale, 'Highlight · H')}><Highlighter size={18} /></button>
        <button class:active={activeTool === 'counter'} onclick={() => onSetTool('counter')} title={t($locale, 'Counter · N')}><Hash size={18} /></button>
        <button disabled={!canRedo} onclick={onRedo} title={t($locale, 'Redo · Ctrl/⌘ Shift Z')}><Redo2 size={18} /></button>
        <button disabled={!canDelete} onclick={onDelete} title={t($locale, 'Delete selected annotation · Delete')}><Trash2 size={18} /></button>
      </div>
    {/if}
  </div>
  <span class="divider"></span>
  <div class="tool-group">
    <button disabled={!canUndo} onclick={onUndo} title={t($locale, 'Undo · Ctrl/⌘ Z')}><Undo2 size={18} /></button>
  </div>
  <span class="divider"></span>
  <div class="tool-group actions">
    <button onclick={() => onFinalize(true)} title={t($locale, 'Copy and insert')}><Copy size={18} /></button>
    <button class="confirm" onclick={() => onFinalize(false)} title={t($locale, 'Insert into document · Enter')}><Check size={19} /></button>
    <button class="cancel" onclick={onCancel} title={t($locale, 'Cancel · Esc')}><X size={19} /></button>
  </div>
</div>
